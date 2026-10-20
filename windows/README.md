# ChipDock for Windows

C# / WinUI 3（Windows App SDK 2.5.1）原生客户端，与 macOS 版共享 Rust Core。主窗口使用系统 Mica（不支持时使用 Desktop Acrylic），界面由系统控件在代码中构建，图标使用 Segoe Fluent Icons。

最低运行系统为 Windows 10 1809（内部版本 17763），支持 x64 和 ARM64。

## 界面

与 macOS 版相同的单一烧录流程，按 Windows 习惯呈现：

- **标题栏与菜单栏**：内容延伸到标题栏，标题显示当前固件。菜单栏提供文件、目标、操作、查看、帮助；工具栏中央显示状态和烧录进度，右侧依次为配置、设置、侧栏（日志与内存，带未读错误角标）。侧栏按钮统一控制面板开关，重新打开时恢复上次选中的页签。
- **主区域**：固件摘要卡片或拖放区与最近使用列表；固件内容按加载地址显示起始最多 4 KiB 的十六进制和 ASCII 数据，空洞显示为 `··`。预览随可用宽度调整每行字节数，数据不换行，滚动条隐藏；它读取固件文件，不访问目标芯片。烧录设置为 Windows 设置风格的卡片行；底部是唯一的主操作按钮（连接目标 / 烧录 / 预检查）。
- **最近使用**：保留最多 10 条记录，列表超过 5 条时滚动显示；支持双击或 Enter 打开、Delete 或行内按钮移除、右键菜单、清除失效记录和清空列表。文件不存在时显示标记并禁用打开和定位，窗口重新激活时刷新状态；移除记录不会删除固件文件。历史保存按顺序处理，避免打开文件与移除记录相互覆盖。
- **窗口布局与进度**：较矮的窗口使用紧凑间距，额外高度分配给拖放区或固件内容预览；设置与操作之间保持固定间距。进度百分比四舍五入并单独显示，消息末尾重复的百分比会隐藏，窄窗口优先收缩消息文本。
- **右侧面板**：日志与内存在同一面板切换。日志支持筛选、仅显示错误、复制、导出和清空；内存支持按地址读取、内存映射跳转、十六进制视图和导出二进制。
- **对话框**：目标配置（芯片名在输入框中直接搜索）、芯片候选选择、保存配置、擦除确认、错误详情、设置和更新。WinUI 同时只显示一个对话框，连接失败直接显示在目标配置对话框内；目标无法唯一识别时改为打开候选列表。

快捷键：`Ctrl+O` 打开固件，`Ctrl+R` 扫描探针，`Ctrl+K` / `Ctrl+Shift+K` 连接 / 断开，`Ctrl+Enter` 烧录，`Ctrl+Shift+R` 读取内存，`Ctrl+Shift+S` 保存配置，`Ctrl+J` 日志，`Ctrl+Alt+M` 内存，`Ctrl+,` 设置。硬件操作进行中关闭窗口会被阻止并给出说明。

应用为单实例：再次启动或从资源管理器打开固件时，参数交给已运行的窗口。

## Visual Studio

安装 Visual Studio 2026 的 Windows 应用开发、C++ 桌面开发工具和对应 Windows SDK，.NET 10 LTS、Rust MSVC 工具链。打开 `windows/ChipDock.sln`，选择 `Debug` / `Release` 和 `x64` / `ARM64` 后直接生成并运行。

MSBuild（`ChipDock/Build/Rust.targets`）会依次构建 `chipdock-csharp` 动态库、运行 Interoptopus 生成器，再编译 WinUI；不需要预先执行脚本。有 rustup 时自动补齐所选目标和生成器宿主目标；找不到 Cargo 时依次检查 PATH 和 `%USERPROFILE%/.cargo/bin/cargo.exe`。设计时 IntelliSense 使用已签入的绑定，不启动 Cargo。

命令行使用同一构建链：

```powershell
dotnet build windows/ChipDock.sln -c Debug -p:Platform=x64
dotnet build windows/ChipDock.sln -c Release -p:Platform=ARM64
```

Rust 动态库位于 `target/<目标三元组>/<debug或release>/chipdock_csharp.dll`，构建时复制到应用目录。

## 桥接

与 BibCiTeX 相同，Windows 使用 Interoptopus 0.16 生成 C# 绑定，macOS 继续使用 UniFFI：

- `crates/chipdock-core`：`csharp` 特性为共享模型添加 Interoptopus 类型元数据，业务逻辑不重复。
- `crates/chipdock-csharp`：薄导出层。请求和响应均为类型化 Wire 结构，错误以 `ErrorResponse` 返回，保留候选目标与恢复建议；Rust panic 在边界转换为结果。时间戳在 C# 侧为 Unix 毫秒。
- `crates/chipdock-csharp-bindgen`：生成 `ChipDock/Generated/Interop.cs`（命名空间 `ChipDock.Core`）。修改 Rust API 后 MSBuild 会重新生成，也可以手动运行 `cargo run -p chipdock-csharp-bindgen`。
- `ChipDock/Services/RustCore.cs`：在线程池调用阻塞的硬件与文件操作，释放 Wire 缓冲区并把错误转换为 `CoreException`。烧录和擦除期间 UI 线程每 100 ms 取一次进度事件。

配置与最近使用的固件由 Rust Core 存储在 `%APPDATA%\FlashDesk`；外观、烧录默认值与更新偏好存储在 `%LOCALAPPDATA%\FlashDesk\settings.json`。

启动或运行期间可捕获的未处理 .NET / WinUI 异常会写入 `%LOCALAPPDATA%\FlashDesk\logs\crash.log`，包含异常消息和调用栈，可用于排查主窗口尚未打开时的闪退。

## 运行时

应用为非打包、依赖框架的 WinUI 3。`Program.Main` 在加载 WinUI 前检查 Windows App Runtime 2.5.1，缺失时下载微软安装程序、校验固定的 SHA-256 后静默安装；已有兼容版本时直接使用。`ChipDock.exe --prepare-runtime` 只准备运行时，不打开界面。升级 Windows App SDK 时需同时更新 `WindowsRuntime.cs` 中的版本、下载地址和哈希。

调试探针通过 probe-rs 访问。连接时，应用检查所选探针的 USB 调试接口；缺少驱动、已绑定可识别的旧版驱动或缺少 WinUSB 接口 GUID 时，通过 UAC 授权运行短时安装进程，尝试绑定 Windows 内置且已签名的 WinUSB、补齐接口 GUID，然后等待设备启动且访问路径启用后继续连接。要求使用与 Windows 架构一致的 x64 / ARM64 版 ChipDock，无需下载旧版协同安装程序。已有正常 WinUSB 不重复安装；取消授权、拔出设备、安装失败或需要重启均会显示具体提示。

| 类型 | 自动安装 / 修复范围 |
| --- | --- |
| ST-Link V2 / V2-1 / V3 系列 | 独立 V2 或复合设备的调试接口 MI_00；可迁移 STTub 系列旧驱动，保留短序列号编码兼容 |
| J-Link | probe-rs 可识别的 VID/PID，对独立设备或 MI_00 安装 / 修复 WinUSB，可迁移旧 JLink 驱动 |
| CMSIS-DAP v2 | USB 总线报告名称含 CMSIS-DAP，且接口类为 FF/00 的 Bulk 接口；不更改 v1 HID 接口 |
| FTDI / Olimex | probe-rs 支持的 VID/PID 的通道 A（MI_00），或独立 FT232H；可迁移 FTDIBUS，不修改其他 UART 通道 |
| WCH-Link RISC-V 模式 | 1A86:8010 的调试接口；ARM 的 CMSIS-DAP 模式按 CMSIS-DAP 规则处理 |
| CH347 | 1A86:55DD / 55DE / 55E8 的调试接口，优先 MI_04，缺少时使用 MI_02 |
| Glasgow | 必须已由 probe-rs 枚举出 applet 的序列号和 IN/OUT 接口；只修复选定的接口对，不猜测 applet |

系统扫描会补充部分驱动异常、未出现在 probe-rs 列表中的设备。由于 Windows 实例 ID 中的序列号可能被转换大小写，仅在同一 VID/PID 下能确认唯一物理设备时补充无序列号的选择器；修复后重新通过 probe-rs 枚举并核对物理设备。多个同型号且无法可靠区分的设备不会被自动选择或批量安装。

复合设备父节点、HID、CDC 串口、存储和 Hub 接口均排除；仅迁移已识别的驱动服务，未知服务显示人工恢复建议。WinUSB 可用性预检失败时不会提权安装。Black Magic Probe 和 SiFli UART 使用串口，保留串口驱动；缺少驱动时应使用 Windows Update 或对应芯片厂商的驱动包。某些旧 J-Link 可先通过 [J-Link Configurator](https://kb.segger.com/J-Link_WinUSB_driver_selection) 切换 WinUSB；无法自动处理的 USB 调试接口可使用 [Zadig](https://zadig.akeo.ie/)，只修改调试接口，切换后原厂工具的可用性取决于其驱动要求。其他要求见 [probe-rs 文档](https://probe.rs/docs/getting-started/probe-setup/)。

## 自动更新

安装包和更新由 [Velopack](https://velopack.io/) 1.2 提供，行为与 macOS 版的 Sparkle 一致：

- 从 **帮助 → 检查更新…** 或设置的“更新”部分手动检查。默认每 6 小时自动检查一次（跨启动计时），并在后台下载；下载的更新在退出 ChipDock 时静默安装，未能安装时在下次启动、打开窗口前安装。自动检查和自动下载安装均可在设置中关闭；关闭自动下载会撤销已准备的更新。
- 手动检查发现新版本时显示发布说明，可立即下载并重新启动。硬件操作进行中不能检查更新，下载完成后的重启也会等待操作结束；窗口在硬件操作期间不能关闭，因此退出时安装也不会打断操作。
- 只有通过安装包安装的副本会检查更新；从 Visual Studio 运行的开发构建和未配置公钥的构建会在设置中说明原因。

Windows 构建没有代码签名证书，因此更新信息单独签名：

- 每个架构使用一个通道（`win-x64`、`win-arm64`）。`update-feed` Release 上的 `releases.win-<arch>.signed.json` 把 Velopack 的 `releases.win-<arch>.json` 和它的 ECDSA P-256 签名放在同一个文件中，替换期间不会出现更新信息与签名不匹配。
- 应用先用 `ChipDock.csproj` 中 `UpdatePublicKey` 嵌入的公钥验证签名，再解析内容：只接受一个包 ID 为 `FlashDesk` 的稳定版完整包，文件名必须是安全的 `.nupkg` 基本名，并给出大小和 SHA-256。安装包从对应的 `v<版本号>` Release 下载，下载后校验大小与 SHA-256，不一致时删除；Velopack 不会安装比当前更旧的版本。
- 首次发布前没有更新信息，检查更新会提示暂时无法获取更新信息。

### 签名密钥

公钥已写入 `ChipDock.csproj` 的 `<UpdatePublicKey>`，对应的私钥保存在 GitHub `release` 环境的 `WINDOWS_UPDATE_PRIVATE_KEY` secret 中，不在仓库内。发布时会先用私钥签名，再用工程公钥验证，两者不匹配即失败。

需要更换密钥时运行 `dotnet run --project windows/UpdateTool -c Release -- keygen <私钥文件>`，用输出的公钥替换 `<UpdatePublicKey>`，再以 `gh secret set WINDOWS_UPDATE_PRIVATE_KEY --env release < 私钥文件` 更新 secret 并删除私钥文件。更换后，旧版本无法验证新的更新信息，需要用户手动安装新版本。

## 发布

1. 同步 `Cargo.toml`、`macos/project.yml` 与 `windows/ChipDock/ChipDock.csproj` 中的版本；CI 会检查 Rust Core 与 Windows 应用版本一致。
2. 创建与版本一致的标签（例如 `v0.1.0`）并发布 GitHub Release；Release 正文作为更新说明。

`Windows` 工作流在 x64 与 ARM64 检查全部通过后，分别在对应架构的 Windows Runner 上运行 `windows/scripts/publish.ps1`：校验标签、Rust Core 与应用版本一致且已配置公钥，`dotnet publish` 依赖框架的输出，再用 `vpk pack` 生成：

- `ChipDock-<版本号>-windows-<arch>.exe` 与 `ChipDock-<版本号>-windows-<arch>.msi`：安装程序，缺少时会安装 .NET 10 运行时和 VC++ 运行库；Windows App Runtime 仍由应用首次启动时准备。
- `FlashDesk-<版本号>-win-<arch>-full.nupkg`：应用内更新下载的完整包，用户安装使用 exe 或 msi。

随后 `publish` 作业（`release` 环境）运行 `windows/scripts/publish-release.ps1`：用私钥签名两个架构的更新信息并立即用工程公钥验证，确认版本、包名、大小和 SHA-256，并与已发布的更新信息比较，拒绝版本回退或替换同版本中内容不同的安装包。版本 Release 中已存在的同名文件必须字节一致；全部安装包上传后才更新 `update-feed`。该作业与 macOS 发布共用 `flashdesk-update-feed` 并发组，两个平台不会同时修改更新源。

本机打包（输出未签名，不会发布）：

```powershell
./windows/scripts/publish.ps1 -Architecture x64 -Version 0.1.0 -BuildNumber 1 -OutputDirectory dist/windows-x64
```

## 测试

```powershell
cargo test --locked -p chipdock-core -p chipdock-csharp --all-targets --all-features
dotnet run --project windows/Tests -- target/x86_64-pc-windows-msvc/debug/chipdock_csharp.dll
dotnet run --project windows/Tests -- --workbench-only
```

- Rust 测试经由 Wire 编码调用导出函数，覆盖芯片搜索、配置与最近文件的往返（含中文路径与 1970 年前的时间戳）、无效配置 ID 的结构化错误、进度时间戳换算和 panic 边界。
- `--workbench-only` 仅运行可移植的历史记录与进度文字逻辑检查，覆盖文件失效与恢复、清理、并发保存顺序、保存失败后的恢复、Windows 路径大小写和百分比处理，不启动 WinUI 或调用设备 API。
- `windows/Tests` 是可移植的 .NET 程序：更新部分覆盖签名验证、篡改与错误密钥、更新信息的结构限制、危险文件名、下载地址、包校验、取消与发布回退保护；传入已构建的 `chipdock_csharp` 动态库时，还通过生成的 C# 绑定调用真实的 Rust Core（芯片搜索、内存映射、BIN 分析、结构化错误、配置与最近文件往返），不连接目标芯片。不传参数时跳过 Rust/C# 桥接检查。
- USB 驱动策略和模拟安装流程可在宿主机独立运行：`dotnet run --project windows/Tests -- --driver-rules-only`。覆盖型号与接口筛选、HID/串口/存储保护、旧驱动迁移、单设备补充发现、多设备歧义、FTDI 通道、Glasgow 接口对、ST-Link 短序列号、GUID 修复，以及取消、失败、重启要求、延迟就绪和设备拔出。它们不调用 Windows 安装 API。仅在 Windows 上额外运行真实 SetupAPI 只读枚举、WinUSB 预检与接口状态检查；传入 Rust 动态库后还核对真实 ST-Link 选择器。测试不会安装驱动或连接目标芯片；模拟测试不能替代 Windows x64 / ARM64 实机驱动验收。

`.github/workflows/windows.yml` 在 x64 与 ARM64 上运行 Rust Core 与 Interoptopus 桥接的 Clippy 和测试、WinUI 编译、绑定一致性检查、`windows/Tests` 和发布资源检查；macOS 不测试 Windows 桥接。界面通过人工验收。

### v0.1.2 改名过渡

应用与安装程序显示为 ChipDock；源码工程与可执行文件名统一为 `ChipDock` / `ChipDock.exe`，Velopack 包 ID `FlashDesk`、原配置目录与更新验证公钥保持不变。更新检查优先 `tangxiangong/chipdock`，网络不可用时回退 `tangxiangong/flashdesk`；包下载使用相同迁移策略，签名或哈希失败直接拒绝更新。发布脚本使用 `GITHUB_REPOSITORY`，支持在仓库改名前后继续发布。
