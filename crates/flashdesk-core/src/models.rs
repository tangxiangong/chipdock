use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 支持的固件文件格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(uniffi::Enum)]
pub enum FirmwareFormat {
    /// ELF 可执行文件，地址信息来自文件内容。
    Elf,
    /// Intel HEX 文件，地址信息来自文件内容。
    Hex,
    /// 原始二进制文件，需要调用方提供烧录基地址。
    Bin,
}

/// 前端传入的固件文件信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct FirmwareInput {
    /// 固件文件的本地路径。
    pub path: String,
    /// 用户显式选择的格式；为空时根据文件扩展名推断。
    pub format: Option<FirmwareFormat>,
    /// BIN 固件的烧录基地址。
    pub base_address: Option<u64>,
}

/// 固件文件中会被写入 Flash 的地址段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct FirmwareUsageSegment {
    /// 起始写入地址。
    pub start: u64,
    /// 结束地址，开区间。
    pub end: u64,
    /// 该连续地址段的字节数。
    pub size: u64,
}

/// 固件 Flash 占用分析结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 可用调试探针的前端摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 一次目标连接或目标操作所需的选择信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 调试探针线协议。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(uniffi::Enum)]
pub enum WireProtocol {
    /// Serial Wire Debug。
    Swd,
    /// JTAG。
    Jtag,
}

/// 固件烧录行为选项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 前端提交的一次烧录任务请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 前端提交的一次整片擦除请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct EraseRequest {
    /// 选中的探针；为空时要求当前只连接一个探针。
    pub probe: Option<String>,
    /// 目标芯片和连接参数。
    pub target: TargetSelection,
}

/// 前端提交的一次连接探测请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct ConnectRequest {
    /// 选中的探针；为空时要求当前只连接一个探针。
    pub probe: Option<String>,
    /// 目标芯片和连接参数。
    pub target: TargetSelection,
}

/// 通过调试接口直接读取的目标硬件信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 成功连接目标后的摘要信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 自动识别失败时，后端按硬件可读信息缩小出的目标候选。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct TargetCandidate {
    /// probe-rs 可接收的目标名称。
    pub name: String,
    /// 候选所属芯片族。
    pub family: String,
}

/// 前端提交的一次内存读取请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 内存读取结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
pub struct MemoryReadResult {
    /// 实际读取起始地址。
    pub address: u64,
    /// 实际读取长度，单位字节。
    pub length: u32,
    /// 小写十六进制连续字节字符串。
    pub data_hex: String,
}

/// 目标芯片内存区域类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(uniffi::Enum)]
pub enum MemoryRegionKind {
    /// 非易失存储，通常是 Flash。
    Nvm,
    /// RAM 区域。
    Ram,
    /// probe-rs 未归类的区域。
    Generic,
}

/// 内存区域访问权限。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 目标芯片的内存区域布局。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(uniffi::Record)]
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

/// 后台异步任务标识。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub Uuid);

/// 后台任务类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(uniffi::Enum)]
pub enum JobKind {
    /// 固件烧录任务。
    Flash,
    /// 整片擦除任务。
    Erase,
}

/// 后台任务阶段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(uniffi::Enum)]
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

/// 后台任务推送给前端的进度事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    /// 任务标识。
    pub id: JobId,
    /// 任务类型。
    pub kind: JobKind,
    /// 当前任务阶段。
    pub stage: JobStage,
    /// 任务进度，范围 0.0 到 1.0。
    pub progress: Option<f32>,
    /// 前端可直接展示的阶段消息。
    pub message: String,
    /// 事件产生时间。
    pub at: DateTime<Utc>,
}

/// 用户保存的烧录配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// 配置标识。
    pub id: Uuid,
    /// 用户可见配置名称。
    pub name: String,
    /// 保存的探针选择器。
    pub probe: Option<String>,
    /// 保存的目标选择。
    pub target: TargetSelection,
    /// 保存的烧录选项。
    pub flash_options: FlashOptions,
    /// 保存的 BIN 基地址。
    pub bin_base_address: Option<u64>,
    /// 最近更新时间。
    pub updated_at: DateTime<Utc>,
}

/// 最近使用的固件文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFile {
    /// 固件文件路径。
    pub path: String,
    /// 最近使用时的固件格式。
    pub format: FirmwareFormat,
    /// 最近使用时的 BIN 基地址。
    pub base_address: Option<u64>,
    /// 最近使用时间。
    pub used_at: DateTime<Utc>,
}
