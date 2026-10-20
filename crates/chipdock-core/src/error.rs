use crate::models::{TargetCandidate, TargetInformation};
use serde::Serialize;
use std::{error::Error, path::Path};

type BoxedError = Box<dyn Error + Send + Sync + 'static>;

/// 应用内部统一错误结果类型。
pub type Result<T> = std::result::Result<T, AppError>;

/// 前端可稳定匹配的错误码。
#[cfg_attr(feature = "csharp", interoptopus::ffi)]
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
#[cfg_attr(feature = "csharp", interoptopus::ffi)]
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
                recovery: probe_not_found_recovery().to_string(),
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
                message: usb_issue(source.as_ref())
                    .map(|issue| issue.message().to_string())
                    .unwrap_or_else(|| self.to_string()),
                detail: Some(root_cause_message(source.as_ref())),
                target_candidates: Vec::new(),
                target_information: None,
                recovery: usb_issue(source.as_ref())
                    .map(UsbIssue::recovery)
                    .unwrap_or("确认芯片连接、接口、速度和访问地址后重试。")
                    .to_string(),
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

fn probe_not_found_recovery() -> &'static str {
    if cfg!(target_os = "macos") {
        "检查 USB 数据线后重新扫描。使用 Apple 芯片的 Mac 笔记本时，请先解锁 Mac，重新插拔烧录器并允许配件连接；可在系统设置的“隐私与安全性”中检查配件授权。关闭其他调试工具后重试。"
    } else if cfg!(target_os = "windows") {
        "检查 USB 数据线后重新扫描。USB 调试接口需要可用的 WinUSB 或 HID 驱动；串口型烧录器需安装对应的串口驱动。请关闭其他调试工具后重试。"
    } else {
        "接好烧录器后重新扫描，并检查当前用户的 USB 访问权限。"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UsbIssue {
    Permission,
    Busy,
    Disconnected,
    Driver,
    Unavailable,
}

impl UsbIssue {
    fn message(self) -> &'static str {
        match self {
            Self::Permission => "无法访问烧录器：USB 访问被拒绝",
            Self::Busy => "烧录器的 USB 接口正被占用",
            Self::Disconnected => "烧录器已断开连接",
            Self::Driver => "烧录器的 USB 接口或驱动不可用",
            Self::Unavailable => "无法打开烧录器的 USB 接口",
        }
    }

    fn recovery(self) -> &'static str {
        match self {
            Self::Disconnected => "检查 USB 数据线和供电，重新插拔烧录器后扫描并连接。",
            Self::Busy => {
                if cfg!(target_os = "macos") {
                    "关闭正在使用此烧录器的 IDE、OpenOCD、probe-rs、ST-LINK Server 或 J-Link 工具后重试。若仍被占用，可能有 USB/FTDI 内核驱动占用了调试接口；重新插拔设备并检查已安装的驱动。ChipDock 无法自动解除 macOS 内核驱动的占用。"
                } else {
                    "关闭正在使用此烧录器的 IDE、OpenOCD、probe-rs、ST-LINK Server 或 J-Link 工具，重新插拔设备后重试。"
                }
            }
            Self::Permission | Self::Unavailable if cfg!(target_os = "macos") => {
                "使用 Apple 芯片的 Mac 笔记本时，请解锁 Mac，重新插拔烧录器并在系统提示中允许配件连接；可在系统设置的“隐私与安全性”中检查配件授权。关闭其他调试工具。若仍失败，请检查是否有 USB/FTDI 驱动占用了调试接口。"
            }
            Self::Driver if cfg!(target_os = "macos") => {
                "此 USB 接口当前无法供 ChipDock 使用。关闭其他调试工具，检查是否有 USB/FTDI 内核驱动占用了调试接口，并确认烧录器固件及工作模式正确；通常无需额外安装 USB 驱动。"
            }
            Self::Permission | Self::Driver | Self::Unavailable if cfg!(target_os = "windows") => {
                "重新扫描并连接，允许 ChipDock 修复所选调试接口的 WinUSB 驱动。J-Link 也可通过 J-Link Configurator 切换为 WinUSB；无法自动识别的 USB 调试接口可用 Zadig 处理。串口型烧录器需对应的串口驱动。请保留 HID、串口和存储接口的原驱动，并关闭其他调试工具。"
            }
            _ => "检查烧录器的 USB 驱动和访问权限，关闭其他调试工具并重新插拔设备后重试。",
        }
    }
}

/// Classify typed USB errors anywhere in the source chain. File permission
/// failures must not be mistaken for USB authorization problems.
fn usb_issue(error: &(dyn Error + 'static)) -> Option<UsbIssue> {
    let mut current = Some(error);
    let mut usb_context = false;
    while let Some(error) = current {
        if matches!(
            error.downcast_ref::<probe_rs::probe::DebugProbeError>(),
            Some(probe_rs::probe::DebugProbeError::Usb(_))
        ) {
            usb_context = true;
        }
        if let Some(error) = error.downcast_ref::<probe_rs::probe::ProbeCreationError>() {
            match error {
                probe_rs::probe::ProbeCreationError::Usb(_) => usb_context = true,
                probe_rs::probe::ProbeCreationError::CouldNotOpen => {
                    return Some(UsbIssue::Unavailable);
                }
                _ => {}
            }
        }
        // io::Error may hide its contained error from source(), so inspect
        // get_ref() as well (nusb maps Busy to io::ErrorKind::Other).
        let io = error.downcast_ref::<std::io::Error>();
        let usb = error.downcast_ref::<nusb::Error>().or_else(|| {
            io.and_then(std::io::Error::get_ref)
                .and_then(|error| error.downcast_ref::<nusb::Error>())
        });
        if let Some(error) = usb {
            return match error.kind() {
                nusb::ErrorKind::Busy => Some(UsbIssue::Busy),
                nusb::ErrorKind::PermissionDenied => Some(UsbIssue::Permission),
                nusb::ErrorKind::Disconnected => Some(UsbIssue::Disconnected),
                nusb::ErrorKind::Unsupported => Some(UsbIssue::Driver),
                nusb::ErrorKind::NotFound => Some(UsbIssue::Unavailable),
                _ => None,
            };
        }
        if usb_context && let Some(error) = io {
            match error.kind() {
                std::io::ErrorKind::PermissionDenied => return Some(UsbIssue::Permission),
                std::io::ErrorKind::ResourceBusy => return Some(UsbIssue::Busy),
                std::io::ErrorKind::NotConnected => return Some(UsbIssue::Disconnected),
                std::io::ErrorKind::Unsupported => return Some(UsbIssue::Driver),
                _ => {}
            }
        }
        current = error.source();
    }
    None
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

#[cfg(test)]
mod usb_tests {
    use super::*;

    fn creation_error(kind: std::io::ErrorKind) -> AppError {
        AppError::operation(probe_rs::probe::ProbeCreationError::Usb(
            std::io::Error::new(kind, "USB root cause"),
        ))
    }

    #[test]
    fn usb_permission_error_should_explain_authorization_and_preserve_detail() {
        let response = creation_error(std::io::ErrorKind::PermissionDenied).to_response();
        assert_eq!(response.message, "无法访问烧录器：USB 访问被拒绝");
        assert_eq!(response.detail.as_deref(), Some("USB root cause"));
        if cfg!(target_os = "macos") {
            assert!(response.recovery.contains("解锁 Mac"));
            assert!(response.recovery.contains("允许配件连接"));
        }
    }

    #[test]
    fn busy_usb_interface_should_suggest_releasing_other_debug_tools() {
        let error = AppError::operation(probe_rs::probe::DebugProbeError::Usb(
            std::io::Error::new(std::io::ErrorKind::ResourceBusy, "interface busy"),
        ));
        let response = error.to_response();
        assert_eq!(response.message, "烧录器的 USB 接口正被占用");
        assert!(response.recovery.contains("关闭正在使用此烧录器"));
        if cfg!(target_os = "macos") {
            assert!(response.recovery.contains("内核驱动"));
        }
    }

    #[test]
    fn disconnected_probe_should_request_rescan_instead_of_driver_installation() {
        let response = creation_error(std::io::ErrorKind::NotConnected).to_response();
        assert_eq!(response.message, "烧录器已断开连接");
        assert!(!response.recovery.contains("驱动"));
    }

    #[test]
    fn unsupported_usb_driver_should_report_interface_recovery() {
        let response = creation_error(std::io::ErrorKind::Unsupported).to_response();
        assert_eq!(response.message, "烧录器的 USB 接口或驱动不可用");
    }

    #[test]
    fn file_permission_error_should_not_be_classified_as_usb_authorization() {
        let response = AppError::operation(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "firmware file denied",
        ))
        .to_response();
        assert_eq!(response.message, "操作失败");
        assert_eq!(response.detail.as_deref(), Some("firmware file denied"));
    }

    #[test]
    fn failed_probe_open_should_include_platform_specific_recovery() {
        let response =
            AppError::operation(probe_rs::probe::ProbeCreationError::CouldNotOpen).to_response();
        assert_eq!(response.message, "无法打开烧录器的 USB 接口");
        if cfg!(target_os = "macos") {
            assert!(response.recovery.contains("配件授权"));
        }
    }

    #[test]
    fn empty_scan_recovery_should_include_mac_accessory_authorization() {
        let response = AppError::ProbeNotFound.to_response();
        if cfg!(target_os = "macos") {
            assert!(response.recovery.contains("配件授权"));
        }
    }
}
