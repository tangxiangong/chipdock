use std::{collections::VecDeque, sync::Arc, time::Duration};

use anyhow::{Result, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tokio::{
    select,
    sync::{Mutex, broadcast, watch},
    time::Instant,
};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortType, StopBits};

mod capture;
mod checksum;
pub mod encoding;
mod framing;
mod modbus;
mod plot;
pub mod program;
mod transport;
pub mod types;
mod usb_ids;
pub mod workbench;
mod workflow;

use transport::{Reader, VIRTUAL_PORTS, Writer};

use encoding::{DecodedText, ReceiveDecoder, encode_text};

use types::{
    ChecksumAlgorithm, ControlLine, FlowControl as ConfigFlowControl, LineEnding,
    Parity as ConfigParity, PortInfo, SendFormat, SendOptions, SerialConfig, SerialStatus,
};

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("串口已被其他会话占用：{0}")]
    PortBusy(String),
    #[error("串口正在发送")]
    Busy,
    #[error("串口未连接")]
    NotConnected,
    #[error("请先断开当前串口")]
    AlreadyConnected,
}

const READ_BUFFER_SIZE: usize = 4096;
const MAX_FRAMES: usize = 2000;
const MAX_BUFFERED_BYTES: usize = 8 * 1024 * 1024;

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollBatch {
    pub status: SerialStatus,
    pub frames: Vec<SerialFrameEvent>,
    pub dropped_frames: u64,
    pub next_cursor: u64,
    pub has_more: bool,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Receive,
    Send,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialFrameEvent {
    pub direction: Direction,
    pub bytes: Vec<u8>,
    pub hex: String,
    pub text: DecodedText,
    pub timestamp: String,
    pub source: String,
    pub sequence: u64,
    pub parsed: Vec<program::Record>,
}

impl SerialFrameEvent {
    // Bound the bytes, three decoded representations and hexadecimal display together.
    fn cost(&self) -> usize {
        self.bytes
            .len()
            .saturating_mul(16)
            .saturating_add(1024)
            .saturating_add(
                self.parsed
                    .iter()
                    .map(|entry| {
                        entry.hex.len() + entry.message.len() + entry.payload_hex.len() + 256
                    })
                    .sum::<usize>(),
            )
    }

    fn new(direction: Direction, bytes: Vec<u8>, text: DecodedText) -> Self {
        Self {
            direction,
            hex: bytes_to_hex(&bytes),
            text,
            bytes,
            source: match direction {
                Direction::Receive => "serial",
                Direction::Send => "manual",
            }
            .into(),
            parsed: Vec::new(),
            sequence: 0,
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("串口发送失败（已写入 {sent} 字节）：{message}")]
pub struct TransmissionError {
    sent: usize,
    message: String,
}

struct SerialConnection {
    generation: u64,
    writer: Writer,
    cancel: watch::Sender<bool>,
    rx: broadcast::Sender<Arc<Vec<u8>>>,
}

#[derive(Clone, PartialEq, Eq)]
struct PortIdentity {
    vid: String,
    pid: String,
    serial_number: String,
}

impl PortIdentity {
    fn of(port: &PortInfo) -> Option<Self> {
        Some(Self {
            vid: port.vid.clone()?,
            pid: port.pid.clone()?,
            serial_number: port
                .serial_number
                .clone()
                .filter(|number| !number.trim().is_empty())?,
        })
    }

    fn unique_path<'a>(&self, ports: &'a [PortInfo]) -> Option<&'a str> {
        let mut matching = ports
            .iter()
            .filter(|port| Self::of(port).as_ref() == Some(self));
        let first = matching.next()?;
        matching.next().is_none().then_some(&first.path)
    }
}

#[derive(Default)]
struct SerialState {
    decoder: ReceiveDecoder,
    framer: framing::Framer,
    program: program::Program,
    program_buffer: Vec<u8>,
    selected_protocol: Option<String>,
    parsed: VecDeque<(u64, program::Record)>,
    rx_offset: u64,
    display_offset: u64,
    pending: Option<(String, Instant)>,
    feedback: Option<String>,
    closed: bool,
    reconnect: Option<PortIdentity>,
    rts: Option<bool>,
    dtr: Option<bool>,
    connection: Option<SerialConnection>,
    generation: u64,
    path: Option<String>,
    config: Option<SerialConfig>,
    rx_bytes: u64,
    tx_bytes: u64,
    opened_at: Option<String>,
    last_error: Option<String>,
    frames: VecDeque<SerialFrameEvent>,
    buffered_bytes: usize,
    next_sequence: u64,
    legacy_cursor: u64,
    started: Option<Instant>,
    count_start: Option<Instant>,
    log: capture::Capture,
    raw: capture::Capture,
    job: workflow::JobState,
    responder: workflow::Responder,
}

impl SerialState {
    fn push(&mut self, mut frame: SerialFrameEvent) {
        let mut cost = frame.cost();
        if cost > MAX_BUFFERED_BYTES {
            frame.parsed.clear();
            cost = frame.cost();
        }
        while self.frames.len() >= MAX_FRAMES || self.buffered_bytes + cost > MAX_BUFFERED_BYTES {
            if let Some(old) = self.frames.pop_front() {
                self.buffered_bytes = self.buffered_bytes.saturating_sub(old.cost());
            } else {
                break;
            }
        }
        self.next_sequence += 1;
        frame.sequence = self.next_sequence;
        self.buffered_bytes += cost;
        self.frames.push_back(frame);
    }

    fn page(&self, cursor: u64, limit: usize) -> PollBatch {
        let first = self
            .frames
            .front()
            .map_or(self.next_sequence + 1, |frame| frame.sequence);
        let cursor = cursor.min(self.next_sequence);
        let dropped = (first - 1).saturating_sub(cursor);
        let frames: Vec<_> = self
            .frames
            .iter()
            .filter(|frame| frame.sequence > cursor)
            .take(limit)
            .cloned()
            .collect();
        let next_cursor = frames
            .last()
            .map_or(cursor + dropped, |frame| frame.sequence);
        PollBatch {
            status: self.status(),
            frames,
            dropped_frames: dropped,
            next_cursor,
            has_more: next_cursor < self.next_sequence,
        }
    }

    fn receive(&mut self, bytes: &[u8]) {
        self.rx_bytes = self.rx_bytes.saturating_add(bytes.len() as u64);
        self.log.queue(Direction::Receive, bytes, "serial");
        self.raw.queue(Direction::Receive, bytes, "serial");
        if let Some(connection) = &self.connection
            && connection.rx.receiver_count() > 0
        {
            let _ = connection.rx.send(Arc::new(bytes.to_vec()));
        }
        let program_offset = self.rx_offset - self.program_buffer.len() as u64;
        self.rx_offset += bytes.len() as u64;
        if self.program.has_receive() {
            self.program_buffer.extend_from_slice(bytes);
            let (records, consumed) = self.program.decode(&self.program_buffer);
            self.program_buffer.drain(..consumed);
            self.expire_feedback();
            if let Some((name, _)) = &self.pending
                && records
                    .iter()
                    .any(|(_, entry)| entry.valid && &entry.name == name)
            {
                self.feedback = Some(format!("收到匹配反馈：{name}（未验证请求序号）"));
                self.pending = None;
            }
            self.parsed.extend(
                records
                    .into_iter()
                    .map(|(end, record)| (program_offset + end as u64, record)),
            );
            if self.parsed.len() > MAX_FRAMES {
                self.parsed.drain(..self.parsed.len() - MAX_FRAMES);
            }
        }
        for bytes in self.framer.push(bytes, Instant::now()) {
            self.push_rx(bytes);
        }
    }

    fn push_rx(&mut self, bytes: Vec<u8>) {
        let decoded = self.decoder.decode(&bytes, false);
        let mut frame = SerialFrameEvent::new(Direction::Receive, bytes, decoded);
        self.display_offset += frame.bytes.len() as u64;
        while self
            .parsed
            .front()
            .is_some_and(|(end, _)| *end <= self.display_offset)
        {
            if let Some((_, record)) = self.parsed.pop_front() {
                frame.parsed.push(record);
            }
        }
        self.push(frame);
    }

    fn flush_rx(&mut self) {
        let bytes = self.framer.flush();
        if !bytes.is_empty() {
            self.push_rx(bytes);
        }
    }

    fn expire_feedback(&mut self) {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, deadline)| Instant::now() > *deadline)
            && let Some((name, _)) = self.pending.take()
        {
            self.feedback = Some(format!("等待反馈超时：{name}"));
        }
    }

    fn status(&self) -> SerialStatus {
        let seconds = self
            .count_start
            .map_or(0.0, |start| start.elapsed().as_secs_f64())
            .max(1.0);
        SerialStatus {
            selected_protocol: self.selected_protocol.clone(),
            job_kind: self.job.kind(),
            job_done: self.job.done,
            job_total: self.job.total,
            job_error: self.job.error.clone(),
            uptime_ms: self
                .started
                .map_or(0, |start| start.elapsed().as_millis() as u64),
            rx_per_second: self
                .count_start
                .map_or(0, |_| (self.rx_bytes as f64 / seconds) as u64),
            tx_per_second: self
                .count_start
                .map_or(0, |_| (self.tx_bytes as f64 / seconds) as u64),
            logging: self.log.active(),
            capturing: self.raw.active(),
            capture_error: self.log.error().or_else(|| self.raw.error()),
            connected: self.connection.is_some(),
            feedback: self.feedback.clone(),
            reconnecting: self.connection.is_none() && self.reconnect.is_some(),
            generation: self.generation,
            rts: self.rts,
            dtr: self.dtr,
            path: self.path.clone(),
            config: self.config.clone(),
            rx_bytes: self.rx_bytes,
            tx_bytes: self.tx_bytes,
            opened_at: self.opened_at.clone(),
            last_error: self.last_error.clone(),
        }
    }
}

#[derive(Default)]
pub struct SerialManager {
    state: Arc<Mutex<SerialState>>,
    reader: Mutex<Option<tokio::task::JoinHandle<()>>>,
    lifecycle: Mutex<()>,
    tx: Arc<Mutex<()>>,
}

impl SerialManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_ports() -> Result<Vec<PortInfo>> {
        let mut ports = tokio_serial::available_ports()
            .map_err(|error| anyhow!("无法读取串口列表：{error}"))?
            .into_iter()
            .filter(|port| {
                !cfg!(target_os = "macos")
                    || (!port.port_name.starts_with("/dev/tty.")
                        && !matches!(
                            port.port_name.as_str(),
                            "/dev/cu.debug-console" | "/dev/cu.Bluetooth-Incoming-Port"
                        ))
            })
            .map(port_info)
            .collect::<Vec<_>>();
        ports.sort_by(|a, b| a.path.cmp(&b.path));
        ports.extend(VIRTUAL_PORTS.map(transport::virtual_info));
        Ok(ports)
    }

    pub async fn connect(&self, path: String, config: SerialConfig) -> Result<SerialStatus> {
        self.connect_with_recovery(path, config, false).await
    }

    pub async fn connect_with_recovery(
        &self,
        path: String,
        config: SerialConfig,
        reconnect: bool,
    ) -> Result<SerialStatus> {
        let _operation = self.lifecycle.lock().await;
        config.validate()?;
        if path.trim().is_empty() {
            bail!("请选择串口");
        }
        {
            let state = self.state.lock().await;
            if state.closed {
                bail!("串口会话已关闭");
            }
            if state.connection.is_some() {
                return Err(ConnectionError::AlreadyConnected.into());
            }
        }
        self.disconnect_inner().await;
        let identity = if reconnect && !VIRTUAL_PORTS.contains(&path.as_str()) {
            let ports = Self::list_ports()?;
            ports
                .iter()
                .find(|port| port.path == path)
                .and_then(PortIdentity::of)
                .filter(|identity| identity.unique_path(&ports).is_some())
        } else {
            None
        };
        let (reader, writer) = transport::open(&path, serial_builder(&path, &config), &config)?;
        Ok(self.attach(reader, writer, path, config, identity).await)
    }

    pub async fn reconnect_if_available(&self) {
        let Ok(_operation) = self.lifecycle.try_lock() else {
            return;
        };
        let (identity, config) = {
            let state = self.state.lock().await;
            if state.closed || state.connection.is_some() {
                return;
            }
            let (Some(identity), Some(config)) = (&state.reconnect, &state.config) else {
                return;
            };
            (identity.clone(), config.clone())
        };
        let Ok(ports) = Self::list_ports() else {
            return;
        };
        let Some(path) = identity.unique_path(&ports) else {
            return;
        };
        let Ok(_tx) = self.tx.try_lock() else {
            return;
        };
        let Ok((reader, writer)) = transport::open(path, serial_builder(path, &config), &config)
        else {
            return;
        };
        self.attach(reader, writer, path.into(), config, Some(identity))
            .await;
    }

    #[cfg(all(test, unix))]
    async fn attach_stream(
        &self,
        stream: tokio_serial::SerialStream,
        path: String,
        config: SerialConfig,
    ) -> SerialStatus {
        let (reader, writer) = transport::simulated(stream, &path).unwrap();
        self.attach(reader, writer, path, config, None).await
    }

    async fn attach(
        &self,
        reader: Reader,
        writer: Writer,
        path: String,
        config: SerialConfig,
        reconnect: Option<PortIdentity>,
    ) -> SerialStatus {
        let (cancel, mut cancelled) = watch::channel(false);
        let (rx, _) = broadcast::channel(64);

        let (generation, status) = {
            let mut state = self.state.lock().await;
            state.generation = state.generation.wrapping_add(1);
            let generation = state.generation;
            state.connection = Some(SerialConnection {
                generation,
                writer,
                cancel,
                rx,
            });
            state.decoder = ReceiveDecoder::new();
            state.framer.flush();
            state.program_buffer.clear();
            state.parsed.clear();
            state.rx_offset = 0;
            state.display_offset = 0;
            state.pending = None;
            state.feedback = None;
            state.responder.reset();
            state.reconnect = reconnect;
            state.rts = None;
            state.dtr = None;
            state.path = Some(path);
            state.config = Some(config);
            state.started = Some(Instant::now());
            state.count_start = state.started;
            state.rx_bytes = 0;
            state.tx_bytes = 0;
            state.opened_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
            state.last_error = None;
            (generation, state.status())
        };

        let state = Arc::clone(&self.state);
        let tx = Arc::clone(&self.tx);
        let shared = Arc::clone(&state);
        let reader_task = tokio::spawn(async move {
            let mut reader = reader;
            let mut buffer = [0_u8; READ_BUFFER_SIZE];

            loop {
                let deadline = state.lock().await.framer.deadline();
                select! {
                    biased;
                    changed = cancelled.changed() => {
                        if changed.is_err() || *cancelled.borrow() {
                            break;
                        }
                    }
                    _ = sleep_deadline(deadline) => {
                        let mut state = state.lock().await;
                        if state.generation == generation && state.framer.deadline().is_some_and(|at| Instant::now() >= at) {
                            state.flush_rx();
                        }
                    }
                    result = reader.read(&mut buffer) => {
                        match result {
                            Ok(0) => {
                                finish_connection(&state, generation, Some("串口已断开".into())).await;
                                break;
                            }
                            Ok(count) => {
                                let mut state = state.lock().await;
                                if state.generation == generation {
                                    state.receive(&buffer[..count]);
                                    if let Some(reply) = state.responder.receive(&buffer[..count]) {
                                        workflow::queue_reply(Arc::clone(&shared), Arc::clone(&tx), reply, generation, cancelled.clone());
                                    }
                                }
                            }
                            Err(error) => {
                                finish_connection(&state, generation, Some(format!("串口读取失败：{error}"))).await;
                                break;
                            }
                        }
                    }
                }
            }

            let mut state = state.lock().await;
            if state.generation == generation {
                state.flush_rx();
                // RX totals and capture already include these bytes.
                if let Some(text) = std::mem::take(&mut state.decoder).finish() {
                    state.push(SerialFrameEvent::new(Direction::Receive, Vec::new(), text));
                }
            }
        });
        *self.reader.lock().await = Some(reader_task);

        status
    }

    pub async fn disconnect(&self) -> SerialStatus {
        let _operation = self.lifecycle.lock().await;
        self.disconnect_inner().await
    }

    pub async fn close(&self) {
        let _operation = self.lifecycle.lock().await;
        self.state.lock().await.closed = true;
        self.disconnect_inner().await;
        let mut state = self.state.lock().await;
        let _ = state.log.stop().await;
        let _ = state.raw.stop().await;
    }

    async fn disconnect_inner(&self) -> SerialStatus {
        let connection = {
            let mut state = self.state.lock().await;
            state.reconnect = None;
            state.last_error = None;
            state.pending = None;
            state.feedback = None;
            state.started = None;
            state.count_start = None;
            state.connection.take()
        };
        if let Some(connection) = connection {
            let _ = connection.cancel.send(true);
        }
        let _tx = self.tx.lock().await;
        if let Some(reader) = self.reader.lock().await.take() {
            let _ = reader.await;
        }
        self.state.lock().await.status()
    }

    pub async fn status(&self) -> SerialStatus {
        let mut state = self.state.lock().await;
        state.expire_feedback();
        state.status()
    }

    pub async fn set_line(&self, line: ControlLine, enabled: bool) -> Result<SerialStatus> {
        let _operation = self.lifecycle.lock().await;
        let mut state = self.state.lock().await;
        let connection = state
            .connection
            .as_ref()
            .ok_or(ConnectionError::NotConnected)?;
        if matches!(line, ControlLine::Rts)
            && state
                .config
                .as_ref()
                .is_some_and(|config| config.flow_control == ConfigFlowControl::Hardware)
        {
            bail!("RTS 由硬件流控控制");
        }
        connection.writer.set_line(line, enabled)?;
        match line {
            ControlLine::Rts => state.rts = Some(enabled),
            ControlLine::Dtr => state.dtr = Some(enabled),
        }
        Ok(state.status())
    }

    pub async fn send(
        &self,
        input: String,
        format: SendFormat,
        line_ending: LineEnding,
    ) -> Result<()> {
        self.send_with_checksum(input, format, line_ending, ChecksumAlgorithm::None)
            .await
    }

    pub async fn send_with_checksum(
        &self,
        input: String,
        format: SendFormat,
        line_ending: LineEnding,
        checksum: ChecksumAlgorithm,
    ) -> Result<()> {
        let bytes = prepare_send(
            &input,
            format,
            line_ending,
            checksum,
            SendOptions::default(),
        )?;
        self.send_bytes(bytes).await
    }

    pub async fn send_options(
        &self,
        input: &str,
        format: SendFormat,
        ending: LineEnding,
        checksum: ChecksumAlgorithm,
        options: SendOptions,
    ) -> Result<()> {
        self.send_bytes(prepare_send(input, format, ending, checksum, options)?)
            .await
    }

    pub async fn send_bytes(&self, bytes: Vec<u8>) -> Result<()> {
        let _tx = self.try_send()?;
        self.write(bytes, "manual").await
    }

    pub async fn send_external(&self, input: &str, format: SendFormat) -> Result<()> {
        let bytes = prepare_send(
            input,
            format,
            LineEnding::None,
            ChecksumAlgorithm::None,
            SendOptions::default(),
        )?;
        let _tx = self.try_send()?;
        self.write(bytes, "ipc").await
    }

    fn try_send(&self) -> Result<tokio::sync::MutexGuard<'_, ()>> {
        self.tx.try_lock().map_err(|_| ConnectionError::Busy.into())
    }

    async fn write(&self, bytes: Vec<u8>, source: &str) -> Result<()> {
        write_transport(&self.state, bytes, source).await
    }

    pub async fn poll(&self) -> PollBatch {
        let mut state = self.state.lock().await;
        state.expire_feedback();
        let batch = state.page(state.legacy_cursor, MAX_FRAMES);
        state.legacy_cursor = batch.next_cursor;
        batch
    }

    pub async fn busy(&self) -> bool {
        self.lifecycle.try_lock().is_err() || self.tx.try_lock().is_err() || {
            let state = self.state.lock().await;
            state.connection.is_some()
                || state.reconnect.is_some()
                || state.log.active()
                || state.raw.active()
        }
    }

    pub async fn protocols(&self) -> Vec<String> {
        self.state.lock().await.program.command_names()
    }

    pub async fn select_protocol(&self, name: Option<String>) -> Result<()> {
        let mut state = self.state.lock().await;
        if state.closed {
            bail!("串口会话已关闭");
        }
        if let Some(name) = &name
            && !state.program.command_names().contains(name)
        {
            bail!("发送模板不存在：{name}");
        }
        state.selected_protocol = name;
        Ok(())
    }

    pub async fn send_command(&self, input: &str, use_selected: bool) -> Result<String> {
        let _tx = self.try_send()?;
        let command = {
            let mut state = self.state.lock().await;
            if state.connection.is_none() {
                return Err(ConnectionError::NotConnected.into());
            }
            let input = if use_selected {
                let name = state
                    .selected_protocol
                    .as_deref()
                    .ok_or_else(|| anyhow!("请先选择发送模板"))?;
                format!("{name} {input}")
            } else {
                input.to_owned()
            };
            let command = state.program.encode(&input)?;
            state.pending = (!command.response.is_empty()).then(|| {
                (
                    command.response.clone(),
                    Instant::now() + Duration::from_millis(command.timeout_ms),
                )
            });
            state.feedback = state
                .pending
                .as_ref()
                .map(|(name, _)| format!("等待反馈：{name}"));
            command
        };
        let hex = bytes_to_hex(&command.bytes);
        self.write(
            command.bytes,
            if use_selected {
                "ipc/command"
            } else {
                "command"
            },
        )
        .await?;
        Ok(hex)
    }

    pub async fn records(&self, cursor: u64, limit: u32) -> PollBatch {
        let mut state = self.state.lock().await;
        state.expire_feedback();
        state.page(cursor, limit.clamp(1, MAX_FRAMES as u32) as usize)
    }
}

async fn write_transport(
    shared: &Arc<Mutex<SerialState>>,
    mut bytes: Vec<u8>,
    source: &str,
) -> Result<()> {
    if bytes.is_empty() || bytes.len() > 256 * 1024 {
        bail!("单次发送内容必须在 1–262144 字节之间");
    }
    let (generation, writer, mut cancel) = {
        let state = shared.lock().await;
        let connection = state
            .connection
            .as_ref()
            .ok_or(ConnectionError::NotConnected)?;
        (
            connection.generation,
            connection.writer.clone(),
            connection.cancel.subscribe(),
        )
    };

    let mut sent = 0;
    // Do not flush with tcdrain, which can block when hardware flow control stalls.
    let result = select! {
        biased;
        _ = cancel.wait_for(|cancelled| *cancelled) => Err(anyhow!("串口连接已结束")),
        result = tokio::time::timeout(Duration::from_secs(3), async {
            while sent < bytes.len() {
                let end = (sent + READ_BUFFER_SIZE).min(bytes.len());
                let count = writer.write(&bytes[sent..end]).await?;
                if count == 0 { bail!("串口写入了 0 字节"); }
                sent += count;
            }
            Ok(())
        }) => result.unwrap_or_else(|_| Err(anyhow!("串口发送超时"))),
    };
    if sent != 0 {
        let mut state = shared.lock().await;
        if state.generation == generation {
            bytes.truncate(sent);
            state.tx_bytes = state.tx_bytes.saturating_add(sent as u64);
            let text = ReceiveDecoder::new().decode(&bytes, true);
            let mut frame = SerialFrameEvent::new(Direction::Send, bytes, text);
            frame.source = source.into();
            state.log.queue(Direction::Send, &frame.bytes, source);
            state.push(frame);
        }
    }
    if let Err(error) = result {
        let error = TransmissionError {
            sent,
            message: error.to_string(),
        };
        finish_connection(shared, generation, Some(error.to_string())).await;
        return Err(error.into());
    }
    Ok(())
}

async fn sleep_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

fn prepare_send(
    input: &str,
    format: SendFormat,
    ending: LineEnding,
    algorithm: ChecksumAlgorithm,
    options: SendOptions,
) -> Result<Vec<u8>> {
    let mut bytes = encode_input(input, format)?;
    if algorithm != ChecksumAlgorithm::None {
        if bytes.is_empty() {
            bail!("发送内容不能为空");
        }
        let code = calculate_range(&bytes, algorithm, options)?;
        let end = bytes.len() - options.skip_suffix as usize;
        bytes.splice(end..end, code);
    }
    if !matches!(format, SendFormat::Hex) || options.hex_line_ending {
        bytes.extend_from_slice(ending.bytes());
    }
    if bytes.is_empty() {
        bail!("发送内容不能为空");
    }
    Ok(bytes)
}

fn calculate_range(
    bytes: &[u8],
    algorithm: ChecksumAlgorithm,
    options: SendOptions,
) -> Result<Vec<u8>> {
    let start = options.skip_prefix as usize;
    let range = bytes
        .len()
        .checked_sub(options.skip_suffix as usize)
        .and_then(|end| bytes.get(start..end))
        .filter(|range| !range.is_empty())
        .ok_or_else(|| anyhow!("校验范围为空或超出发送内容"))?;
    Ok(checksum::calculate(range, algorithm))
}

fn serial_builder(path: &str, config: &SerialConfig) -> tokio_serial::SerialPortBuilder {
    tokio_serial::new(path, config.baud_rate)
        .data_bits(match config.data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            _ => DataBits::Eight,
        })
        .stop_bits(if config.stop_bits == 2 {
            StopBits::Two
        } else {
            StopBits::One
        })
        .parity(match config.parity {
            ConfigParity::None | ConfigParity::Mark | ConfigParity::Space => Parity::None,
            ConfigParity::Odd => Parity::Odd,
            ConfigParity::Even => Parity::Even,
        })
        .flow_control(match config.flow_control {
            ConfigFlowControl::None => FlowControl::None,
            ConfigFlowControl::Software => FlowControl::Software,
            ConfigFlowControl::Hardware => FlowControl::Hardware,
        })
        .timeout(Duration::from_millis(100))
}

async fn finish_connection(
    state: &Arc<Mutex<SerialState>>,
    generation: u64,
    error: Option<String>,
) {
    let mut state = state.lock().await;
    if state
        .connection
        .as_ref()
        .is_some_and(|connection| connection.generation == generation)
    {
        state.connection = None;
        state.started = None;
        state.count_start = None;
        state.pending = None;
        state.last_error = error;
    }
}

fn port_info(port: tokio_serial::SerialPortInfo) -> PortInfo {
    let (manufacturer, serial_number, vid, pid, product) = match port.port_type {
        SerialPortType::UsbPort(usb) => {
            let (manufacturer, product) =
                usb_ids::names(usb.vid, usb.pid, usb.manufacturer, usb.product);
            (
                manufacturer,
                usb.serial_number,
                Some(format!("{:04X}", usb.vid)),
                Some(format!("{:04X}", usb.pid)),
                product,
            )
        }
        _ => (None, None, None, None, None),
    };
    PortInfo {
        display_name: product.unwrap_or_else(|| short_port_name(&port.port_name)),
        path: port.port_name,
        manufacturer,
        serial_number,
        vid,
        pid,
    }
}

fn short_port_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn encode_input(input: &str, format: SendFormat) -> Result<Vec<u8>> {
    match format {
        SendFormat::Ascii | SendFormat::Gbk | SendFormat::Utf8 | SendFormat::Latin1 => {
            encode_text(input, format)
        }
        SendFormat::Hex => parse_hex(input),
    }
}

fn parse_hex(input: &str) -> Result<Vec<u8>> {
    let compact = input
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',' && *character != '-')
        .collect::<String>();
    if compact.is_empty() {
        return Ok(Vec::new());
    }
    if !compact.is_ascii() {
        bail!("包含无效的十六进制字符");
    }
    if compact.len() % 2 != 0 {
        bail!("十六进制内容必须由完整字节组成");
    }
    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| anyhow!("包含无效的十六进制字符"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[cfg(unix)]
    use tokio_serial::SerialStream;

    #[test]
    fn discovered_ports_use_catalog_names_and_keep_unknown_devices() {
        let known = port_info(tokio_serial::SerialPortInfo {
            port_name: "COM3".into(),
            port_type: SerialPortType::UsbPort(tokio_serial::UsbPortInfo {
                interface: None,
                vid: 0x1a86,
                pid: 0x7523,
                manufacturer: None,
                product: None,
                serial_number: Some("serial-123".into()),
            }),
        });
        assert_eq!(known.display_name, "CH340 serial converter");
        assert_eq!(known.manufacturer.as_deref(), Some("QinHeng Electronics"));
        assert_eq!(known.serial_number.as_deref(), Some("serial-123"));
        assert_eq!(known.vid.as_deref(), Some("1A86"));
        assert_eq!(known.pid.as_deref(), Some("7523"));
        let unknown = port_info(tokio_serial::SerialPortInfo {
            port_name: "/dev/cu.custom".into(),
            port_type: SerialPortType::Unknown,
        });
        assert_eq!(unknown.display_name, "cu.custom");
        assert_eq!(unknown.path, "/dev/cu.custom");
    }

    #[test]
    fn reconnect_requires_one_match_and_can_follow_a_changed_path() {
        let port = PortInfo {
            path: "old".into(),
            display_name: "Device".into(),
            manufacturer: None,
            serial_number: Some("123".into()),
            vid: Some("1111".into()),
            pid: Some("2222".into()),
        };
        let identity = PortIdentity::of(&port).unwrap();
        let moved = PortInfo {
            path: "new".into(),
            ..port.clone()
        };
        assert_eq!(
            identity.unique_path(std::slice::from_ref(&moved)),
            Some("new")
        );
        assert_eq!(identity.unique_path(&[port, moved]), None);
    }

    #[test]
    fn parses_common_hex_input_styles() {
        assert_eq!(parse_hex("AA 01 ff").unwrap(), vec![0xAA, 0x01, 0xFF]);
        assert_eq!(parse_hex("0xAA,0x01").unwrap(), vec![0xAA, 0x01]);
        assert_eq!(parse_hex("AA-01").unwrap(), vec![0xAA, 0x01]);
    }

    #[test]
    fn rejects_incomplete_hex_bytes() {
        assert!(parse_hex("A 01").is_err());
    }

    #[test]
    fn hex_sends_raw_bytes_and_rejects_non_ascii_without_panicking() {
        assert_eq!(
            encode_input("D6 D0 CE C4", SendFormat::Hex).unwrap(),
            [0xd6, 0xd0, 0xce, 0xc4]
        );
        assert!(encode_input("中文", SendFormat::Hex).is_err());
        assert!(encode_input("é", SendFormat::Hex).is_err());
    }

    #[test]
    fn line_endings_are_only_bytes() {
        assert_eq!(LineEnding::Crlf.bytes(), b"\r\n");
    }

    #[test]
    fn latin1_checksum_skips_suffix_and_hex_line_endings_are_opt_in() {
        let options = SendOptions {
            skip_prefix: 1,
            skip_suffix: 1,
            hex_line_ending: true,
        };
        assert_eq!(
            prepare_send(
                "41 FF 42",
                SendFormat::Hex,
                LineEnding::Cr,
                ChecksumAlgorithm::Lrc,
                options
            )
            .unwrap(),
            [0x41, 0xff, 1, 0x42, 13]
        );
        assert_eq!(encode_input("ÿ", SendFormat::Latin1).unwrap(), [0xff]);
        assert!(encode_input("中", SendFormat::Latin1).is_err());
        assert_eq!(
            checksum::calculate(b"123456789", ChecksumAlgorithm::Crc16Ccitt),
            [0x29, 0xb1]
        );
        assert_eq!(
            checksum::calculate(b"123456789", ChecksumAlgorithm::Crc32),
            [0x26, 0x39, 0xf4, 0xcb]
        );
    }

    #[tokio::test]
    async fn record_cursors_are_independent_and_report_eviction() {
        let manager = SerialManager::new();
        {
            let mut state = manager.state.lock().await;
            for _ in 0..MAX_FRAMES + 2 {
                state.receive(b"x");
            }
        }
        let first = manager.records(0, 2).await;
        assert_eq!(first.dropped_frames, 2);
        assert_eq!(first.frames[0].sequence, 3);
        assert_eq!(manager.poll().await.frames.len(), MAX_FRAMES);
        assert_eq!(manager.records(0, 2).await.next_cursor, first.next_cursor);
        assert_eq!(
            manager.records(first.next_cursor, 1).await.frames[0].sequence,
            5
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn checksummed_sends_encode_the_payload_before_checksum_and_line_ending() {
        let (mut master, slave) = SerialStream::pair().unwrap();
        let manager = SerialManager::new();
        manager
            .attach_stream(slave, "pty-checksum".into(), SerialConfig::default())
            .await;
        for (input, format, expected) in [
            ("41 42", SendFormat::Hex, vec![0x41, 0x42, 0x83]),
            ("AB", SendFormat::Ascii, vec![0x41, 0x42, 0x83, 0x0d, 0x0a]),
            ("中", SendFormat::Gbk, vec![0xd6, 0xd0, 0xa6, 0x0d, 0x0a]),
            (
                "中",
                SendFormat::Utf8,
                vec![0xe4, 0xb8, 0xad, 0x49, 0x0d, 0x0a],
            ),
        ] {
            for _ in 0..2 {
                manager
                    .send_with_checksum(
                        input.into(),
                        format,
                        LineEnding::Crlf,
                        ChecksumAlgorithm::Sum8,
                    )
                    .await
                    .unwrap();
                let mut written = vec![0; expected.len()];
                tokio::time::timeout(Duration::from_secs(1), master.read_exact(&mut written))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(written, expected, "{format:?}");
                let batch = manager.poll().await;
                assert_eq!(batch.frames.last().unwrap().bytes, expected);
            }
        }
        assert_eq!(manager.disconnect().await.tx_bytes, 38);
    }

    #[tokio::test]
    async fn empty_payload_cannot_send_a_checksum_only_frame() {
        let manager = SerialManager::new();
        let error = manager
            .send_with_checksum(
                " ".into(),
                SendFormat::Hex,
                LineEnding::None,
                ChecksumAlgorithm::Crc16Modbus,
            )
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "发送内容不能为空");
    }

    #[test]
    fn display_backpressure_drops_records_without_changing_transport_totals() {
        let mut state = SerialState {
            rx_bytes: 123,
            ..Default::default()
        };
        for _ in 0..=MAX_FRAMES {
            let text = ReceiveDecoder::new().decode(b"x", true);
            state.push(SerialFrameEvent::new(
                Direction::Receive,
                b"x".to_vec(),
                text,
            ));
        }
        assert_eq!(state.frames.len(), MAX_FRAMES);
        assert_eq!(state.page(0, 1).dropped_frames, 1);
        assert!(state.buffered_bytes <= MAX_BUFFERED_BYTES);
        assert_eq!(state.status().rx_bytes, 123);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stalled_write_in_one_session_leaves_other_session_writable() {
        let (_unread, slave) = SerialStream::pair().unwrap();
        let stalled = Arc::new(SerialManager::new());
        stalled
            .attach_stream(slave, "stalled-simulation".into(), SerialConfig::default())
            .await;
        let active = Arc::clone(&stalled);
        let pending = tokio::spawn(async move { active.send_bytes(vec![0x01; 256 * 1024]).await });
        tokio::time::timeout(Duration::from_secs(1), async {
            while stalled.tx.try_lock().is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let (mut peer, slave) = SerialStream::pair().unwrap();
        let independent = SerialManager::new();
        independent
            .attach_stream(
                slave,
                "independent-simulation".into(),
                SerialConfig::default(),
            )
            .await;
        tokio::time::timeout(Duration::from_secs(1), async {
            independent.send_bytes(vec![0x42]).await.unwrap();
            assert_eq!(peer.read_u8().await.unwrap(), 0x42);
            stalled.disconnect().await;
        })
        .await
        .unwrap();
        assert!(pending.await.unwrap().is_err());
        assert!(independent.status().await.connected);
        independent.close().await;
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pty_roundtrip_and_disconnect_stop_the_old_reader() {
        let (mut master, slave) = SerialStream::pair().unwrap();
        let manager = SerialManager::new();
        manager
            .attach_stream(slave, "pty".into(), SerialConfig::default())
            .await;
        master.write_all("中文".as_bytes()).await.unwrap();
        let mut received = String::new();
        for _ in 0..100 {
            let batch = manager.poll().await;
            for frame in batch.frames {
                received.push_str(&frame.text.utf8);
            }
            if received == "中文" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(received, "中文");
        manager
            .send("41 42".into(), SendFormat::Hex, LineEnding::Crlf)
            .await
            .unwrap();
        let mut written = [0; 2];
        tokio::time::timeout(Duration::from_secs(1), master.read_exact(&mut written))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&written, b"AB");
        let status = manager.disconnect().await;
        assert!(!status.connected);
        assert_eq!(status.rx_bytes, 6);
        assert_eq!(status.tx_bytes, 2);
        assert!(manager.reader.lock().await.is_none());
        let (_master, slave) = SerialStream::pair().unwrap();
        manager
            .attach_stream(slave, "pty".into(), SerialConfig::default())
            .await;
        assert_eq!(manager.poll().await.status.rx_bytes, 0);
        manager.disconnect().await;
    }
}
