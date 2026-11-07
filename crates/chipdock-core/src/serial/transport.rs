use std::{
    collections::HashSet,
    future::poll_fn,
    pin::Pin,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow, bail};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::mpsc,
};
use tokio_serial::{SerialPort, SerialPortBuilder, SerialPortBuilderExt, SerialStream};

use super::types::{ControlLine, PortInfo, SerialConfig};

pub const VIRTUAL_PORTS: [&str; 2] = ["VIRTUAL-A", "VIRTUAL-B"];
const VIRTUAL_QUEUE: usize = 64;

#[derive(Clone)]
pub(super) enum Writer {
    Native(Arc<Mutex<Physical>>),
    Virtual(usize),
}

pub(super) enum Reader {
    Native(Arc<Mutex<Physical>>),
    Virtual(VirtualReader),
}

pub(super) struct Physical {
    stream: SerialStream,
    _port: PortLease,
}

struct PortLease(String);

static PORTS: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static VIRTUAL: Mutex<[Option<mpsc::Sender<Vec<u8>>>; 2]> = Mutex::new([None, None]);

fn locked<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl PortLease {
    fn acquire(path: &str) -> Result<Self> {
        let key = if cfg!(windows) {
            path.trim_start_matches(r"\\.\").to_ascii_uppercase()
        } else {
            let canonical = std::fs::canonicalize(path)
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok());
            let path = canonical.as_deref().unwrap_or(path);
            path.strip_prefix("/dev/tty.")
                .map_or_else(|| path.to_owned(), |name| format!("/dev/cu.{name}"))
        };
        if !locked(&PORTS)
            .get_or_insert_with(HashSet::new)
            .insert(key.clone())
        {
            return Err(super::ConnectionError::PortBusy(path.into()).into());
        }
        Ok(Self(key))
    }
}

impl Drop for PortLease {
    fn drop(&mut self) {
        if let Some(ports) = locked(&PORTS).as_mut() {
            ports.remove(&self.0);
        }
    }
}

pub(super) struct VirtualReader {
    index: usize,
    receiver: mpsc::Receiver<Vec<u8>>,
}

impl Drop for VirtualReader {
    fn drop(&mut self) {
        locked(&VIRTUAL)[self.index] = None;
    }
}

pub(super) fn virtual_info(path: &str) -> PortInfo {
    PortInfo {
        path: path.into(),
        display_name: path.into(),
        manufacturer: None,
        serial_number: None,
        vid: None,
        pid: None,
    }
}

pub(super) fn open(
    path: &str,
    builder: SerialPortBuilder,
    config: &SerialConfig,
) -> Result<(Reader, Writer)> {
    if let Some(index) = VIRTUAL_PORTS.iter().position(|port| *port == path) {
        let mut ports = locked(&VIRTUAL);
        if ports[index].is_some() {
            return Err(super::ConnectionError::PortBusy(path.into()).into());
        }
        let (sender, receiver) = mpsc::channel(VIRTUAL_QUEUE);
        ports[index] = Some(sender);
        return Ok((
            Reader::Virtual(VirtualReader { index, receiver }),
            Writer::Virtual(1 - index),
        ));
    }
    let lease = PortLease::acquire(path)?;
    let stream = builder
        .open_native_async()
        .map_err(|error| anyhow!("无法打开 {path}：{error}"))?;
    #[cfg(windows)]
    configure_windows(&stream, config)?;
    #[cfg(not(windows))]
    let _ = config;
    Ok(split(stream, lease))
}

fn split(stream: SerialStream, lease: PortLease) -> (Reader, Writer) {
    let port = Arc::new(Mutex::new(Physical {
        stream,
        _port: lease,
    }));
    (Reader::Native(Arc::clone(&port)), Writer::Native(port))
}

#[cfg(all(test, unix))]
pub(super) fn simulated(stream: SerialStream, path: &str) -> Result<(Reader, Writer)> {
    Ok(split(stream, PortLease::acquire(path)?))
}

impl Reader {
    pub async fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Native(port) => {
                let mut read = ReadBuf::new(buffer);
                poll_fn(|cx| Pin::new(&mut locked(port).stream).poll_read(cx, &mut read)).await?;
                Ok(read.filled().len())
            }
            Self::Virtual(port) => {
                let Some(bytes) = port.receiver.recv().await else {
                    return Ok(0);
                };
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
        }
    }
}

impl Writer {
    pub async fn write(&self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Native(port) => {
                poll_fn(|cx| Pin::new(&mut locked(port).stream).poll_write(cx, bytes)).await
            }
            Self::Virtual(peer) => {
                let sender = locked(&VIRTUAL)[*peer].clone();
                if let Some(sender) = sender {
                    // Closing the peer leaves this endpoint open, like disconnecting a cable.
                    let _ = sender.send(bytes.to_vec()).await;
                }
                Ok(bytes.len())
            }
        }
    }

    pub fn set_line(&self, line: ControlLine, enabled: bool) -> Result<()> {
        let Self::Native(port) = self else {
            bail!("虚拟串口没有硬件控制线")
        };
        let mut port = locked(port);
        match line {
            ControlLine::Rts => port.stream.write_request_to_send(enabled)?,
            ControlLine::Dtr => port.stream.write_data_terminal_ready(enabled)?,
        }
        Ok(())
    }
}

#[cfg(windows)]
fn configure_windows(stream: &SerialStream, config: &SerialConfig) -> Result<()> {
    use super::types::Parity;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Devices::Communication::*;

    if config.stop_bits != 15 && !matches!(config.parity, Parity::Mark | Parity::Space) {
        return Ok(());
    }
    let handle = stream.as_raw_handle();
    let mut settings = DCB {
        DCBlength: size_of::<DCB>() as u32,
        ..Default::default()
    };
    // The stream owns the live handle and the DCB buffer is initialized and sized above.
    if unsafe { GetCommState(handle, &mut settings) } == 0 {
        bail!("读取串口驱动参数失败：{}", std::io::Error::last_os_error());
    }
    if config.stop_bits == 15 {
        settings.StopBits = ONE5STOPBITS;
    }
    match config.parity {
        Parity::Mark | Parity::Space => {
            settings.Parity = if config.parity == Parity::Mark {
                MARKPARITY
            } else {
                SPACEPARITY
            };
            settings._bitfield |= 1 << 1; // fParity
        }
        _ => {}
    }
    if unsafe { SetCommState(handle, &settings) } == 0 {
        bail!(
            "驱动不支持所选的校验位或停止位：{}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}
