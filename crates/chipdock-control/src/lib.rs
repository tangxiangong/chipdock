use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{io, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

pub use chipdock_core::serial::types::{FlowControl, Parity, SerialConfig};

#[cfg(unix)]
#[path = "unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

pub use platform::Listener;
pub const VERSION: u32 = 1;
pub const MAX_REQUEST: usize = 1024 * 1024;
pub const MAX_RESPONSE: usize = 16 * 1024 * 1024;
pub const MAX_PAGE: u32 = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const CALL_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Sessions,
    Ports,
    Status {
        session_id: u64,
    },
    Records {
        session_id: u64,
        #[serde(default)]
        cursor: u64,
        #[serde(default = "default_limit")]
        limit: u32,
    },
    Protocols {
        session_id: u64,
    },
    Connect {
        session_id: u64,
        path: String,
        #[serde(default)]
        config: SerialConfig,
        #[serde(default)]
        reconnect: bool,
    },
    Disconnect {
        session_id: u64,
    },
    Send {
        session_id: u64,
        format: SendMode,
        input: String,
    },
    SelectProtocol {
        session_id: u64,
        name: Option<String>,
    },
    OpenPlot {
        session_id: u64,
    },
}

const fn default_limit() -> u32 {
    MAX_PAGE
}

impl Request {
    pub fn mutates(&self) -> bool {
        matches!(
            self,
            Self::Connect { .. }
                | Self::Disconnect { .. }
                | Self::Send { .. }
                | Self::SelectProtocol { .. }
                | Self::OpenPlot { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendMode {
    Text,
    Hex,
    Command,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub version: u32,
    pub id: Uuid,
    pub request: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub version: u32,
    pub id: Uuid,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Result(Value),
    Error(ApiError),
}

impl Response {
    pub fn from_result(id: Uuid, result: Result<Value, ApiError>) -> Self {
        Self {
            version: VERSION,
            id,
            outcome: match result {
                Ok(value) => Outcome::Result(value),
                Err(error) => Outcome::Error(error),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unavailable,
    Disabled,
    ReadOnly,
    SessionNotFound,
    PortNotFound,
    PortBusy,
    Busy,
    NotConnected,
    InvalidRequest,
    VersionMismatch,
    Timeout,
    OutcomeUnknown,
    OperationFailed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

pub async fn read_message<T: DeserializeOwned>(
    stream: &mut (impl AsyncRead + Unpin),
    max: usize,
) -> io::Result<T> {
    let len = stream.read_u32().await? as usize;
    if len == 0 || len > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IPC message length is invalid",
        ));
    }
    let mut data = vec![0; len];
    stream.read_exact(&mut data).await?;
    serde_json::from_slice(&data).map_err(io::Error::other)
}

pub async fn write_message<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
    max: usize,
) -> io::Result<()> {
    let data = serde_json::to_vec(value).map_err(io::Error::other)?;
    if data.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IPC message exceeds the limit",
        ));
    }
    stream.write_u32(data.len() as u32).await?;
    stream.write_all(&data).await?;
    stream.flush().await
}

pub async fn call(request: Request) -> Result<Value, ApiError> {
    let mut stream = tokio::time::timeout(CONNECT_TIMEOUT, platform::connect())
        .await
        .map_err(|_| ApiError::new(ErrorCode::Unavailable, "连接 ChipDock 超时"))?
        .map_err(|error| {
            ApiError::new(
                ErrorCode::Unavailable,
                format!("无法连接 ChipDock：{error}"),
            )
        })?;
    exchange(&mut stream, request).await
}

pub async fn exchange(
    stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    request: Request,
) -> Result<Value, ApiError> {
    let mutated = request.mutates();
    let envelope = Envelope {
        version: VERSION,
        id: Uuid::new_v4(),
        request,
    };
    if serde_json::to_vec(&envelope).map_or(true, |value| value.len() > MAX_REQUEST) {
        return Err(ApiError::new(ErrorCode::InvalidRequest, "本地控制请求过大"));
    }
    let result = tokio::time::timeout(CALL_TIMEOUT, async {
        write_message(stream, &envelope, MAX_REQUEST).await?;
        read_message::<Response>(stream, MAX_RESPONSE).await
    })
    .await;
    let response = match result {
        Ok(Ok(response)) => response,
        Ok(Err(error)) if mutated => {
            return Err(ApiError::new(
                ErrorCode::OutcomeUnknown,
                format!("IPC 已中断，操作结果未确认：{error}"),
            ));
        }
        Ok(Err(error)) => {
            return Err(ApiError::new(
                ErrorCode::Unavailable,
                format!("无法读取 ChipDock 响应：{error}"),
            ));
        }
        Err(_) if mutated => {
            return Err(ApiError::new(
                ErrorCode::OutcomeUnknown,
                "操作超时，结果未确认。请查询状态和记录",
            ));
        }
        Err(_) => return Err(ApiError::new(ErrorCode::Timeout, "本地控制请求超时")),
    };
    if response.version != VERSION || response.id != envelope.id {
        return Err(ApiError::new(
            ErrorCode::VersionMismatch,
            "ChipDock IPC 版本或响应标识不匹配",
        ));
    }
    match response.outcome {
        Outcome::Result(value) => Ok(value),
        Outcome::Error(error) => Err(error),
    }
}
