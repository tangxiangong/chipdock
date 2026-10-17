use crate::models::{TargetCandidate, TargetInformation};
use serde::Serialize;
use std::{error::Error, path::Path};

type BoxedError = Box<dyn Error + Send + Sync + 'static>;

/// 应用内部统一错误结果类型。
pub type Result<T> = std::result::Result<T, AppError>;

/// 前端可稳定匹配的错误码。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[derive(uniffi::Enum)]
pub enum ErrorCode {
    /// 未找到可用探针。
    ProbeNotFound,
    /// 目标芯片连接成功但识别失败。
    TargetIdentifyFailed,
    /// 固件文件扩展名或格式不受支持。
    UnsupportedFirmwareFormat,
    /// BIN 固件缺少基地址。
    MissingBinBaseAddress,
    /// 固件地址格式、对齐或范围无效。
    InvalidFirmwareAddress,
    /// probe-rs 底层操作失败。
    ProbeRsFailure,
    /// 文件系统读写失败。
    IoFailure,
    /// 用户输入未通过校验。
    InvalidUserInput,
    /// 本地配置、历史或日志存储失败。
    StorageFailure,
    /// 当前已有烧录或擦除任务在运行。
    JobAlreadyRunning,
}

/// 序列化给前端的统一错误响应。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct ErrorResponse {
    /// 稳定错误码，供前端分支处理。
    pub code: ErrorCode,
    /// 用户可见的主错误信息。
    pub message: String,
    /// 供诊断日志记录的原始错误；用户提示只展示 `message`。
    pub detail: Option<String>,
    /// 自动识别失败时按当前硬件信息缩小出的候选目标。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub target_candidates: Vec<TargetCandidate>,
    /// 已通过调试接口读取到的目标硬件信息。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_information: Option<TargetInformation>,
    /// 用户可见的恢复建议。
    pub recovery: String,
}

/// 应用内部错误类型。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 未找到可用探针。
    #[error("未找到烧录器")]
    ProbeNotFound,
    /// 目标芯片识别失败。
    #[error("无法识别目标芯片")]
    TargetIdentifyFailed {
        detail: String,
        candidates: Vec<TargetCandidate>,
        target_information: Option<TargetInformation>,
    },
    /// 固件格式不受支持。
    #[error("不支持的固件格式")]
    UnsupportedFirmwareFormat { path: String },
    /// BIN 固件缺少基地址。
    #[error("BIN 文件必须填写基地址")]
    MissingBinBaseAddress,
    /// 固件地址无效。
    #[error("固件地址无效")]
    InvalidFirmwareAddress { detail: String },
    /// 底层设备操作失败，并保留可追溯的原始错误链。
    #[error("操作失败")]
    ProbeRsFailure {
        #[source]
        source: BoxedError,
    },
    /// 文件系统读写失败。
    #[error("文件读写失败")]
    IoFailure(#[from] std::io::Error),
    /// 用户输入无效。
    #[error("用户输入无效")]
    InvalidUserInput { detail: String },
    /// 本地配置、历史或日志存储失败。
    #[error("本地配置存储失败")]
    StorageFailure { detail: String },
    /// 当前已有异步任务在运行。
    #[error("已有任务正在运行")]
    JobAlreadyRunning,
}

impl AppError {
    /// 包装底层操作错误，保留其 `source()` 链直到响应序列化边界。
    pub fn operation(error: impl Error + Send + Sync + 'static) -> Self {
        Self::ProbeRsFailure {
            source: Box::new(error),
        }
    }

    /// 转换为前端可安全展示的错误响应。
    pub fn to_response(&self) -> ErrorResponse {
        match self {
            Self::ProbeNotFound => ErrorResponse {
                code: ErrorCode::ProbeNotFound,
                message: self.to_string(),
                detail: None,
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "接好烧录器后重新扫描。".to_string(),
            },
            Self::TargetIdentifyFailed {
                detail,
                candidates,
                target_information,
            } => ErrorResponse {
                code: ErrorCode::TargetIdentifyFailed,
                message: self.to_string(),
                detail: Some(detail.clone()),
                target_candidates: candidates.clone(),
                target_information: target_information.clone(),
                recovery: "从缩小后的候选目标中选择一个后重试连接。".to_string(),
            },
            Self::UnsupportedFirmwareFormat { path } => ErrorResponse {
                code: ErrorCode::UnsupportedFirmwareFormat,
                message: self.to_string(),
                detail: Some(format!("文件名：{}", display_file_name(path))),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "选择 .elf、.hex 或 .bin 文件。".to_string(),
            },
            Self::MissingBinBaseAddress => ErrorResponse {
                code: ErrorCode::MissingBinBaseAddress,
                message: self.to_string(),
                detail: None,
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "为 BIN 文件填写十六进制基地址，例如 0x08000000。".to_string(),
            },
            Self::InvalidFirmwareAddress { detail } => ErrorResponse {
                code: ErrorCode::InvalidFirmwareAddress,
                message: self.to_string(),
                detail: Some(detail.clone()),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "检查地址格式、对齐和目标芯片 Flash 映射。".to_string(),
            },
            Self::ProbeRsFailure { source } => ErrorResponse {
                code: ErrorCode::ProbeRsFailure,
                message: self.to_string(),
                detail: Some(root_cause_message(source.as_ref())),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "确认芯片连接、接口、速度和访问地址后重试。".to_string(),
            },
            Self::IoFailure(err) => ErrorResponse {
                code: ErrorCode::IoFailure,
                message: self.to_string(),
                detail: Some(format!("文件系统错误：{}", err.kind())),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "确认文件存在且当前用户有读写权限。".to_string(),
            },
            Self::InvalidUserInput { detail } => ErrorResponse {
                code: ErrorCode::InvalidUserInput,
                message: self.to_string(),
                detail: Some(detail.clone()),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "按字段提示修正输入后重试。".to_string(),
            },
            Self::StorageFailure { detail } => ErrorResponse {
                code: ErrorCode::StorageFailure,
                message: self.to_string(),
                detail: Some(detail.clone()),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "检查应用数据目录权限，或删除损坏的本地配置文件。".to_string(),
            },
            Self::JobAlreadyRunning => ErrorResponse {
                code: ErrorCode::JobAlreadyRunning,
                message: self.to_string(),
                detail: None,
                target_candidates: Vec::new(),
                target_information: None,
                recovery: "等待当前任务结束后重试。".to_string(),
            },
        }
    }
}

fn root_cause_message(error: &(dyn Error + 'static)) -> String {
    let mut root_cause = error;
    while let Some(source) = root_cause.source() {
        root_cause = source;
    }
    root_cause.to_string()
}

/// 展开错误来源链，得到可直接在当前进程内展示的完整错误描述。
///
/// probe-rs 的顶层错误类型（如 `FileDownloadError::Flash`）常常只是一层包装，
/// 真正有诊断价值的原因在 `source()` 链的更深处，仅调用 `to_string()` 会丢失这部分信息。
pub fn describe_error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![error.to_string()];
    let mut current = error;
    while let Some(source) = current.source() {
        parts.push(source.to_string());
        current = source;
    }
    parts.join("：")
}

fn display_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "所选文件".to_string())
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_response().serialize(serializer)
    }
}
