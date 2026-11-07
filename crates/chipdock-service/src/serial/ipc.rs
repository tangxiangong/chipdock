use super::{SerialError, Service, service};
use chipdock_control::{
    ApiError, Envelope, ErrorCode, Listener, MAX_PAGE, MAX_REQUEST, MAX_RESPONSE, Request,
    Response, SendMode, VERSION, read_message, write_message,
};
use chipdock_core::serial::{ConnectionError, SerialManager, types::*};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Semaphore, oneshot},
    task::JoinHandle,
};

const OFF: u8 = 0;
const READ_ONLY: u8 = 1;
const CONTROL: u8 = 2;
const LIMIT: usize = 16;

static MODE: AtomicU8 = AtomicU8::new(OFF);
static LISTENER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static PLOTS: Mutex<VecDeque<PlotAction>> = Mutex::new(VecDeque::new());
static NEXT_PLOT: AtomicU64 = AtomicU64::new(1);

struct PlotAction {
    id: u64,
    session: u64,
    claimed: bool,
    result: oneshot::Sender<Result<(), String>>,
}

pub fn configure(mode: u8) -> Result<(), SerialError> {
    if mode > CONTROL {
        return Err(anyhow::anyhow!("未知本地控制权限").into());
    }
    let service = service()?;
    let mut task = LISTENER
        .lock()
        .map_err(|_| anyhow::anyhow!("本地控制状态异常"))?;
    if task.as_ref().is_none_or(|task| task.is_finished()) {
        let listener = {
            let _runtime = service.runtime.enter();
            Listener::bind().map_err(|error| anyhow::anyhow!("无法启动本地控制：{error}"))?
        };
        *task = Some(service.runtime.spawn(serve(service, listener)));
    }
    MODE.store(mode, Ordering::Release);
    if mode != CONTROL
        && let Ok(mut actions) = PLOTS.lock()
    {
        actions.clear();
    }
    Ok(())
}

pub(super) fn stop() {
    MODE.store(OFF, Ordering::Release);
    if let Ok(mut task) = LISTENER.lock()
        && let Some(task) = task.take()
    {
        task.abort();
    }
    if let Ok(mut actions) = PLOTS.lock() {
        actions.clear();
    }
}

pub fn take_plot() -> Option<(u64, u64)> {
    let mut actions = PLOTS.lock().ok()?;
    if MODE.load(Ordering::Acquire) != CONTROL {
        return None;
    }
    let action = actions
        .iter_mut()
        .find(|action| !action.claimed && !action.result.is_closed())?;
    action.claimed = true;
    Some((action.id, action.session))
}

pub fn finish_plot(id: u64, failure: Option<String>) {
    if let Ok(mut actions) = PLOTS.lock()
        && let Some(index) = actions
            .iter()
            .position(|action| action.id == id && action.claimed)
        && let Some(action) = actions.remove(index)
    {
        let _ = action.result.send(failure.map_or(Ok(()), Err));
    }
}

async fn queue_plot(session: u64) -> Result<Value, ApiError> {
    let id = NEXT_PLOT.fetch_add(1, Ordering::Relaxed);
    let (result, completion) = oneshot::channel();
    {
        let mut actions = PLOTS.lock().map_err(|_| failure("曲线窗口队列异常"))?;
        actions.retain(|action| !action.result.is_closed());
        if MODE.load(Ordering::Acquire) != CONTROL {
            return Err(ApiError::new(
                ErrorCode::ReadOnly,
                "当前权限禁止打开曲线窗口",
            ));
        }
        if actions.len() >= LIMIT {
            return Err(ApiError::new(ErrorCode::Busy, "曲线窗口队列已满"));
        }
        actions.push_back(PlotAction {
            id,
            session,
            claimed: false,
            result,
        });
    }
    let completed = tokio::time::timeout(Duration::from_secs(4), completion).await;
    if let Ok(mut actions) = PLOTS.lock() {
        actions.retain(|action| action.id != id);
    }
    match completed {
        Ok(Ok(Ok(()))) => Ok(json!({ "sessionId": session, "opened": true })),
        Ok(Ok(Err(error))) => Err(failure(error)),
        Ok(Err(_)) => Err(ApiError::new(ErrorCode::ReadOnly, "曲线窗口请求被取消")),
        Err(_) => Err(ApiError::new(
            ErrorCode::OutcomeUnknown,
            "曲线窗口请求超时，结果未确认",
        )),
    }
}

async fn serve(service: &'static Service, mut listener: Listener) {
    let limit = Arc::new(Semaphore::new(LIMIT));
    loop {
        let Ok(stream) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(75)).await;
            continue;
        };
        if let Ok(permit) = Arc::clone(&limit).try_acquire_owned() {
            tokio::spawn(async move {
                let _permit = permit;
                let _ = serve_one(stream, service, &MODE).await;
            });
        }
    }
}

async fn serve_one(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    service: &Service,
    mode: &AtomicU8,
) -> io::Result<()> {
    let envelope: Envelope<Value> = tokio::time::timeout(
        Duration::from_secs(2),
        read_message(&mut stream, MAX_REQUEST),
    )
    .await??;
    let result = if envelope.version != VERSION {
        Err(ApiError::new(
            ErrorCode::VersionMismatch,
            format!("ChipDock IPC 需要版本 {VERSION}"),
        ))
    } else {
        match serde_json::from_value::<Request>(envelope.request) {
            Err(error) => Err(ApiError::new(ErrorCode::InvalidRequest, error.to_string())),
            Ok(request) => dispatch(service, mode.load(Ordering::Acquire), request).await,
        }
    };
    let response = Response::from_result(envelope.id, result);
    tokio::time::timeout(
        Duration::from_secs(2),
        write_message(&mut stream, &response, MAX_RESPONSE),
    )
    .await?
}

fn failure(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::OperationFailed, message)
}

fn operation(error: anyhow::Error) -> ApiError {
    let code = match error.downcast_ref::<ConnectionError>() {
        Some(ConnectionError::PortBusy(_)) => ErrorCode::PortBusy,
        Some(ConnectionError::Busy | ConnectionError::AlreadyConnected) => ErrorCode::Busy,
        Some(ConnectionError::NotConnected) => ErrorCode::NotConnected,
        None if error.is::<chipdock_core::serial::TransmissionError>() => ErrorCode::OutcomeUnknown,
        None => ErrorCode::OperationFailed,
    };
    ApiError::new(code, error.to_string())
}

fn manager(service: &Service, id: u64) -> Result<Arc<SerialManager>, ApiError> {
    service
        .sessions()
        .map_err(|error| failure(error.to_string()))?
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::new(ErrorCode::SessionNotFound, format!("串口会话不存在：{id}")))
}

async fn ports() -> Result<Vec<PortInfo>, ApiError> {
    tokio::task::spawn_blocking(SerialManager::list_ports)
        .await
        .map_err(|error| failure(error.to_string()))?
        .map_err(operation)
}

async fn dispatch(service: &Service, mode: u8, request: Request) -> Result<Value, ApiError> {
    if mode == OFF {
        return Err(ApiError::new(
            ErrorCode::Disabled,
            "ChipDock 本地控制已关闭",
        ));
    }
    if mode == READ_ONLY && request.mutates() {
        return Err(ApiError::new(
            ErrorCode::ReadOnly,
            "ChipDock 本地控制仅允许读取",
        ));
    }
    let value = match request {
        Request::Sessions => {
            let sessions: Vec<_> = service
                .sessions()
                .map_err(|error| failure(error.to_string()))?
                .iter()
                .map(|(&id, session)| (id, Arc::clone(session)))
                .collect();
            let mut result = Vec::new();
            for (id, session) in sessions {
                result.push(json!({ "sessionId": id, "status": session.status().await }));
            }
            json!({ "sessions": result })
        }
        Request::Ports => json!({ "ports": ports().await? }),
        Request::Status { session_id } => {
            json!({ "status": manager(service, session_id)?.status().await })
        }
        Request::Records {
            session_id,
            cursor,
            limit,
        } => {
            if limit == 0 || limit > MAX_PAGE {
                return Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    format!("记录页大小须为 1–{MAX_PAGE}"),
                ));
            }
            let mut page = manager(service, session_id)?.records(cursor, limit).await;
            // Bound JSON expansion for large records while keeping the next cursor usable.
            let mut bytes = 0;
            let mut keep = 0;
            for frame in &page.frames {
                bytes += serde_json::to_vec(frame)
                    .map_err(|error| failure(error.to_string()))?
                    .len();
                if bytes > MAX_RESPONSE - 65_536 && keep > 0 {
                    break;
                }
                keep += 1;
            }
            if keep < page.frames.len() {
                page.frames.truncate(keep);
                page.next_cursor = page.frames.last().map_or(cursor, |frame| frame.sequence);
                page.has_more = true;
            }
            json!({ "records": page })
        }
        Request::Protocols { session_id } => {
            json!({ "protocols": manager(service, session_id)?.protocols().await })
        }
        Request::Connect {
            session_id,
            path,
            config,
            reconnect,
        } => {
            let session = manager(service, session_id)?;
            config
                .validate()
                .map_err(|error| ApiError::new(ErrorCode::InvalidRequest, error.to_string()))?;
            if !matches!(path.as_str(), "VIRTUAL-A" | "VIRTUAL-B")
                && !ports().await?.iter().any(|port| port.path == path)
            {
                return Err(ApiError::new(
                    ErrorCode::PortNotFound,
                    format!("串口不存在：{path}"),
                ));
            }
            json!({ "status": session.connect_with_recovery(path, config, reconnect).await.map_err(operation)? })
        }
        Request::Disconnect { session_id } => {
            json!({ "status": manager(service, session_id)?.disconnect().await })
        }
        Request::Send {
            session_id,
            format,
            input,
        } => {
            let session = manager(service, session_id)?;
            if matches!(format, SendMode::Command) {
                json!({ "hex": session.send_command(&input, true).await.map_err(operation)? })
            } else {
                let format = if matches!(format, SendMode::Hex) {
                    SendFormat::Hex
                } else {
                    SendFormat::Utf8
                };
                session
                    .send_external(&input, format)
                    .await
                    .map_err(operation)?;
                json!({ "sent": true })
            }
        }
        Request::SelectProtocol { session_id, name } => {
            manager(service, session_id)?
                .select_protocol(name.clone())
                .await
                .map_err(|error| ApiError::new(ErrorCode::InvalidRequest, error.to_string()))?;
            json!({ "selected": name })
        }
        Request::OpenPlot { session_id } => {
            manager(service, session_id)?;
            queue_plot(session_id).await?
        }
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chipdock_control::{Outcome, exchange};

    #[test]
    fn local_transport_enforces_modes_and_version_without_hardware() {
        let service = Service::new().unwrap();
        service.runtime.block_on(async {
            for (mode, request, expected) in [
                (OFF, Request::Sessions, Some(ErrorCode::Disabled)),
                (
                    READ_ONLY,
                    Request::Disconnect { session_id: 1 },
                    Some(ErrorCode::ReadOnly),
                ),
                (READ_ONLY, Request::Status { session_id: 1 }, None),
                (
                    CONTROL,
                    Request::Status { session_id: 999 },
                    Some(ErrorCode::SessionNotFound),
                ),
            ] {
                let mode = AtomicU8::new(mode);
                let (mut client, server) = tokio::io::duplex(64 * 1024);
                let (received, served) = tokio::join!(
                    exchange(&mut client, request),
                    serve_one(server, &service, &mode)
                );
                served.unwrap();
                assert_eq!(received.err().map(|error| error.code), expected);
            }

            let (mut client, server) = tokio::io::duplex(4096);
            let mode = AtomicU8::new(CONTROL);
            let (reply, served) = tokio::join!(
                async {
                    let request = Envelope {
                        id: Default::default(),
                        version: VERSION + 1,
                        request: json!({"command": "future"}),
                    };
                    write_message(&mut client, &request, MAX_REQUEST)
                        .await
                        .unwrap();
                    read_message::<Response>(&mut client, MAX_RESPONSE)
                        .await
                        .unwrap()
                },
                serve_one(server, &service, &mode)
            );
            served.unwrap();
            assert!(matches!(
                reply.outcome,
                Outcome::Error(ApiError {
                    code: ErrorCode::VersionMismatch,
                    ..
                })
            ));
        });
    }
}
