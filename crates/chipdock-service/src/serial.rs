//! Process-wide serial transport, independent of the probe worker thread.
use chipdock_core::serial::{PollBatch, SerialManager, types::*};
use std::sync::{Mutex, OnceLock};
use tokio::runtime::{Builder, Runtime};

#[derive(Debug, thiserror::Error)]
pub enum SerialError {
    #[error("{message}")]
    Operation { message: String },
}
impl From<anyhow::Error> for SerialError {
    fn from(error: anyhow::Error) -> Self {
        Self::Operation {
            message: error.to_string(),
        }
    }
}
struct Service {
    runtime: Runtime,
    serial: SerialManager,
    calls: Mutex<()>,
}
static SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
fn service() -> Result<&'static Service, SerialError> {
    SERVICE
        .get_or_init(|| {
            Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map(|runtime| Service {
                    runtime,
                    serial: SerialManager::new(),
                    calls: Mutex::new(()),
                })
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|message| SerialError::Operation {
            message: message.clone(),
        })
}
fn perform<T>(operation: impl FnOnce(&Service) -> anyhow::Result<T>) -> Result<T, SerialError> {
    let service = service()?;
    let _guard = service.calls.lock().map_err(|_| SerialError::Operation {
        message: "串口核心状态异常，请重新启动应用".into(),
    })?;
    operation(service).map_err(Into::into)
}

pub fn serial_ports() -> Result<Vec<PortInfo>, SerialError> {
    SerialManager::list_ports().map_err(Into::into)
}

pub fn serial_poll() -> Result<PollBatch, SerialError> {
    perform(|s| Ok(s.runtime.block_on(s.serial.poll())))
}

pub fn serial_connect(path: String, config: SerialConfig) -> Result<SerialStatus, SerialError> {
    perform(|s| s.runtime.block_on(s.serial.connect(path, config)))
}

pub fn serial_disconnect() -> Result<SerialStatus, SerialError> {
    perform(|s| Ok(s.runtime.block_on(s.serial.disconnect())))
}

pub fn serial_send(
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
) -> Result<(), SerialError> {
    perform(|s| {
        s.runtime
            .block_on(s.serial.send(input, format, line_ending))
    })
}

pub fn serial_send_with_checksum(
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
    checksum: ChecksumAlgorithm,
) -> Result<(), SerialError> {
    perform(|s| {
        s.runtime.block_on(
            s.serial
                .send_with_checksum(input, format, line_ending, checksum),
        )
    })
}
