use anyhow::{Result, anyhow, ensure};
use chrono::{SecondsFormat, Utc};
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use super::{Direction, SerialFrameEvent, bytes_to_hex};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Txt,
    Csv,
    Bin,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub path: PathBuf,
    pub format: Format,
    pub roll_bytes: u64,
}

#[derive(Default)]
pub(super) struct Capture {
    sender: Option<mpsc::SyncSender<Line>>,
    worker: Option<thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
    path: Option<PathBuf>,
}

pub(super) struct Line {
    pub direction: Direction,
    pub bytes: Vec<u8>,
    pub source: String,
    pub time: String,
}

impl Line {
    pub fn new(direction: Direction, bytes: &[u8], source: &str) -> Self {
        Self {
            direction,
            bytes: bytes.to_vec(),
            source: source.into(),
            time: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

impl Capture {
    pub fn start(settings: Settings) -> Result<Self> {
        ensure!(
            (1..=16 * 1024 * 1024 * 1024).contains(&settings.roll_bytes),
            "滚动大小超出范围"
        );
        let path = settings.path.clone();
        let writer = Rolling::new(settings)?;
        let (sender, receiver) = mpsc::sync_channel::<Line>(64);
        let error = Arc::new(Mutex::new(None));
        let worker_error = Arc::clone(&error);
        let worker = thread::Builder::new()
            .name("ChipDock serial capture".into())
            .spawn(move || {
                let mut writer = writer;
                let interval = Duration::from_secs(1);
                let mut flushed = Instant::now();
                loop {
                    let line =
                        match receiver.recv_timeout(interval.saturating_sub(flushed.elapsed())) {
                            Ok(line) => Some(line),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                    if let Some(line) = line
                        && let Err(error) = writer.write(&line)
                    {
                        set_error(
                            &worker_error,
                            format!(
                                "采集写入失败，已停止 {}。首个缺口为 {} 的 {} 字节：{error}",
                                writer.path.display(),
                                line.time,
                                line.bytes.len()
                            ),
                        );
                        return;
                    }
                    if flushed.elapsed() >= interval {
                        if let Err(error) = writer.file.flush() {
                            set_error(
                                &worker_error,
                                format!("采集刷新失败 {}：{error}", writer.path.display()),
                            );
                            return;
                        }
                        flushed = Instant::now();
                    }
                }
                if let Err(error) = writer.file.flush() {
                    set_error(
                        &worker_error,
                        format!("采集刷新失败 {}：{error}", writer.path.display()),
                    );
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            error,
            path: Some(path),
        })
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|error| error.clone())
    }

    pub fn active(&self) -> bool {
        self.sender.is_some() && self.error().is_none()
    }

    pub fn queue(&mut self, direction: Direction, bytes: &[u8], source: &str) {
        if let Some(sender) = &self.sender {
            match sender.try_send(Line::new(direction, bytes, source)) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(line)) => {
                    set_error(
                        &self.error,
                        format!(
                            "采集队列已满，已停止 {}。首个缺口为 {} 的 {} 字节",
                            self.path.as_deref().unwrap_or(Path::new("")).display(),
                            line.time,
                            line.bytes.len()
                        ),
                    );
                    self.sender = None;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => self.sender = None,
            }
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let result = tokio::task::spawn_blocking(move || {
                worker.join().map_err(|_| anyhow!("采集线程异常退出"))
            })
            .await;
            if let Err(error) = result
                .map_err(anyhow::Error::from)
                .and_then(|result| result)
            {
                set_error(&self.error, error.to_string());
            }
            if let Some(error) = self.error() {
                return Err(anyhow!(error));
            }
        }
        Ok(())
    }
}

fn set_error(slot: &Mutex<Option<String>>, message: String) {
    if let Ok(mut error) = slot.lock()
        && error.is_none()
    {
        *error = Some(message);
    }
}

struct Rolling {
    settings: Settings,
    path: PathBuf,
    part: u32,
    file: BufWriter<File>,
    bytes: u64,
    records: usize,
}

impl Rolling {
    fn new(settings: Settings) -> Result<Self> {
        let path = settings.path.clone();
        let file = new_file(&path)?;
        let mut writer = Self {
            settings,
            path,
            file,
            bytes: 0,
            records: 0,
            part: 0,
        };
        writer.header()?;
        Ok(writer)
    }

    fn header(&mut self) -> Result<()> {
        if matches!(self.settings.format, Format::Csv) {
            self.file.write_all(CSV_HEADER)?;
            self.bytes = CSV_HEADER.len() as u64;
        }
        Ok(())
    }

    fn write(&mut self, line: &Line) -> Result<()> {
        let data = format_line(
            self.settings.format,
            None,
            &line.time,
            line.direction,
            &line.source,
            &line.bytes,
            None,
        );
        if self.records != 0 && self.bytes + data.len() as u64 > self.settings.roll_bytes {
            self.file.flush()?;
            self.part += 1;
            let mut name = self.settings.path.as_os_str().to_owned();
            name.push(format!(".{:04}", self.part));
            self.path = name.into();
            self.file = new_file(&self.path)?;
            self.bytes = 0;
            self.records = 0;
            self.header()?;
        }
        self.file.write_all(&data)?;
        self.records += 1;
        self.bytes += data.len() as u64;
        Ok(())
    }
}

fn new_file(path: &Path) -> Result<BufWriter<File>> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(BufWriter::new)
        .map_err(|error| anyhow!("无法创建 {}：{error}", path.display()))
}

const CSV_HEADER: &[u8] = b"sequence,timestamp,direction,source,bytes,hex,text,checksum\r\n";

fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Receive => "RX",
        Direction::Send => "TX",
    }
}

pub(super) fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn format_line(
    format: Format,
    sequence: Option<u64>,
    time: &str,
    direction: Direction,
    source: &str,
    bytes: &[u8],
    checksum: Option<bool>,
) -> Vec<u8> {
    let direction = direction_name(direction);
    match format {
        Format::Bin => bytes.to_vec(),
        Format::Txt => {
            format!("{time}\t{direction}\t{source}\t{}\n", bytes_to_hex(bytes)).into_bytes()
        }
        Format::Csv => format!(
            "{},{time},{direction},{},{},{},{},{}\r\n",
            sequence.map(|n| n.to_string()).unwrap_or_default(),
            quote(source),
            bytes.len(),
            quote(&bytes_to_hex(bytes)),
            quote(&String::from_utf8_lossy(bytes)),
            checksum.map(|value| value.to_string()).unwrap_or_default()
        )
        .into_bytes(),
    }
}

pub(super) fn export(path: &Path, format: Format, records: Vec<SerialFrameEvent>) -> Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut output = BufWriter::new(file.as_file_mut());
        if matches!(format, Format::Csv) {
            output.write_all(CSV_HEADER)?;
        }
        for frame in records {
            let checks = frame.parsed.iter().filter_map(|entry| entry.checksum_valid);
            let checksum = checks
                .clone()
                .next()
                .map(|_| checks.clone().all(|valid| valid));
            output.write_all(&format_line(
                format,
                Some(frame.sequence),
                &frame.timestamp,
                frame.direction,
                &frame.source,
                &frame.bytes,
                checksum,
            ))?;
        }
        output.flush()?;
    }
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn small_capture_is_flushed_while_still_active() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.bin");
        let mut capture = Capture::start(Settings {
            path: path.clone(),
            format: Format::Bin,
            roll_bytes: 4096,
        })
        .unwrap();
        capture.queue(Direction::Receive, b"sample", "serial");
        tokio::time::timeout(Duration::from_secs(5), async {
            while std::fs::read(&path).unwrap() != b"sample" {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert!(capture.active());
        capture.stop().await.unwrap();
    }

    #[tokio::test]
    async fn rolling_raw_capture_preserves_bytes_across_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.bin");
        let mut capture = Capture::start(Settings {
            path: path.clone(),
            format: Format::Bin,
            roll_bytes: 3,
        })
        .unwrap();
        capture.queue(Direction::Receive, b"abc", "serial");
        capture.queue(Direction::Receive, b"\0\xff", "serial");
        capture.stop().await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        assert_eq!(
            std::fs::read(directory.path().join("capture.bin.0001")).unwrap(),
            b"\0\xff"
        );
        assert!(
            Capture::start(Settings {
                path,
                format: Format::Bin,
                roll_bytes: 3
            })
            .is_err()
        );
    }
}
