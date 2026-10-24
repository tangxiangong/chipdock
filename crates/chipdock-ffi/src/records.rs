//! UniFFI registrations for shared types; the service owns their definitions.

pub use chipdock_core::{
    error::{ErrorCode, ErrorResponse},
    models::*,
    serial::{Direction, PollBatch, SerialFrameEvent, encoding::DecodedText, types::*},
};
pub use chipdock_service::{
    records::{CoreError, ProgressEvent, RecentFirmware, SavedProfile},
    serial::SerialError,
    timestamp::AppTimestamp,
};

uniffi::custom_type!(AppTimestamp, f64, { remote });

#[uniffi::remote(Enum)]
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

#[uniffi::remote(Record)]
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

#[uniffi::remote(Enum)]
pub enum FirmwareFormat {
    /// ELF 可执行文件，地址信息来自文件内容。
    Elf,
    /// Intel HEX 文件，地址信息来自文件内容。
    Hex,
    /// 原始二进制文件，需要调用方提供烧录基地址。
    Bin,
}

#[uniffi::remote(Record)]
pub struct FirmwareInput {
    /// 固件文件的本地路径。
    pub path: String,
    /// 用户显式选择的格式；为空时根据文件扩展名推断。
    pub format: Option<FirmwareFormat>,
    /// BIN 固件的烧录基地址。
    pub base_address: Option<u64>,
}

#[uniffi::remote(Record)]
pub struct FirmwareUsageSegment {
    /// 起始写入地址。
    pub start: u64,
    /// 结束地址，开区间。
    pub end: u64,
    /// 该连续地址段的字节数。
    pub size: u64,
}

#[uniffi::remote(Record)]
pub struct FirmwareUsage {
    /// 解析采用的固件格式。
    pub format: FirmwareFormat,
    /// 去重合并后的实际写入字节数。
    pub used_bytes: u64,
    /// 从最小写入地址到最大写入地址的覆盖跨度，包含空洞。
    pub span_bytes: u64,
    /// 最小写入地址。
    pub start_address: Option<u64>,
    /// 最大写入地址，开区间。
    pub end_address: Option<u64>,
    /// 合并后的连续写入地址段。
    pub segments: Vec<FirmwareUsageSegment>,
}

#[uniffi::remote(Record)]
pub struct FirmwarePreview {
    /// 与预览对应的固件占用信息。
    pub usage: FirmwareUsage,
    /// 预览的第一个地址。
    pub start_address: u64,
    /// 逐地址的文件数据；None 表示固件未写入的空洞。
    pub bytes: Vec<Option<u8>>,
}

#[uniffi::remote(Record)]
pub struct ProbeSummary {
    /// probe-rs 可解析的探针选择器字符串。
    pub identifier: String,
    /// USB vendor id。
    pub vendor_id: u16,
    /// USB product id。
    pub product_id: u16,
    /// 探针序列号。
    pub serial_number: Option<String>,
    /// 探针产品名。
    pub product: Option<String>,
}

#[uniffi::remote(Record)]
pub struct TargetSelection {
    /// 目标芯片型号；为空时尝试自动识别。
    pub chip: Option<String>,
    /// 调试线协议。
    pub protocol: WireProtocol,
    /// 连接速度，单位 kHz。
    pub speed_khz: Option<u32>,
    /// 是否使用 connect-under-reset。
    pub connect_under_reset: bool,
}

#[uniffi::remote(Enum)]
pub enum WireProtocol {
    /// Serial Wire Debug。
    Swd,
    /// JTAG。
    Jtag,
}

#[uniffi::remote(Record)]
pub struct FlashOptions {
    /// 写入后逐字节校验。
    pub verify: bool,
    /// 只执行预检查，不修改芯片内容。
    pub dry_run: bool,
    /// 跳过自动擦除。
    pub skip_erase: bool,
    /// 允许整片擦除。
    pub allow_erase_all: bool,
    /// 写入完成后复位目标。
    pub reset_after: bool,
}

#[uniffi::remote(Record)]
pub struct FlashRequest {
    /// 待烧录固件。
    pub firmware: FirmwareInput,
    /// 选中的探针；为空时要求当前只连接一个探针。
    pub probe: Option<String>,
    /// 目标芯片和连接参数。
    pub target: TargetSelection,
    /// 烧录选项。
    pub options: FlashOptions,
}

#[uniffi::remote(Record)]
pub struct EraseRequest {
    /// 选中的探针；为空时要求当前只连接一个探针。
    pub probe: Option<String>,
    /// 目标芯片和连接参数。
    pub target: TargetSelection,
}

#[uniffi::remote(Record)]
pub struct ConnectRequest {
    /// 选中的探针；为空时要求当前只连接一个探针。
    pub probe: Option<String>,
    /// 目标芯片和连接参数。
    pub target: TargetSelection,
}

#[uniffi::remote(Record)]
pub struct TargetInformation {
    /// 从调试签名识别出的设备型号或系列。
    pub device_type: String,
    /// 厂商调试模块中的设备 ID。
    pub device_id: Option<u16>,
    /// 厂商调试模块中的芯片修订 ID。
    pub revision_id: Option<u16>,
    /// 从 CPUID 识别出的 CPU 内核名称。
    pub cpu: Option<String>,
    /// 从厂商系统存储区读出的 Flash 容量，单位字节。
    pub flash_size_bytes: Option<u64>,
}

#[uniffi::remote(Record)]
pub struct ConnectionInfo {
    /// 实际使用的探针选择器字符串。
    pub probe: String,
    /// 实际连接的芯片型号。
    pub chip: String,
    /// 实际使用的线协议。
    pub protocol: WireProtocol,
    /// 实际连接速度，单位 kHz。
    pub speed_khz: Option<u32>,
    /// 是否使用 connect-under-reset。
    pub connect_under_reset: bool,
    /// 从目标寄存器读取的硬件信息。
    pub target_information: Option<TargetInformation>,
}

#[uniffi::remote(Record)]
pub struct TargetCandidate {
    /// probe-rs 可接收的目标名称。
    pub name: String,
    /// 候选所属芯片族。
    pub family: String,
}

#[uniffi::remote(Record)]
pub struct MemoryRequest {
    /// 选中的探针；为空时要求当前只连接一个探针。
    pub probe: Option<String>,
    /// 目标芯片和连接参数。
    pub target: TargetSelection,
    /// 起始地址。
    pub address: u64,
    /// 读取长度，单位字节。
    pub length: u32,
}

#[uniffi::remote(Record)]
pub struct MemoryReadResult {
    /// 实际读取起始地址。
    pub address: u64,
    /// 实际读取长度，单位字节。
    pub length: u32,
    /// 小写十六进制连续字节字符串。
    pub data_hex: String,
}

#[uniffi::remote(Enum)]
pub enum MemoryRegionKind {
    /// 非易失存储，通常是 Flash。
    Nvm,
    /// RAM 区域。
    Ram,
    /// probe-rs 未归类的区域。
    Generic,
}

#[uniffi::remote(Record)]
pub struct MemoryAccessInfo {
    /// 是否可读。
    pub read: bool,
    /// 是否可写。
    pub write: bool,
    /// 是否可执行。
    pub execute: bool,
    /// 是否可作为启动区域。
    pub boot: bool,
}

#[uniffi::remote(Record)]
pub struct MemoryRegionLayout {
    /// 区域名称。
    pub name: Option<String>,
    /// 区域类型。
    pub kind: MemoryRegionKind,
    /// 起始地址。
    pub start: u64,
    /// 结束地址。
    pub end: u64,
    /// 区域大小，单位字节。
    pub size: u64,
    /// 关联的核心名称。
    pub cores: Vec<String>,
    /// 是否是其他区域的别名。
    pub is_alias: bool,
    /// 访问权限。
    pub access: MemoryAccessInfo,
}

#[uniffi::remote(Enum)]
pub enum JobKind {
    /// 固件烧录任务。
    Flash,
    /// 整片擦除任务。
    Erase,
}

#[uniffi::remote(Enum)]
pub enum JobStage {
    /// 任务已创建，等待执行。
    Queued,
    /// 正在连接探针和目标芯片。
    Connecting,
    /// 正在准备固件、会话或擦除参数。
    Preparing,
    /// 正在擦除目标存储。
    Erasing,
    /// 正在写入固件。
    Programming,
    /// 正在校验写入内容。
    Verifying,
    /// 正在复位目标芯片。
    Resetting,
    /// 任务成功完成。
    Completed,
    /// 任务失败。
    Failed,
}

#[uniffi::remote(Enum)]
pub enum Parity {
    #[default]
    None,
    Odd,
    Even,
}

#[uniffi::remote(Enum)]
pub enum FlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

#[uniffi::remote(Record)]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
    pub flow_control: FlowControl,
}

#[uniffi::remote(Record)]
pub struct PortInfo {
    pub path: String,
    pub display_name: String,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub vid: Option<String>,
    pub pid: Option<String>,
}

#[uniffi::remote(Record)]
pub struct SerialStatus {
    pub connected: bool,
    pub path: Option<String>,
    pub config: Option<SerialConfig>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub opened_at: Option<String>,
    pub last_error: Option<String>,
}

#[uniffi::remote(Enum)]
pub enum SendFormat {
    Hex,
    Ascii,
    Gbk,
    #[serde(rename = "utf-8")]
    Utf8,
}

#[uniffi::remote(Enum)]
pub enum ChecksumAlgorithm {
    None,
    Sum8,
    OddParity,
    EvenParity,
    Xor8,
    Crc16Modbus,
}

#[uniffi::remote(Enum)]
pub enum LineEnding {
    None,
    Lf,
    Cr,
    Crlf,
}

#[uniffi::remote(Record)]
pub struct DecodedText {
    pub ascii: String,
    pub gbk: String,
    #[serde(rename = "utf-8")]
    pub utf8: String,
}

#[uniffi::remote(Record)]
pub struct PollBatch {
    pub status: SerialStatus,
    pub frames: Vec<SerialFrameEvent>,
    pub dropped_frames: u64,
}

#[uniffi::remote(Enum)]
pub enum Direction {
    Receive,
    Send,
}

#[uniffi::remote(Record)]
pub struct SerialFrameEvent {
    pub direction: Direction,
    pub bytes: Vec<u8>,
    pub hex: String,
    pub text: DecodedText,
    pub timestamp: String,
}

#[uniffi::remote(Error)]
pub enum CoreError {
    /// Retains target candidates and recovery advice.
    Operation { response: Box<ErrorResponse> },
}

#[uniffi::remote(Record)]
pub struct ProgressEvent {
    pub id: String,
    pub kind: JobKind,
    pub stage: JobStage,
    pub progress: Option<f32>,
    pub message: String,
    pub at: AppTimestamp,
}

#[uniffi::remote(Record)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    pub probe: Option<String>,
    pub target: TargetSelection,
    pub flash_options: FlashOptions,
    pub bin_base_address: Option<u64>,
    pub updated_at: AppTimestamp,
}

#[uniffi::remote(Record)]
pub struct RecentFirmware {
    pub path: String,
    pub format: FirmwareFormat,
    pub base_address: Option<u64>,
    pub used_at: AppTimestamp,
}

#[uniffi::remote(Error)]
pub enum SerialError {
    Operation { message: String },
}
