use std::{collections::VecDeque, sync::Arc, time::Duration};

use anyhow::{Result, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, WriteHalf},
    select,
    sync::{Mutex, watch},
};
use tokio_serial::{
    DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialPortType, SerialStream, StopBits,
};

pub mod encoding;
pub mod service;
pub mod types;
mod usb_ids;

use encoding::{DecodedText, ReceiveDecoder, encode_text};

use types::{
    FlowControl as ConfigFlowControl, LineEnding, Parity as ConfigParity, PortInfo, SendFormat,
    SerialConfig, SerialStatus,
};

const READ_BUFFER_SIZE: usize = 4096;
const MAX_FRAMES: usize = 2000;
const MAX_BUFFERED_BYTES: usize = 8 * 1024 * 1024;

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct PollBatch {
    pub status: SerialStatus,
    pub frames: Vec<SerialFrameEvent>,
    pub dropped_frames: u64,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(uniffi::Enum)]
pub enum Direction {
    Receive,
    Send,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct SerialFrameEvent {
    pub direction: Direction,
    pub bytes: Vec<u8>,
    pub hex: String,
    pub text: DecodedText,
    pub timestamp: String,
}

impl SerialFrameEvent {
    // Bound the bytes, three decoded representations and hexadecimal display together.
    fn cost(&self) -> usize {
        self.bytes.len().saturating_mul(16).saturating_add(1024)
    }

    fn new(direction: Direction, bytes: Vec<u8>, text: DecodedText) -> Self {
        Self {
            direction,
            hex: bytes_to_hex(&bytes),
            text,
            bytes,
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

struct SerialConnection {
    generation: u64,
    writer: Arc<Mutex<WriteHalf<SerialStream>>>,
    cancel: watch::Sender<bool>,
}

#[derive(Default)]
struct SerialState {
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
    dropped_frames: u64,
}

impl SerialState {
    fn push(&mut self, frame: SerialFrameEvent) {
        let cost = frame.cost();
        if cost > MAX_BUFFERED_BYTES {
            self.dropped_frames = self.dropped_frames.saturating_add(1);
            return;
        }
        while self.frames.len() >= MAX_FRAMES || self.buffered_bytes + cost > MAX_BUFFERED_BYTES {
            if let Some(old) = self.frames.pop_front() {
                self.buffered_bytes = self.buffered_bytes.saturating_sub(old.cost());
                self.dropped_frames = self.dropped_frames.saturating_add(1);
            } else {
                break;
            }
        }
        self.buffered_bytes += cost;
        self.frames.push_back(frame);
    }

    fn status(&self) -> SerialStatus {
        SerialStatus {
            connected: self.connection.is_some(),
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
        Ok(ports)
    }

    pub async fn connect(&self, path: String, config: SerialConfig) -> Result<SerialStatus> {
        config.validate()?;
        if path.trim().is_empty() {
            bail!("请选择串口");
        }

        if self.state.lock().await.connection.is_some() {
            bail!("请先断开当前串口");
        }
        self.disconnect().await;
        let stream = serial_builder(&path, &config)
            .open_native_async()
            .map_err(|error| anyhow!("无法打开 {path}：{error}"))?;
        Ok(self.attach_stream(stream, path, config).await)
    }

    async fn attach_stream(
        &self,
        stream: SerialStream,
        path: String,
        config: SerialConfig,
    ) -> SerialStatus {
        let (reader, writer) = tokio::io::split(stream);
        let (cancel, mut cancelled) = watch::channel(false);

        let (generation, old_connection, status) = {
            let mut state = self.state.lock().await;
            state.generation = state.generation.wrapping_add(1);
            let generation = state.generation;
            let old_connection = state.connection.take();
            state.connection = Some(SerialConnection {
                generation,
                writer: Arc::new(Mutex::new(writer)),
                cancel,
            });
            state.path = Some(path);
            state.config = Some(config);
            state.rx_bytes = 0;
            state.tx_bytes = 0;
            state.opened_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
            state.last_error = None;
            (generation, old_connection, state.status())
        };

        if let Some(old) = old_connection {
            let _ = old.cancel.send(true);
        }

        let state = Arc::clone(&self.state);
        let reader_task = tokio::spawn(async move {
            let mut reader = reader;
            let mut decoder = ReceiveDecoder::new();
            let mut buffer = [0_u8; READ_BUFFER_SIZE];

            loop {
                select! {
                    changed = cancelled.changed() => {
                        if changed.is_err() || *cancelled.borrow() {
                            break;
                        }
                    }
                    result = reader.read(&mut buffer) => {
                        match result {
                            Ok(0) => {
                                finish_connection(&state, generation, Some("串口已断开".into())).await;
                                break;
                            }
                            Ok(count) => {
                                let bytes = buffer[..count].to_vec();
                                let mut state = state.lock().await;
                                if state.connection.as_ref().is_some_and(|connection| connection.generation == generation) {
                                    state.rx_bytes = state.rx_bytes.saturating_add(count as u64);
                                    let text = decoder.decode(&bytes, false);
                                    state.push(SerialFrameEvent::new(Direction::Receive, bytes, text));
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

            // The bytes were already recorded. Only publish the decoder tail;
            // an empty byte vector keeps RX totals and HEX data unchanged.
            if let Some(text) = decoder.finish() {
                let mut state = state.lock().await;
                if state.generation == generation {
                    state.push(SerialFrameEvent::new(Direction::Receive, Vec::new(), text));
                }
            }
        });
        *self.reader.lock().await = Some(reader_task);

        status
    }

    pub async fn disconnect(&self) -> SerialStatus {
        let (connection, status) = {
            let mut state = self.state.lock().await;
            let connection = state.connection.take();
            state.last_error = None;
            (connection, state.status())
        };
        if let Some(connection) = connection {
            let _ = connection.cancel.send(true);
        }
        if let Some(reader) = self.reader.lock().await.take() {
            let _ = reader.await;
        }
        status
    }

    pub async fn send(
        &self,
        input: String,
        format: SendFormat,
        line_ending: LineEnding,
    ) -> Result<()> {
        let mut bytes = encode_input(&input, format)?;
        if !matches!(format, SendFormat::Hex) {
            bytes.extend_from_slice(line_ending.bytes());
        }
        if bytes.is_empty() {
            bail!("发送内容不能为空");
        }

        let (generation, writer) = {
            let state = self.state.lock().await;
            let connection = state
                .connection
                .as_ref()
                .ok_or_else(|| anyhow!("串口未连接"))?;
            (connection.generation, Arc::clone(&connection.writer))
        };

        let write_result = tokio::time::timeout(Duration::from_secs(3), async {
            let mut writer = writer.lock().await;
            // SerialStream writes directly to the OS. Its Unix flush calls
            // blocking tcdrain, which cannot be interrupted by an async timeout
            // when flow control stalls or a PTY peer has not read the bytes.
            writer.write_all(&bytes).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "串口发送超时",
            ))
        });

        if let Err(error) = write_result {
            let reason = format!("串口发送失败：{error}");
            finish_connection(&self.state, generation, Some(reason.clone())).await;
            bail!(reason);
        }

        let mut state = self.state.lock().await;
        if state
            .connection
            .as_ref()
            .is_some_and(|connection| connection.generation == generation)
        {
            state.tx_bytes = state.tx_bytes.saturating_add(bytes.len() as u64);
            let text = ReceiveDecoder::new().decode(&bytes, true);
            state.push(SerialFrameEvent::new(Direction::Send, bytes, text));
        }
        Ok(())
    }

    pub async fn poll(&self) -> PollBatch {
        let mut state = self.state.lock().await;
        let frames = state.frames.drain(..).collect();
        state.buffered_bytes = 0;
        PollBatch {
            status: state.status(),
            frames,
            dropped_frames: std::mem::take(&mut state.dropped_frames),
        }
    }
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
            ConfigParity::None => Parity::None,
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
        SendFormat::Ascii | SendFormat::Gbk | SendFormat::Utf8 => encode_text(input, format),
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
        assert_eq!(state.dropped_frames, 1);
        assert!(state.buffered_bytes <= MAX_BUFFERED_BYTES);
        assert_eq!(state.status().rx_bytes, 123);
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
