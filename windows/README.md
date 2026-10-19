# FlashDesk for Windows

C# / WinUI 3（Windows App SDK 2.5.1）原生客户端，与 macOS 版共享 Rust Core。主窗口使用系统 Mica（不支持时使用 Desktop Acrylic），界面由系统控件在代码中构建，图标使用 Segoe Fluent Icons。

最低运行系统为 Windows 10 1809（内部版本 17763），支持 x64 和 ARM64。

## 界面

与 macOS 版相同的单一烧录流程，按 Windows 习惯呈现：

- **标题栏与菜单栏**：内容延伸到标题栏，标题显示当前固件。菜单栏提供文件、目标、操作、查看、帮助；工具栏中央显示状态和烧录进度，右侧依次为配置、设置、侧栏（日志与内存，带未读错误角标）。侧栏按钮统一控制面板开关，重新打开时恢复上次选中的页签。
- **主区域**：固件摘要卡片或拖放区与最近使用列表；烧录设置为 Windows 设置风格的卡片行；底部是唯一的主操作按钮（连接目标 / 烧录 / 预检查）。
- **右侧面板**：日志与内存在同一面板切换。日志支持筛选、仅显示错误、复制、导出和清空；内存支持按地址读取、内存映射跳转、十六进制视图和导出二进制。
- **对话框**：目标配置（芯片名在输入框中直接搜索）、芯片候选选择、保存配置、擦除确认、错误详情、设置和更新。WinUI 同时只显示一个对话框，连接失败直接显示在目标配置对话框内；目标无法唯一识别时改为打开候选列表。

快捷键：`Ctrl+O` 打开固件，`Ctrl+R` 扫描探针，`Ctrl+K` / `Ctrl+Shift+K` 连接 / 断开，`Ctrl+Enter` 烧录，`Ctrl+Shift+R` 读取内存，`Ctrl+Shift+S` 保存配置，`Ctrl+J` 日志，`Ctrl+Alt+M` 内存，`Ctrl+,` 设置。硬件操作进行中关闭窗口会被阻止并给出说明。

应用为单实例：再次启动或从资源管理器打开固件时，参数交给已运行的窗口。

## Visual Studio

安装 Visual Studio 2026 的 Windows 应用开发、C++ 桌面开发工具和对应 Windows SDK，.NET 10 LTS、Rust MSVC 工具链。打开 `windows/FlashDesk.sln`，选择 `Debug` / `Release` 和 `x64` / `ARM64` 后直接生成并运行。

MSBuild（`FlashDesk/Build/Rust.targets`）会依次构建 `flashdesk-csharp` 动态库、运行 Interoptopus 生成器，再编译 WinUI；不需要预先执行脚本。有 rustup 时自动补齐所选目标和生成器宿主目标；找不到 Cargo 时依次检查 PATH 和 `%USERPROFILE%/.cargo/bin/cargo.exe`。设计时 IntelliSense 使用已签入的绑定，不启动 Cargo。

命令行使用同一构建链：

```powershell
dotnet build windows/FlashDesk.sln -c Debug -p:Platform=x64
dotnet build windows/FlashDesk.sln -c Release -p:Platform=ARM64
```

Rust 动态库位于 `target/<目标三元组>/<debug或release>/flashdesk_csharp.dll`，构建时复制到应用目录。

## 桥接

与 BibCiTeX 相同，Windows 使用 Interoptopus 0.16 生成 C# 绑定，macOS 继续使用 UniFFI：

- `crates/flashdesk-core`：`csharp` 特性为共享模型添加 Interoptopus 类型元数据，业务逻辑不重复。
- `crates/flashdesk-csharp`：薄导出层。请求和响应均为类型化 Wire 结构，错误以 `ErrorResponse` 返回，保留候选目标与恢复建议；Rust panic 在边界转换为结果。时间戳在 C# 侧为 Unix 毫秒。
- `crates/flashdesk-csharp-bindgen`：生成 `FlashDesk/Generated/Interop.cs`（命名空间 `FlashDesk.Core`）。修改 Rust API 后 MSBuild 会重新生成，也可以手动运行 `cargo run -p flashdesk-csharp-bindgen`。
- `FlashDesk/Services/RustCore.cs`：在线程池调用阻塞的硬件与文件操作，释放 Wire 缓冲区并把错误转换为 `CoreException`。烧录和擦除期间 UI 线程每 100 ms 取一次进度事件。

配置与最近使用的固件由 Rust Core 存储在 `%APPDATA%\FlashDesk`；外观、烧录默认值与更新偏好存储在 `%LOCALAPPDATA%\FlashDesk\settings.json`。

## 运行时

应用为非打包、依赖框架的 WinUI 3。`Program.Main` 在加载 WinUI 前检查 Windows App Runtime 2.5.1，缺失时下载微软安装程序、校验固定的 SHA-256 后静默安装；已有兼容版本时直接使用。`FlashDesk.exe --prepare-runtime` 只准备运行时，不打开界面。升级 Windows App SDK 时需同时更新 `WindowsRuntime.cs` 中的版本、下载地址和哈希。

调试探针通过 probe-rs 访问；部分探针（如 ST-Link）需要安装 WinUSB 驱动，详见 [probe-rs 文档](https://probe.rs/docs/getting-started/probe-setup/)。

## 自动更新

安装包和更新由 [Velopack](https://velopack.io/) 1.2 提供，行为与 macOS 版的 Sparkle 一致：

- 从 **帮助 → 检查更新…** 或设置的“更新”部分手动检查。默认每 6 小时自动检查一次（跨启动计时），并在后台下载；下载的更新在退出 FlashDesk 时静默安装，未能安装时在下次启动、打开窗口前安装。自动检查和自动下载安装均可在设置中关闭；关闭自动下载会撤销已准备的更新。
- 手动检查发现新版本时显示发布说明，可立即下载并重新启动。硬件操作进行中不能检查更新，下载完成后的重启也会等待操作结束；窗口在硬件操作期间不能关闭，因此退出时安装也不会打断操作。
- 只有通过安装包安装的副本会检查更新；从 Visual Studio 运行的开发构建和未配置公钥的构建会在设置中说明原因。

Windows 构建没有代码签名证书，因此更新信息单独签名：

- 每个架构使用一个通道（`win-x64`、`win-arm64`）。`update-feed` Release 上的 `releases.win-<arch>.signed.json` 把 Velopack 的 `releases.win-<arch>.json` 和它的 ECDSA P-256 签名放在同一个文件中，替换期间不会出现更新信息与签名不匹配。
- 应用先用 `FlashDesk.csproj` 中 `UpdatePublicKey` 嵌入的公钥验证签名，再解析内容：只接受一个 `FlashDesk` 的稳定版完整包，文件名必须是安全的 `.nupkg` 基本名，并给出大小和 SHA-256。安装包从对应的 `v<版本号>` Release 下载，下载后校验大小与 SHA-256，不一致时删除；Velopack 不会安装比当前更旧的版本。
- 首次发布前没有更新信息，检查更新会提示暂时无法获取更新信息。

### 签名密钥

公钥已写入 `FlashDesk.csproj` 的 `<UpdatePublicKey>`，对应的私钥保存在 GitHub `release` 环境的 `WINDOWS_UPDATE_PRIVATE_KEY` secret 中，不在仓库内。发布时会先用私钥签名，再用工程公钥验证，两者不匹配即失败。

需要更换密钥时运行 `dotnet run --project windows/UpdateTool -c Release -- keygen <私钥文件>`，用输出的公钥替换 `<UpdatePublicKey>`，再以 `gh secret set WINDOWS_UPDATE_PRIVATE_KEY --env release < 私钥文件` 更新 secret 并删除私钥文件。更换后，旧版本无法验证新的更新信息，需要用户手动安装新版本。

## 发布

1. 同步 `Cargo.toml`、`macos/project.yml` 与 `windows/FlashDesk/FlashDesk.csproj` 中的版本；CI 会检查 Rust Core 与 Windows 应用版本一致。
2. 创建与版本一致的标签（例如 `v0.1.0`）并发布 GitHub Release；Release 正文作为更新说明。

`Windows` 工作流在 x64 与 ARM64 检查全部通过后，分别在对应架构的 Windows Runner 上运行 `windows/scripts/publish.ps1`：校验标签、Rust Core 与应用版本一致且已配置公钥，`dotnet publish` 依赖框架的输出，再用 `vpk pack` 生成：

- `FlashDesk-<版本号>-windows-<arch>.exe` 与 `FlashDesk-<版本号>-windows-<arch>.msi`：安装程序，缺少时会安装 .NET 10 运行时和 VC++ 运行库；Windows App Runtime 仍由应用首次启动时准备。
- `FlashDesk-<版本号>-win-<arch>-full.nupkg`：应用内更新下载的完整包，用户安装使用 exe 或 msi。

随后 `publish` 作业（`release` 环境）运行 `windows/scripts/publish-release.ps1`：用私钥签名两个架构的更新信息并立即用工程公钥验证，确认版本、包名、大小和 SHA-256，并与已发布的更新信息比较，拒绝版本回退或替换同版本中内容不同的安装包。版本 Release 中已存在的同名文件必须字节一致；全部安装包上传后才更新 `update-feed`。该作业与 macOS 发布共用 `flashdesk-update-feed` 并发组，两个平台不会同时修改更新源。

本机打包（输出未签名，不会发布）：

```powershell
./windows/scripts/publish.ps1 -Architecture x64 -Version 0.1.0 -BuildNumber 1 -OutputDirectory dist/windows-x64
```

## 测试

```powershell
cargo test --locked -p flashdesk-core -p flashdesk-csharp --all-targets --all-features
dotnet run --project windows/Tests -- target/x86_64-pc-windows-msvc/debug/flashdesk_csharp.dll
```

- Rust 测试经由 Wire 编码调用导出函数，覆盖芯片搜索、配置与最近文件的往返（含中文路径与 1970 年前的时间戳）、无效配置 ID 的结构化错误、进度时间戳换算和 panic 边界。
- `windows/Tests` 是可移植的 .NET 程序：更新部分覆盖签名验证、篡改与错误密钥、更新信息的结构限制、危险文件名、下载地址、包校验、取消与发布回退保护；传入已构建的 `flashdesk_csharp` 动态库时，还通过生成的 C# 绑定调用真实的 Rust Core（芯片搜索、内存映射、BIN 分析、结构化错误、配置与最近文件往返），不访问硬件。不传参数时只运行更新部分。

`.github/workflows/windows.yml` 在 x64 与 ARM64 上运行 Rust Core 与 Interoptopus 桥接的 Clippy 和测试、WinUI 编译、绑定一致性检查、`windows/Tests` 和发布资源检查；macOS 不测试 Windows 桥接。界面通过人工验收。
