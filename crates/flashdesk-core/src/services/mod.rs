/// 固件格式识别和烧录地址校验。
pub mod firmware;
/// 固件烧录任务编排。
pub mod flash;
/// 后端任务 ID 和任务事件发送。
pub mod jobs;
/// 目标内存读取和整片擦除。
pub mod memory;
/// 调试探针枚举和目标连接。
pub mod probe;
/// 用户配置和最近文件存储。
pub mod storage;
/// 芯片搜索和目标内存布局查询。
pub mod target;
