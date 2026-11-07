use anyhow::{Result, anyhow, bail, ensure};
use regex::bytes::Regex;
use serde::Deserialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    select,
    sync::{Mutex, broadcast, watch},
    time::Instant,
};

use super::{
    Direction, SerialFrameEvent, SerialManager, SerialState,
    capture::quote,
    encode_input, prepare_send,
    types::{ChecksumAlgorithm, LineEnding, SendFormat, SendOptions},
    write_transport,
};

const MAX_MATCH: usize = 16 * 1024;
const MAX_STEPS: usize = 10_000;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Packet {
    name: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    order: i32,
    #[serde(default = "enabled")]
    enabled: bool,
    text: String,
    format: SendFormat,
}

fn enabled() -> bool {
    true
}

pub fn validate_packets(packets: &[Packet]) -> Result<()> {
    ensure!(packets.len() <= 256, "包库最多 256 条");
    let mut names = std::collections::HashSet::new();
    for packet in packets {
        ensure!(
            !packet.name.trim().is_empty()
                && packet.name.len() <= 128
                && names.insert(&packet.name),
            "包名称须为 1–128 字节且不可重复"
        );
        ensure!(
            packet.group.len() <= 128 && packet.note.len() <= 1024,
            "分组或备注过长"
        );
        ensure!(
            matches!(packet.format, SendFormat::Hex | SendFormat::Utf8),
            "包库仅使用 HEX 或 UTF-8 文本"
        );
        let bytes = encode_input(&packet.text, packet.format)?;
        ensure!(
            (1..=65536).contains(&bytes.len()),
            "包长度须为 1–65536 字节"
        );
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MatchSpec {
    kind: MatchKind,
    pattern: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum MatchKind {
    Text,
    Hex,
    Regex,
}

enum Matcher {
    Bytes(Vec<u8>),
    Regex(Regex),
}

impl Matcher {
    fn new(spec: &MatchSpec) -> Result<Self> {
        ensure!(
            (1..=1024).contains(&spec.pattern.len()),
            "匹配条件须为 1–1024 字节"
        );
        match spec.kind {
            MatchKind::Regex => {
                let regex = regex::bytes::RegexBuilder::new(&spec.pattern)
                    .size_limit(256 * 1024)
                    .build()?;
                ensure!(!regex.is_match(b""), "正则表达式不能匹配空串");
                Ok(Self::Regex(regex))
            }
            MatchKind::Text => Ok(Self::Bytes(spec.pattern.as_bytes().to_vec())),
            MatchKind::Hex => {
                let bytes = encode_input(&spec.pattern, SendFormat::Hex)?;
                ensure!(!bytes.is_empty(), "HEX 匹配条件不能为空");
                Ok(Self::Bytes(bytes))
            }
        }
    }
    fn matches(&self, bytes: &[u8]) -> bool {
        match self {
            Self::Bytes(pattern) => bytes.windows(pattern.len()).any(|window| window == pattern),
            Self::Regex(regex) => regex.is_match(bytes),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Message {
    text: String,
    format: SendFormat,
    #[serde(default)]
    ending: Option<LineEnding>,
    #[serde(default)]
    checksum: ChecksumAlgorithm,
    #[serde(default)]
    options: SendOptions,
}

impl Message {
    fn prepare(&self) -> Result<Vec<u8>> {
        let bytes = prepare_send(
            &self.text,
            self.format,
            self.ending.unwrap_or(LineEnding::None),
            self.checksum,
            self.options,
        )?;
        ensure!(bytes.len() <= 256 * 1024, "单次发送内容超过上限");
        Ok(bytes)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Step {
    send: Option<Message>,
    #[serde(default)]
    delay_ms: u64,
    wait: Option<MatchSpec>,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    1000
}

struct PreparedStep {
    bytes: Option<Vec<u8>>,
    delay: Duration,
    matcher: Option<Matcher>,
    timeout: Duration,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Job {
    Batch {
        packets: Vec<Packet>,
        selected: Vec<String>,
        interval_ms: u64,
    },
    File {
        path: PathBuf,
        chunk_bytes: usize,
        interval_ms: u64,
    },
    Macro {
        steps: Vec<Step>,
        cycles: u32,
    },
    Repeat {
        message: Message,
        interval_ms: u64,
    },
}

enum PreparedJob {
    Batch {
        packets: Vec<Vec<u8>>,
        delay: Duration,
    },
    File {
        file: tokio::fs::File,
        size: u64,
        chunk: usize,
        delay: Duration,
    },
    Macro {
        steps: Vec<PreparedStep>,
        cycles: u32,
    },
    Repeat {
        bytes: Vec<u8>,
        delay: Duration,
    },
}

fn delay(ms: u64, min: u64) -> Result<Duration> {
    ensure!((min..=3_600_000).contains(&ms), "间隔超出范围");
    Ok(Duration::from_millis(ms))
}

impl Job {
    async fn prepare(self) -> Result<PreparedJob> {
        match self {
            Self::Batch {
                mut packets,
                selected,
                interval_ms,
            } => {
                validate_packets(&packets)?;
                let delay = delay(interval_ms, 0)?;
                packets.sort_by_key(|packet| packet.order);
                let bytes = if selected.is_empty() {
                    packets
                        .iter()
                        .filter(|packet| packet.enabled)
                        .map(|packet| encode_input(&packet.text, packet.format))
                        .collect::<Result<Vec<_>>>()?
                } else {
                    ensure!(selected.len() <= 256, "最多选择 256 个包");
                    selected
                        .iter()
                        .map(|name| {
                            let packet = packets
                                .iter()
                                .find(|packet| packet.enabled && &packet.name == name)
                                .ok_or_else(|| anyhow!("包不存在或已禁用：{name}"))?;
                            encode_input(&packet.text, packet.format)
                        })
                        .collect::<Result<Vec<_>>>()?
                };
                ensure!(!bytes.is_empty(), "请选择启用的包");
                Ok(PreparedJob::Batch {
                    packets: bytes,
                    delay,
                })
            }
            Self::File {
                path,
                chunk_bytes,
                interval_ms,
            } => {
                ensure!(
                    (1..=65536).contains(&chunk_bytes),
                    "块大小须为 1–65536 字节"
                );
                let delay = delay(interval_ms, 0)?;
                ensure!(
                    tokio::fs::metadata(&path).await?.is_file(),
                    "请选择普通文件"
                );
                let file = tokio::fs::File::open(path).await?;
                let metadata = file.metadata().await?;
                ensure!(
                    metadata.is_file() && metadata.len() > 0,
                    "请选择非空普通文件"
                );
                Ok(PreparedJob::File {
                    file,
                    size: metadata.len(),
                    chunk: chunk_bytes,
                    delay,
                })
            }
            Self::Macro { steps, cycles } => {
                ensure!(
                    (1..=128).contains(&steps.len())
                        && (1..=1000).contains(&cycles)
                        && steps.len() * cycles as usize <= MAX_STEPS,
                    "宏最多 128 步、1000 轮，执行上限为 10000 步"
                );
                let prepared = steps
                    .iter()
                    .map(|step| {
                        ensure!(
                            step.send.is_some() || step.wait.is_some() || step.delay_ms > 0,
                            "宏步骤不能为空"
                        );
                        Ok(PreparedStep {
                            bytes: step.send.as_ref().map(Message::prepare).transpose()?,
                            delay: delay(step.delay_ms, 0)?,
                            matcher: step.wait.as_ref().map(Matcher::new).transpose()?,
                            timeout: delay(step.timeout_ms, 10)?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(PreparedJob::Macro {
                    steps: prepared,
                    cycles,
                })
            }
            Self::Repeat {
                message,
                interval_ms,
            } => Ok(PreparedJob::Repeat {
                bytes: message.prepare()?,
                delay: delay(interval_ms, 10)?,
            }),
        }
    }
}

impl PreparedJob {
    fn kind_and_total(&self) -> (&'static str, u64) {
        match self {
            Self::Batch { packets, .. } => ("batch", packets.len() as u64),
            Self::File { size, .. } => ("file", *size),
            Self::Macro { steps, cycles } => ("macro", steps.len() as u64 * u64::from(*cycles)),
            Self::Repeat { .. } => ("repeat", 0),
        }
    }
}

#[derive(Default)]
pub(super) struct JobState {
    kind: Option<&'static str>,
    cancel: Option<watch::Sender<bool>>,
    pub done: u64,
    pub total: u64,
    pub error: Option<String>,
    report: Vec<StepResult>,
}

struct StepResult {
    cycle: u32,
    step: usize,
    result: String,
}

impl JobState {
    pub fn kind(&self) -> Option<String> {
        self.cancel.as_ref().and(self.kind).map(str::to_owned)
    }

    pub fn cancel(&self) {
        if let Some(cancel) = &self.cancel {
            let _ = cancel.send(true);
        }
    }
}

pub async fn start(session: &Arc<SerialManager>, job: Job) -> Result<()> {
    let generation = session
        .state
        .lock()
        .await
        .connection
        .as_ref()
        .ok_or(super::ConnectionError::NotConnected)?
        .generation;
    let prepared = job.prepare().await?;
    let (kind, total) = prepared.kind_and_total();
    let gate = Arc::clone(&session.tx)
        .try_lock_owned()
        .map_err(|_| anyhow!("串口正在发送"))?;
    let (receiver, ended, cancelled) = {
        let mut state = session.state.lock().await;
        let connection = state
            .connection
            .as_ref()
            .ok_or_else(|| anyhow!("串口未连接"))?;
        ensure!(connection.generation == generation, "串口连接已结束");
        let receiver = connection.rx.subscribe();
        let ended = connection.cancel.subscribe();
        let (cancel, cancelled) = watch::channel(false);
        state.job = JobState {
            kind: Some(kind),
            total,
            cancel: Some(cancel),
            ..Default::default()
        };
        (receiver, ended, cancelled)
    };
    let state = Arc::clone(&session.state);
    tokio::spawn(async move {
        let mut runner = Runner {
            state: Arc::clone(&state),
            receiver,
            ended,
            cancelled,
            kind,
        };
        let result = runner.run(prepared).await;
        let mut state = state.lock().await;
        state.job.error = result.err().map(|error| error.to_string());
        state.job.cancel = None;
        drop(gate);
    });
    Ok(())
}

struct Runner {
    state: Arc<Mutex<SerialState>>,
    receiver: broadcast::Receiver<Arc<Vec<u8>>>,
    ended: watch::Receiver<bool>,
    cancelled: watch::Receiver<bool>,
    kind: &'static str,
}

impl Runner {
    async fn wait<T>(&mut self, work: impl Future<Output = T>) -> Result<T> {
        select! {
            biased;
            _ = self.ended.wait_for(|value| *value) => bail!("串口连接已结束"),
            _ = self.cancelled.wait_for(|value| *value) => bail!("发送任务已取消"),
            result = work => Ok(result),
        }
    }

    async fn send(&self, bytes: Vec<u8>) -> Result<()> {
        ensure!(!*self.cancelled.borrow(), "发送任务已取消");
        write_transport(&self.state, bytes, self.kind).await
    }

    async fn advance(&self, amount: u64) {
        self.state.lock().await.job.done += amount;
    }

    async fn run(&mut self, job: PreparedJob) -> Result<()> {
        match job {
            PreparedJob::Batch { packets, delay } => {
                for (index, bytes) in packets.into_iter().enumerate() {
                    if index > 0 {
                        self.wait(tokio::time::sleep(delay)).await?;
                    }
                    self.send(bytes).await?;
                    self.advance(1).await;
                }
            }
            PreparedJob::File {
                mut file,
                size,
                chunk,
                delay,
            } => {
                let mut buffer = vec![0; chunk];
                let mut left = size;
                while left != 0 {
                    let size = chunk.min(left.min(usize::MAX as u64) as usize);
                    let count = self.wait(file.read(&mut buffer[..size])).await??;
                    ensure!(count != 0, "文件在发送期间被截短");
                    if let Err(error) = self.send(buffer[..count].to_vec()).await {
                        if let Some(failure) = error.downcast_ref::<super::TransmissionError>() {
                            self.advance(failure.sent as u64).await;
                        }
                        return Err(error);
                    }
                    self.advance(count as u64).await;
                    left -= count as u64;
                    if left != 0 {
                        self.wait(tokio::time::sleep(delay)).await?;
                    }
                }
            }
            PreparedJob::Macro { steps, cycles } => {
                for cycle in 1..=cycles {
                    for (index, step) in steps.iter().enumerate() {
                        let result = self.run_step(step).await;
                        let summary = result
                            .as_ref()
                            .map(|_| "OK".into())
                            .unwrap_or_else(|error| error.to_string());
                        self.state.lock().await.job.report.push(StepResult {
                            cycle,
                            step: index + 1,
                            result: summary,
                        });
                        result?;
                        self.advance(1).await;
                    }
                }
            }
            PreparedJob::Repeat { bytes, delay } => loop {
                self.send(bytes.clone()).await?;
                self.advance(1).await;
                self.wait(tokio::time::sleep(delay)).await?;
            },
        }
        Ok(())
    }

    async fn run_step(&mut self, step: &PreparedStep) -> Result<()> {
        self.wait(tokio::time::sleep(step.delay)).await?;
        let mut response = self.receiver.resubscribe();
        if let Some(bytes) = &step.bytes {
            self.send(bytes.clone()).await?;
        }
        if let Some(matcher) = &step.matcher {
            let matched = self
                .wait(tokio::time::timeout(step.timeout, async {
                    let mut buffer = Vec::new();
                    loop {
                        let chunk = response.recv().await?;
                        append(&mut buffer, &chunk);
                        if matcher.matches(&buffer) {
                            return Ok::<_, broadcast::error::RecvError>(());
                        }
                    }
                }))
                .await?;
            matched.map_err(|_| anyhow!("等待宏响应超时"))??;
        }
        Ok(())
    }
}

fn append(buffer: &mut Vec<u8>, bytes: &[u8]) {
    buffer.extend_from_slice(bytes);
    if buffer.len() > MAX_MATCH {
        buffer.drain(..buffer.len() - MAX_MATCH);
    }
}

pub async fn export_report(session: &SerialManager, path: &Path) -> Result<usize> {
    let (csv, count) = {
        let state = session.state.lock().await;
        let mut csv = "cycle,step,result\r\n".to_owned();
        for step in &state.job.report {
            use std::fmt::Write as _;
            write!(
                csv,
                "{},{},{}\r\n",
                step.cycle,
                step.step,
                quote(&step.result)
            )?;
        }
        (csv, state.job.report.len())
    };
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))?;
        file.write_all(csv.as_bytes())?;
        file.persist(path)?;
        Ok::<_, anyhow::Error>(count)
    })
    .await?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplyRule {
    name: String,
    trigger: MatchSpec,
    reply: Message,
    #[serde(default)]
    delay_ms: u64,
    #[serde(default = "enabled")]
    enabled: bool,
}

#[derive(Default)]
pub(super) struct Responder {
    rules: Vec<(String, Matcher, Vec<u8>, Duration)>,
    bytes: Vec<u8>,
    last: Option<Instant>,
    pub revision: u64,
}

pub(super) struct Reply {
    name: String,
    bytes: Vec<u8>,
    delay: Duration,
    revision: u64,
}

impl Responder {
    pub fn new(rules: &[ReplyRule]) -> Result<Self> {
        ensure!(rules.len() <= 32, "最多 32 条自动应答规则");
        let rules = rules
            .iter()
            .filter(|rule| rule.enabled)
            .map(|rule| {
                ensure!(
                    !rule.name.trim().is_empty()
                        && rule.name.len() <= 128
                        && rule.delay_ms <= 30_000,
                    "应答名称须为 1–128 字节，延迟最多 30000 毫秒"
                );
                let reply = rule.reply.prepare()?;
                ensure!(reply.len() <= 4096, "单条自动应答最多 4096 字节");
                Ok((
                    rule.name.clone(),
                    Matcher::new(&rule.trigger)?,
                    reply,
                    Duration::from_millis(rule.delay_ms),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            rules,
            ..Default::default()
        })
    }

    pub fn reset(&mut self) {
        self.bytes.clear();
        self.last = None;
    }

    pub fn receive(&mut self, bytes: &[u8]) -> Option<Reply> {
        if self.rules.is_empty() {
            return None;
        }
        append(&mut self.bytes, bytes);
        let (name, _, bytes, delay) = self
            .rules
            .iter()
            .find(|(_, matcher, _, _)| matcher.matches(&self.bytes))?;
        self.bytes.clear();
        let now = Instant::now();
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(100))
        {
            return None;
        }
        self.last = Some(now);
        Some(Reply {
            name: name.clone(),
            bytes: bytes.clone(),
            delay: *delay,
            revision: self.revision,
        })
    }
}

pub(super) fn queue_reply(
    state: Arc<Mutex<SerialState>>,
    tx: Arc<Mutex<()>>,
    reply: Reply,
    generation: u64,
    mut ended: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        select! {
            biased;
            _ = ended.wait_for(|value| *value) => return,
            _ = tokio::time::sleep(reply.delay) => {}
        }
        let gate = tx.try_lock();
        {
            let mut state = state.lock().await;
            if state.generation != generation
                || state.connection.is_none()
                || state.responder.revision != reply.revision
            {
                return;
            }
            if gate.is_err() {
                let message = format!("自动应答 {} 已跳过：发送通道忙", reply.name);
                let text = super::ReceiveDecoder::new().decode(message.as_bytes(), true);
                let mut frame = SerialFrameEvent::new(Direction::Send, Vec::new(), text);
                frame.source = "autoReply/skipped".into();
                state.push(frame);
                return;
            }
        }
        let _ = write_transport(&state, reply.bytes, &format!("autoReply/{}", reply.name)).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::{types::SerialConfig, workbench::execute};

    #[tokio::test]
    async fn macro_can_wait_for_a_delayed_virtual_reply_and_file_cancel_stops_at_chunk_boundary() {
        let a = Arc::new(SerialManager::new());
        let b = Arc::new(SerialManager::new());
        a.connect("VIRTUAL-A".into(), SerialConfig::default())
            .await
            .unwrap();
        b.connect("VIRTUAL-B".into(), SerialConfig::default())
            .await
            .unwrap();
        execute(&b, r#"{"action":"replies","rules":[{"name":"ping","trigger":{"kind":"hex","pattern":"CA FE"},"reply":{"text":"ok","format":"utf-8"},"delayMs":10}]}"#).await.unwrap();
        execute(&a, r#"{"action":"startJob","job":{"kind":"macro","cycles":1,"steps":[{"send":{"text":"CA FE","format":"hex"},"wait":{"kind":"text","pattern":"ok"},"timeoutMs":500}]}}"#).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let status = a.status().await;
                if status.job_kind.is_none() {
                    assert_eq!(status.job_done, 1, "{:?}", status.job_error);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            b.records(0, 20)
                .await
                .frames
                .iter()
                .any(|frame| frame.source == "autoReply/ping")
        );

        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"abcdefghijkl").unwrap();
        start(
            &a,
            Job::File {
                path: file.path().into(),
                chunk_bytes: 4,
                interval_ms: 3000,
            },
        )
        .await
        .unwrap();
        assert!(a.send_bytes(vec![1]).await.is_err());
        tokio::time::timeout(Duration::from_secs(1), async {
            while a.status().await.job_done == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        execute(&a, r#"{"action":"cancelJob"}"#).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while a.status().await.job_kind.is_some() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(a.status().await.job_done, 4);

        let repeat = |ms| {
            format!(
                r#"{{"action":"startJob","job":{{"kind":"repeat","message":{{"text":"42","format":"hex"}},"intervalMs":{ms}}}}}"#
            )
        };
        assert!(execute(&a, &repeat(9)).await.is_err());
        execute(&a, &repeat(3000)).await.unwrap();
        assert!(a.send_bytes(vec![1]).await.is_err());
        tokio::time::timeout(Duration::from_secs(1), async {
            while a.status().await.job_done == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            a.disconnect().await;
        })
        .await
        .unwrap();
        assert!(a.status().await.job_kind.is_none());
        assert_eq!(a.status().await.job_done, 1);
        a.close().await;
        b.close().await;
    }
}
