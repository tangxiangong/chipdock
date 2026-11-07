//! Process-wide serial sessions, independent of the probe worker.
use chipdock_core::serial::{PollBatch, SerialManager, types::*};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::runtime::{Builder, Runtime};

pub mod ipc;

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

pub const DEFAULT_SESSION: u64 = 1;
pub const MAX_SESSIONS: usize = 16;

type Sessions = Arc<Mutex<BTreeMap<u64, Arc<SerialManager>>>>;

struct Service {
    runtime: Runtime,
    sessions: Sessions,
    next_id: AtomicU64,
}

impl Service {
    fn new() -> std::io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let sessions = Arc::new(Mutex::new(BTreeMap::from([(
            DEFAULT_SESSION,
            Arc::new(SerialManager::new()),
        )])));
        let recovery = Arc::clone(&sessions);
        runtime.spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(2));
            loop {
                timer.tick().await;
                let current: Vec<_> = recovery
                    .lock()
                    .map(|sessions| sessions.values().cloned().collect())
                    .unwrap_or_default();
                for session in current {
                    tokio::spawn(async move { session.reconnect_if_available().await });
                }
            }
        });
        Ok(Self {
            runtime,
            sessions,
            next_id: AtomicU64::new(DEFAULT_SESSION + 1),
        })
    }

    fn sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<u64, Arc<SerialManager>>>, SerialError> {
        self.sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("串口核心状态异常，请重新启动应用").into())
    }

    fn session(&self, id: u64) -> Result<Arc<SerialManager>, SerialError> {
        self.sessions()?
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("串口会话不存在：{id}").into())
    }
}

static SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();

fn service() -> Result<&'static Service, SerialError> {
    SERVICE
        .get_or_init(|| Service::new().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|message| SerialError::Operation {
            message: message.clone(),
        })
}

pub fn serial_create_session() -> Result<u64, SerialError> {
    let service = service()?;
    let mut sessions = service.sessions()?;
    if sessions.len() >= MAX_SESSIONS {
        return Err(anyhow::anyhow!("最多打开 {MAX_SESSIONS} 个串口会话").into());
    }
    let id = service.next_id.fetch_add(1, Ordering::Relaxed);
    sessions.insert(id, Arc::new(SerialManager::new()));
    Ok(id)
}

pub fn serial_close_session(id: u64) -> Result<(), SerialError> {
    if id == DEFAULT_SESSION {
        serial_session_disconnect(id)?;
        return Ok(());
    }
    let service = service()?;
    let session = service.sessions()?.remove(&id);
    if let Some(session) = session {
        service.runtime.block_on(session.close());
    }
    Ok(())
}

pub fn serial_shutdown() -> Result<(), SerialError> {
    ipc::stop();
    let service = service()?;
    let sessions = std::mem::take(&mut *service.sessions()?);
    service.runtime.block_on(async {
        for session in sessions.values() {
            session.close().await;
        }
    });
    Ok(())
}

pub fn serial_is_busy() -> Result<bool, SerialError> {
    let service = service()?;
    let sessions: Vec<_> = service.sessions()?.values().cloned().collect();
    Ok(service.runtime.block_on(async {
        for session in sessions {
            if session.busy().await {
                return true;
            }
        }
        false
    }))
}

pub fn serial_ports() -> Result<Vec<PortInfo>, SerialError> {
    SerialManager::list_ports().map_err(Into::into)
}

pub fn serial_session_records(id: u64, cursor: u64, limit: u32) -> Result<PollBatch, SerialError> {
    let service = service()?;
    Ok(service
        .runtime
        .block_on(service.session(id)?.records(cursor, limit)))
}

pub fn serial_session_poll(id: u64) -> Result<PollBatch, SerialError> {
    let service = service()?;
    Ok(service.runtime.block_on(service.session(id)?.poll()))
}

pub fn serial_session_connect(
    id: u64,
    path: String,
    config: SerialConfig,
    reconnect: bool,
) -> Result<SerialStatus, SerialError> {
    let service = service()?;
    service
        .runtime
        .block_on(
            service
                .session(id)?
                .connect_with_recovery(path, config, reconnect),
        )
        .map_err(Into::into)
}

pub fn serial_session_disconnect(id: u64) -> Result<SerialStatus, SerialError> {
    let service = service()?;
    Ok(service.runtime.block_on(service.session(id)?.disconnect()))
}

pub fn serial_session_set_line(
    id: u64,
    line: ControlLine,
    enabled: bool,
) -> Result<SerialStatus, SerialError> {
    let service = service()?;
    service
        .runtime
        .block_on(service.session(id)?.set_line(line, enabled))
        .map_err(Into::into)
}

pub fn serial_session_send(
    id: u64,
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
    checksum: ChecksumAlgorithm,
) -> Result<(), SerialError> {
    let service = service()?;
    service
        .runtime
        .block_on(
            service
                .session(id)?
                .send_with_checksum(input, format, line_ending, checksum),
        )
        .map_err(Into::into)
}

pub fn serial_session_send_options(
    id: u64,
    input: String,
    format: SendFormat,
    ending: LineEnding,
    checksum: ChecksumAlgorithm,
    options: SendOptions,
) -> Result<(), SerialError> {
    let service = service()?;
    service
        .runtime
        .block_on(
            service
                .session(id)?
                .send_options(&input, format, ending, checksum, options),
        )
        .map_err(Into::into)
}

pub fn serial_workbench(id: u64, request: &str) -> Result<String, SerialError> {
    let service = service()?;
    let session = service.session(id)?;
    service
        .runtime
        .block_on(chipdock_core::serial::workbench::execute(&session, request))
        .map_err(Into::into)
}

pub fn serial_poll() -> Result<PollBatch, SerialError> {
    serial_session_poll(DEFAULT_SESSION)
}

pub fn serial_connect(path: String, config: SerialConfig) -> Result<SerialStatus, SerialError> {
    serial_session_connect(DEFAULT_SESSION, path, config, false)
}

pub fn serial_disconnect() -> Result<SerialStatus, SerialError> {
    serial_session_disconnect(DEFAULT_SESSION)
}

pub fn serial_send(
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
) -> Result<(), SerialError> {
    serial_send_with_checksum(input, format, line_ending, ChecksumAlgorithm::None)
}

pub fn serial_send_with_checksum(
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
    checksum: ChecksumAlgorithm,
) -> Result<(), SerialError> {
    serial_session_send(DEFAULT_SESSION, input, format, line_ending, checksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_pair_is_exclusive_and_sessions_disconnect_independently() {
        let a = serial_create_session().unwrap();
        let b = serial_create_session().unwrap();
        let other = serial_create_session().unwrap();
        serial_session_connect(a, "VIRTUAL-A".into(), SerialConfig::default(), false).unwrap();
        serial_session_connect(b, "VIRTUAL-B".into(), SerialConfig::default(), false).unwrap();
        assert!(
            serial_session_connect(other, "VIRTUAL-A".into(), SerialConfig::default(), false)
                .is_err()
        );

        for (from, to) in [(a, b), (b, a)] {
            serial_session_send(
                from,
                "00 ff 23".into(),
                SendFormat::Hex,
                LineEnding::None,
                ChecksumAlgorithm::None,
            )
            .unwrap();
            let mut bytes = Vec::new();
            for _ in 0..50 {
                bytes.extend(
                    serial_session_poll(to)
                        .unwrap()
                        .frames
                        .into_iter()
                        .filter(|frame| {
                            matches!(frame.direction, chipdock_core::serial::Direction::Receive)
                        })
                        .flat_map(|frame| frame.bytes),
                );
                if bytes.len() >= 3 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(bytes, [0, 255, 35]);
        }

        serial_close_session(a).unwrap();
        assert!(serial_session_poll(a).is_err());
        assert!(serial_session_poll(b).unwrap().status.connected);
        assert!(
            serial_session_connect(other, "VIRTUAL-A".into(), SerialConfig::default(), false)
                .is_ok()
        );
        serial_close_session(other).unwrap();
        serial_close_session(b).unwrap();
    }
}
