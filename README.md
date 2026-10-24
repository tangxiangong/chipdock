# ChipDock

ChipDock 是原生嵌入式开发工具集：macOS 版使用 **SwiftUI + UniFFI**，Windows 版使用 **WinUI 3 + Interoptopus**，两者共享同一个 **Rust Core**。

当前发布版本：**v0.2.2**。项目仓库：[tangxiangong/chipdock](https://github.com/tangxiangong/chipdock)。

## 功能

- **固件烧录**：支持 ELF、AXF、Intel HEX 和 BIN，分析写入地址与区段，配置擦除、校验和完成后复位。
- **串口连接**：配置与连接串口，支持 HEX / ASCII / GBK / UTF-8 收发、行尾设置、搜索、方向筛选、自动滚动及 RX / TX 统计；可将当前保留的全部收发记录导出为 JSON，包含时间、方向、原始字节、HEX 和各编码文本，不受筛选影响；内置 `data/usb-device.json` 识别常见 USB 串口适配器。
- **发送控制**：macOS 与 Windows 均支持自定义波特率、可配置毫秒间隔的自动发送，以及校验和、奇／偶校验、异或校验和 CRC16/MODBUS；默认无校验，选中后每次发送自动追加。自动发送在断开连接或发送失败时停止。
- **并行使用**：烧录与串口各用独立工作线程，可边烧录边查看终端，两者互不阻塞。两者位于同级导航；使用 macOS ⌘1 / ⌘2 或 Windows Ctrl+1 / Ctrl+2 切换，运行中的任务和串口收发会继续。
- **擦除芯片**：在主界面整片擦除 Flash，不烧录，执行前需确认。
- **目标连接**：基于 probe-rs，支持 SWD / JTAG、芯片搜索、通信速度和复位下连接。
- **内存查看**：按地址读取内存，查看十六进制与 ASCII，导出二进制，查看芯片内存映射。
- **会话管理**：保存目标配置、记录最近使用的固件，查看操作进度和活动日志。
- **自动更新**：macOS 使用 [Sparkle 2](https://sparkle-project.org/)，Windows 使用 [Velopack](https://velopack.io/)，检查、下载、验证签名并安装更新。

macOS 界面使用系统字体、SF Symbols 和原生控件；应用图标使用 Xcode Icon Composer 制作。Windows 版的界面、构建、桥接、自动更新与发布说明见 [windows/README.md](windows/README.md)。

发送校验按当前编码转换后的数据计算，追加在行尾之前；HEX 模式仍不追加行尾。奇／偶校验对整段数据追加一个 `0` 或 `1` 字节，串口通讯参数中的奇偶校验仍单独设置。CRC16/MODBUS 按[串行线路规范](https://www.modbus.org/docs/Modbus_over_serial_line_V1_02.pdf)以低字节在前的顺序追加。

## 烧录器与驱动

Windows 会在连接时检查并尝试安装 / 修复所选 USB 调试接口的系统 WinUSB 驱动，覆盖 ST-Link、J-Link、CMSIS-DAP v2、已知 FTDI 调试通道、WCH-Link 和 CH347；Glasgow 支持修复已枚举 applet 的接口对。具体范围和限制见 [Windows 驱动说明](windows/README.md#运行时)。HID、串口和存储接口保留原驱动；Black Magic Probe、SiFli UART 使用对应的串口驱动。

macOS 上常见 USB 调试探针通常无需额外驱动。使用 Apple 芯片的 Mac 笔记本时，需解锁并[允许 USB 配件连接](https://support.apple.com/zh-cn/102282)。扫描为空和 USB 访问失败时，应用会显示授权、设备断开或接口占用的恢复建议。若其他调试工具或 USB/FTDI 内核驱动占用了调试接口，需释放占用；当前 USB 库不能在 macOS 自动解除内核驱动占用。

## 构建运行

### macOS

需要 **macOS 14+、Xcode 26+**，以及通过 [rustup](https://rustup.rs/) 安装的稳定版 Rust。

1. 打开 `macos/ChipDock.xcodeproj`。
2. 选择 **ChipDock → My Mac**。
3. 按 **⌘R** 构建运行。

Xcode 会解析 Sparkle 包依赖、编译 Rust 静态库、生成 UniFFI 绑定并链接应用。首次构建需要下载依赖。

也可以从终端构建：

```sh
./macos/build.sh
```

### Windows

需要 Visual Studio 2026（Windows 应用开发与 C++ 桌面开发工具）、.NET 10 和 Rust MSVC 工具链。打开 `windows/ChipDock.sln`，选择 `x64` 或 `ARM64` 后生成运行；MSBuild 会编译 Rust 动态库并生成 C# 绑定。

## 架构

```text
assets/app-icons/             应用图标源文件
data/                        USB 设备目录
bindings/
  generator/                 UniFFI 绑定生成器
  csharp-generator/          Interoptopus 绑定生成器
  generate-swift.sh           Swift/C 绑定生成入口
  generated/                 本地产物，Git 忽略
crates/
  chipdock-core/             固件、探针、串口传输与存储原语
  chipdock-service/          共享应用 API、硬件互斥、进度与串口工作线程
  chipdock-ffi/              Swift UniFFI 导出与共享类型注册
  chipdock-csharp/           Windows Interoptopus 导出层
  chipdock-release-notes/    分平台发布公告
macos/
  ChipDock/Sources/ChipDock/ SwiftUI/AppKit 源码与资源
  ChipDock/Tests/            集成与逻辑检查
  ChipDock.xcodeproj/        Xcode 工程、依赖与 RustBindings 构建目标
  Info.plist                 应用元数据
  build.sh                   CLI 构建入口
  build-bindings.sh          Xcode 调用的 Rust/UniFFI 构建步骤
windows/
  ChipDock/                  WinUI 源码、资源与 Build/MSBuild 目标
  Tests/                     Rust/C# 桥接与更新检查
  UpdateTool/                更新签名与发布校验
  scripts/                   本机构建与打包入口
  ChipDock.sln               Visual Studio 解决方案
ci/                          分平台发布脚本与说明
xtask/                       共享发布验证与 COS 发布逻辑
Justfile                     平台命令别名
```

Rust Core 提供类型化 API、结构化错误和进度事件。探针操作在专用线程上串行执行，串口调用使用另一条专用线程，异步接收由独立的 Tokio 运行时维持；串口不会等待烧录完成。串口传输、编码与 USB 设备库来自 `serial-port` 项目，通过 ChipDock 现有 UniFFI / Interoptopus 绑定提供给两端。

项目整体按 BibCiTeX 的原生分层组织。生成绑定不提交 Git：Xcode 的 `RustBindings` 依赖在 DerivedData 中生成 Swift/C 绑定，MSBuild 在 `bindings/generated/csharp` 中生成 C# 绑定。干净检出后可直接构建，不需要预先执行生成脚本；详见 [bindings/README.md](bindings/README.md)。依赖版本由 `Cargo.lock` 和 `Package.resolved` 固定。

## 测试

```sh
cargo fmt --all -- --check
cargo clippy --locked -p chipdock-core -p chipdock-service -p chipdock-ffi -p chipdock-bindgen --all-targets -- -D warnings
./macos/ChipDock/Tests/run-interop-tests.sh
./macos/ChipDock/Tests/run-update-tests.sh
bash macos/ChipDock/Tests/run-serial-tests.sh
cargo test --locked -p chipdock-release-notes
xcodebuild -project macos/ChipDock.xcodeproj -scheme ChipDock \
  -configuration Debug -destination 'platform=macOS' \
  -derivedDataPath .build/Xcode build
```

- `run-interop-tests.sh` 运行 Rust Core 单元测试和独立的 Swift/UniFFI 集成测试，不启动应用或连接硬件。
- `run-update-tests.sh` 验证更新包签名、文件长度、版本、下载地址，以及发布回退保护。
- `run-serial-tests.sh` 运行不启动界面的 Swift 逻辑检查，覆盖自动发送的停止与重启、设置兼容以及自定义波特率输入。
- `chipdock-release-notes` 的逻辑测试覆盖通用公告、平台筛选、Markdown 保留和错误标记拦截。
- Xcode 构建验证原生应用与生成绑定的编译和链接；GUI 验收由人工进行。

测试按平台划分，均不访问硬件：`Rust` 工作流在 Linux 上检查格式并测试与平台无关的 Rust Core；`macOS` 工作流在 Apple Silicon 和 Intel 上构建应用并测试 Rust 服务、UniFFI 桥接与非 UI 逻辑；`Windows` 工作流在 x64 与 ARM64 上测试 Rust Core、Interoptopus 桥接与应用。Windows 的测试与 CI 见 [windows/README.md](windows/README.md#测试)。

## 自动更新

以下为 macOS；Windows 的行为相同，细节见 [windows/README.md](windows/README.md#自动更新)。

从 **ChipDock → 检查更新…** 或设置的“关于”页面手动检查。默认每 6 小时自动检查；自动检查和自动下载安装均可在设置中关闭。

Sparkle 验证 Ed25519 签名后安装更新并重新启动应用。探针操作未完成、串口仍连接或正在收发时，应用会延后更新重启。

与 BibCiTeX 一样，macOS 与 Windows 每次检查都优先使用腾讯云 COS，主源失败时尝试 GitHub 备用源。有效的主源没有新版本时不会查询备用源。macOS 仅在更新源或下载传输失败时重试一次，保留手动或后台检查模式；取消、签名、验证或安装错误不重试。Windows 的更新源和包下载失败时回退，两个来源都必须通过签名验证；包回退保留已选版本、文件名、大小和 SHA-256，用户取消即停止。

两端统一使用独立的 ChipDock 安装身份：macOS 为 `com.xiaoyu.chipdock`，应用包为 `ChipDock.app`，配置目录为 `~/Library/Application Support/com.xiaoyu.chipdock`；Windows 的 Velopack 包 ID、安装名称与配置目录均为 `ChipDock`，主程序为 `ChipDock.exe`。新身份使用独立的 COS `updates/` 与 GitHub `chipdock-updates` 更新源，继续验证工程中已有的签名公钥。

此前仍使用 FlashDesk 安装身份的版本（包括已发布的 v0.2.0）不再支持原位升级或配置迁移。请退出并卸载旧版，再使用新版安装包重新安装；macOS 将 `ChipDock.app` 放入“应用程序”，Windows 重新创建 ChipDock 快捷方式并移除旧的任务栏固定项。后续 ChipDock 版本之间可正常原位升级。

## 发布

1. 使用尚未发布的新版本号，同步 `Cargo.toml`、`macos/ChipDock.xcodeproj/project.pbxproj` 的 `MARKETING_VERSION` 和 `windows/ChipDock/ChipDock.csproj` 中的版本。不要覆盖身份切换前已发布的安装包。
2. GitHub 仓库的 `release` 环境需要 `SPARKLE_PRIVATE_ED_KEY`、`WINDOWS_UPDATE_PRIVATE_KEY`、`COS_SECRET_ID` 和 `COS_SECRET_KEY` secret。前两项对应工程内的更新公钥；后两项可复用 BibCiTeX 的 COS 发布子账号。
3. 创建与版本一致的标签，例如 `v0.1.0`，并发布 GitHub Release。

`macOS` 与 `Windows` 两个工作流各自构建并上传本平台的安装包和签名更新信息，再更新 GitHub `chipdock-updates`，最后将经过验证的相同安装包和 COS 更新信息发布到 COS。GitHub 备用源就绪后 COS 才会推进。

CI 通过全部检查后，构建 Apple Silicon + Intel universal 应用，生成 `ChipDock-<版本号>-macOS-universal.zip` 和签名 appcast。打包验证应用包文件名、Bundle ID、内部名称、显示名称与可执行文件均属于 ChipDock。版本包上传完成后，才会更新 `chipdock-updates`；流程会拒绝版本回退或覆盖内容不同的已发布安装包。

应用使用 ad-hoc 代码签名，更新包使用独立的 Sparkle Ed25519 签名。本机打包从 `SPARKLE_PRIVATE_ED_KEY` 或登录钥匙串的 `com.xiaoyu.chipdock` 账户读取与工程公钥对应的私钥；私钥不属于项目文件。

本机已构建 Release 应用时，可直接打包并验证：

```sh
./ci/release-macos.sh
```

打包产物位于 `.build/ReleaseAssets/`。`--publish` 模式由 CI 使用。

### COS 主源与 GitHub 备用源

与 BibCiTeX 共用广州地域的 `app-release-1302963684` 桶，ChipDock 仅写入 `chipdock/`，不修改其他应用的对象：

- 主源：`https://app-release-1302963684.cos.ap-guangzhou.myqcloud.com/chipdock/updates`
- 备用：`https://github.com/tangxiangong/chipdock/releases/download/chipdock-updates`
- 安装包与保留的签名更新信息：`chipdock/releases/v<版本号>/`。

桶需启用全球加速；发布通过加速端点上传，客户端通过广州 HTTPS 端点公开下载。发布子账号需要 `cos:HeadObject`、`cos:PutObject` 和 `cos:PutObjectACL`，范围至少覆盖 `chipdock/*`；已有的共享桶发布策略可以继续使用。发布设置 `public-read`，更新源使用 `no-cache, max-age=60`，版本文件使用一年不可变缓存。对象 ACL 和禁止覆盖请求头遵循腾讯云的 [PUT Object 文档](https://cloud.tencent.com/document/product/436/7749)。凭据仅用于发布，不嵌入应用。

与 BibCiTeX 使用同样的 Rust `xtask`，COS v5 签名、上传重试、匿名字节校验和 XML 解析直接复用其实现。`xtask publish-cos` 先下载 GitHub 发布文件，核对 GitHub SHA-256、Sparkle Ed25519 签名和 Windows P-256 签名，并校验 Windows 包大小与 SHA-256。COS appcast 只改安装包 URL，原 XML、安装包、签名和公告内容保持不变；Windows 两个来源使用相同签名更新信息。所有版本文件公开可读且字节一致后才更新可变源。发布前检查全部不可变对象，拒绝覆盖不同内容；先保存 `updates/state.json` 中所有待推进版本，再更新各架构的源，让中断重试或旧版补传不会回退更新源。所有发布工作流共用 `chipdock-updates` 并发组。

COS 上传失败时，使用 **Publish existing release to COS**，填写身份切换后已发布的标签并选择 `all`、`macos` 或 `windows`；无需重建、重新签名或替换 GitHub 安装包。版本 Release 必须同时保留安装包和签名更新信息，补传不再从旧的可变更新源查找缺失文件。不要删除 `state.json`，也不要对仍被更新源引用的文件配置过期清理。

与 BibCiTeX 相同，普通补传拒绝不同字节；只有维护者明确替换了该版本的 GitHub 发布文件后，才启用 `replace_existing`，调用 `xtask replace-cos-release` 修复该版本。修复先验证完整签名发布，要求已有有效 COS 清单，保留其他版本，并遵守更新源的防回退规则。

已有版本预留时，可从经过验证的发布文件恢复损坏的客户端更新源，仍须遵守预留版本的防回退限制。没有预留且现有更新源损坏时，发布会停止，不擅自重置版本记录。GitHub 发布也会先检查全部已有文件的摘要，再上传缺失文件。

本机验证发布工具（不上传）：

```sh
cargo test --locked -p xtask
cargo run --locked -p xtask -- verify-release --tag "v<版本号>"
```

macOS 的回退策略可在 Xcode 已解析 Sparkle 依赖后通过 `bash macos/ChipDock/Tests/run-update-mirror-tests.sh` 验证；可移植 C# 更新逻辑使用 `dotnet run --project windows/Tests/ChipDock.Tests.csproj -- --updates-only`。这些检查不运行应用或访问硬件。

### 分平台更新公告

GitHub Release 正文可以使用与 BibCiTeX 相同的平台标记。macOS 与 Windows 打包脚本使用同一个 `chipdock-release-notes` 解析器，分别将“通用内容 + 本平台内容”写入 Sparkle appcast 和 Velopack 更新包；GitHub 保留完整正文。没有标记的旧公告仍全部显示。

```markdown
# 本次更新

- 改进固件解析，所有平台可用。

<!-- platform:macos -->
## macOS
- 改进窗口恢复。
<!-- /platform -->

<!-- platform:windows -->
## Windows
- 改进驱动安装提示。
<!-- /platform -->
```

- 未标记内容及 `<!-- platform:all -->` 块为通用内容。
- 平台名称区分大小写，支持 `macos`、`windows`、`linux`，以及 `macos,windows` 这样的逗号列表；`all` 必须单独使用。
- 标记必须独占一行，每块以 `<!-- /platform -->` 结束，不能嵌套。平台专属标题也应放在对应块内。
- 解析保留正文顺序、Markdown 和换行，移除平台标记。未知平台、空选择器、错误语法、未闭合或多余结束标记会终止打包，即使错误位于被排除的块内。代码围栏内也不能放置这些保留标记。
- 提供公告文件时，筛选结果不能为空；至少保留通用版本标题或摘要。`linux` 仅用于兼容公告语法，目前没有 Linux 客户端。

本地预览，无需构建应用或发布（`ReleaseNotes.md` 为待发布的完整正文）：

```sh
cargo run --locked -p chipdock-release-notes -- macos ReleaseNotes.md .build/notes-macos.md
cargo run --locked -p chipdock-release-notes -- windows ReleaseNotes.md .build/notes-windows.md
```

本机 macOS 打包通过 `RELEASE_NOTES_FILE` 传入完整正文；Windows 使用 `publish.ps1 -ReleaseNotes`。CI 会自动读取 GitHub Release 正文并按平台筛选，不修改 GitHub 公告，也不回写已发布的更新源。

## 许可证

本项目采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 双许可证。

第三方依赖说明见 [NOTICE](NOTICE)。
