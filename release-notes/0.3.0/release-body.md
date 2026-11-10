# ChipDock v0.3.0

ChipDock 首次提供 Linux 正式安装包，并在三端引入多会话串口工作台。

## 本次更新

- 同时连接多个串口设备。每个会话独立保存连接参数、发送历史和协议设置。
- 新增接收分帧、协议模板、Modbus RTU、包库、文件分块发送、宏和自动应答。
- 支持原始数据采集、日志轮转、记录搜索与导出，以及按会话查看实时曲线。
- 串口工作台的“本地控制”可设为关闭、只读或允许控制，供 CLI 和 MCP 客户端访问。

<!-- platform:macos -->
## macOS

- 下载 `ChipDock-0.3.0-macOS-universal.zip`，解压后将 `ChipDock.app` 拖入“应用程序”。支持 macOS 14+、Apple 芯片和 Intel。
- 随应用附带 Rust CLI 和 MCP 工具。目标连接失败会在当前目标设置面板或主窗口显示错误。
- 应用采用 ad-hoc 代码签名，未经 Apple 公证。应用内更新通过 Sparkle Ed25519 签名验证。
<!-- /platform -->

<!-- platform:windows -->
## Windows

- 下载 `ChipDock-0.3.0-windows-x64.exe` 或 `.msi`。ARM64 请选择 `ChipDock-0.3.0-windows-arm64.exe` 或 `.msi`。支持 Windows 10 1809+。
- 随应用附带 Rust CLI 和 MCP 工具。目标连接失败时会显示原生错误对话框。
- 安装包未使用代码签名证书。应用内更新验证签名、文件大小和 SHA-256。
<!-- /platform -->

<!-- platform:linux -->
## Linux

- 首次发布 GPUI 客户端，支持固件烧录、目标内存查看、多会话串口调试和独立的日志与曲线窗口。
- 选择 `chipdock-gpui-0.3.0-linux-<架构>.deb`、`.rpm` 或 `.tar.gz`。Intel／AMD 64 位选 `x86_64`，ARM64 选 `aarch64`。运行需要 glibc 2.39+、图形桌面和 Vulkan 驱动。
- 三种安装包均含应用图标和桌面入口。便携包解压后可运行 `bash install.sh` 安装到当前用户目录。DEB 和 RPM 由系统包管理器更新。
<!-- /platform -->
