# ChipDock v0.2.3

本版新增串口命令模板与接收解析，并改善 macOS 和 Windows 的串口操作体验。

## 本次更新

- **串口命令模板**：支持命名命令、参数化帧编码、发送预览和历史记录，可通过拖入帧片段快速编辑命令。
- **接收解析与反馈**：支持接收缓冲解析、校验和验证及命令反馈跟踪，保留原始接收记录；协议规则可持久保存。
- **Windows 串口界面**：优化协议编辑器与发送区布局。
- **macOS 性能**：修复 macOS 27.2 下工具栏布局导致的空闲卡顿。
- **串口依赖**：更新底层串口库。

<!-- platform:macos -->
## macOS

- 下载 `ChipDock-0.2.3-macOS-universal.zip`，解压后将 `ChipDock.app` 拖入“应用程序”。适用于 macOS 14 或更新版本，支持 Apple Silicon 与 Intel。

应用采用 ad-hoc 代码签名，未经过 Apple 公证；自动更新包通过 Sparkle Ed25519 签名验证。
<!-- /platform -->

<!-- platform:windows -->
## Windows

- x64：`ChipDock-0.2.3-windows-x64.exe` 或 `ChipDock-0.2.3-windows-x64.msi`。
- ARM64：`ChipDock-0.2.3-windows-arm64.exe` 或 `ChipDock-0.2.3-windows-arm64.msi`。

适用于 Windows 10 1809 或更新版本。请使用与系统架构一致的安装程序；`ChipDock-0.2.3-win-<arch>-full.nupkg` 供应用内自动更新使用。

安装包未使用代码签名证书；自动更新验证 ECDSA P-256 签名、文件大小和 SHA-256。
<!-- /platform -->

## 升级说明

v0.2.1 和 v0.2.2 用户可通过应用内自动更新升级。v0.2.0 及更早版本仍使用旧安装身份，请先退出并卸载旧版，再使用本版安装包重新安装；Windows 用户还需重新固定任务栏中的 ChipDock。
