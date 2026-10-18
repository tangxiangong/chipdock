# FlashDesk

FlashDesk 是原生嵌入式固件工作台：macOS 版使用 **SwiftUI + UniFFI**，Windows 版使用 **WinUI 3 + Interoptopus**，两者共享同一个 **Rust Core**。

当前版本：**v0.1.0**。源代码：[tangxiangong/flashdesk](https://github.com/tangxiangong/flashdesk)。

## 功能

- **固件烧录**：支持 ELF、AXF、Intel HEX 和 BIN，分析写入地址与区段，配置擦除、校验和完成后复位。
- **目标连接**：基于 probe-rs，支持 SWD / JTAG、芯片搜索、通信速度和复位下连接。
- **内存查看**：按地址读取内存，查看十六进制与 ASCII，导出二进制，查看芯片内存映射。
- **会话管理**：保存目标配置、记录最近使用的固件，查看操作进度和活动日志。
- **自动更新**：macOS 使用 [Sparkle 2](https://sparkle-project.org/)，Windows 使用 [Velopack](https://velopack.io/)，检查、下载、验证签名并安装更新。

macOS 界面使用系统字体、SF Symbols 和原生控件；应用图标使用 Xcode Icon Composer 制作。Windows 版的界面、构建、桥接、自动更新与发布说明见 [windows/README.md](windows/README.md)。

## 构建运行

### macOS

需要 **macOS 14+、Xcode 26+**，以及通过 [rustup](https://rustup.rs/) 安装的稳定版 Rust。

1. 打开 `macos/FlashDesk.xcodeproj`。
2. 选择 **FlashDesk → My Mac**。
3. 按 **⌘R** 构建运行。

Xcode 会解析 Sparkle 包依赖、编译 Rust 静态库、生成 UniFFI 绑定并链接应用。首次构建需要下载依赖。

也可以从终端运行：

```sh
./script/build_and_run.sh
```

### Windows

需要 Visual Studio 2026（Windows 应用开发与 C++ 桌面开发工具）、.NET 10 和 Rust MSVC 工具链。打开 `windows/FlashDesk.sln`，选择 `x64` 或 `ARM64` 后生成运行；MSBuild 会编译 Rust 动态库并生成 C# 绑定。

## 架构

```text
macos/
  FlashDesk/                SwiftUI 应用
    App/                    应用入口与菜单
    Views/                  原生界面
    Stores/                 应用状态与操作流程
    Services/               Rust 调用与 Sparkle 更新服务
    Models/ Support/        数据模型与辅助功能
    Generated/              UniFFI 生成的 Swift / C 绑定
    Resources/              应用元数据、图标与许可证
  FlashDeskTests/           应用集成与更新回归测试
  FlashDesk.xcodeproj/      Xcode 工程与 Swift 包锁文件
  project.yml               XcodeGen 工程定义
windows/
  FlashDesk/                WinUI 3 应用
    Views/ Stores/          原生界面与应用状态
    Services/               Rust 调用、运行时引导、设置与 Velopack 更新
    Generated/              Interoptopus 生成的 C# 绑定
    Build/                  MSBuild 中的 Rust 构建、绑定生成与发布资源
  Tests/                    更新验证与 Rust/C# 桥接测试
  UpdateTool/               更新密钥生成、签名与发布校验
  scripts/                  安装包构建与发布
  FlashDesk.sln             Visual Studio 解决方案
crates/
  flashdesk-core/           固件分析、目标连接、烧录、内存与存储
  flashdesk-bindgen/        UniFFI 绑定生成工具
  flashdesk-csharp/         Windows 的 Interoptopus 导出层
  flashdesk-csharp-bindgen/ C# 绑定生成工具
script/                     构建、运行、测试与发布工具
```

Rust Core 提供类型化 API、结构化错误和进度事件。硬件操作串行执行，阻塞操作由后台队列调用。

修改 Rust API 后，Xcode 构建会重新生成绑定。依赖版本由 `Cargo.lock` 和 `Package.resolved` 固定。调整工程结构时，在 `macos/` 下运行 `xcodegen generate` 并同步生成的工程。

## 测试

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
./script/test_backend.sh
./script/test_updates.sh
xcodebuild -project macos/FlashDesk.xcodeproj -scheme FlashDesk \
  -configuration Debug -destination 'platform=macOS' \
  -derivedDataPath .build/Xcode test
```

- `test_backend.sh` 运行 Rust 单元测试和独立的 Swift/UniFFI 集成测试，不启动应用或连接硬件。
- `test_updates.sh` 验证更新包签名、文件长度、版本、下载地址，以及发布回退保护。
- Xcode 测试覆盖应用集成、窗口回归、Sparkle 配置和硬件操作期间的重启保护。

CI 在 Apple Silicon 和 Intel 上执行检查，并保存 Xcode 测试报告。Windows 的测试与 CI 见 [windows/README.md](windows/README.md#测试)。

## 自动更新

以下为 macOS；Windows 的行为相同，细节见 [windows/README.md](windows/README.md#自动更新)。

从 **FlashDesk → 检查更新…** 或设置的“关于”页面手动检查。默认每 6 小时自动检查；自动检查和自动下载安装均可在设置中关闭。

Sparkle 验证 Ed25519 签名后安装更新并重新启动应用。硬件操作未完成时，应用会延后更新重启。

更新源使用 GitHub Release 中的 `update-feed/appcast.xml`。首次发布前没有 appcast，检查更新会提示暂时无法获取更新信息。

## 发布

1. 同步 `Cargo.toml`、`macos/project.yml` 和 `windows/FlashDesk/FlashDesk.csproj` 中的版本，在 `macos/` 下运行 `xcodegen generate`。
2. GitHub 仓库的 `release` 环境需要 `SPARKLE_PRIVATE_ED_KEY` 与 `WINDOWS_UPDATE_PRIVATE_KEY` secret，分别对应工程内的 Sparkle 公钥和 Windows 更新公钥（均已配置）。
3. 创建与版本一致的标签，例如 `v0.1.0`，并发布 GitHub Release。

`macOS` 与 `Windows` 两个工作流各自构建并上传本平台的安装包，再更新 `update-feed` 中本平台的更新信息。

CI 通过全部检查后，构建 Apple Silicon + Intel universal 应用，生成 `FlashDesk-<版本号>-macOS-universal.zip` 和签名 appcast。版本包上传完成后，才会更新 `update-feed`；流程会拒绝版本回退或覆盖内容不同的已发布安装包。

应用使用 ad-hoc 代码签名，更新包使用独立的 Sparkle Ed25519 签名。FlashDesk 的本机签名密钥保存在登录钥匙串的 `com.xiaoyu.flashdesk` 账户下；私钥不属于项目文件。

本机已构建 Release 应用时，可直接打包并验证：

```sh
./script/package_release.sh
```

打包产物位于 `.build/ReleaseAssets/`。`--publish` 模式由 CI 使用。

## 许可证

本项目采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 双许可证。

第三方依赖说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
