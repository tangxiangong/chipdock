# ChipDock v0.2.2

本版改进 macOS 与 Windows 的任务状态、工具窗口标题和串口发送区操作。

## 本次更新

- **任务进度与完成状态**：全片擦除等没有百分比的任务显示持续活动的进度条；擦除、烧录和预检完成后显示明确的完成标记。开始新任务时清除上一次任务状态。
- **工具窗口标题**：窗口标题跟随当前选择的“固件烧录”或“串口连接”，选择固件后仍保留工具名称。
- **串口发送区**：将展开与收起按钮移到接收记录下方，与滚动、导出和清空操作并列；使用明确的文字和方向箭头，并改善窄窗口下的排列。

<!-- platform:macos -->
## macOS

- 任务完成时向辅助功能播报结果。
- 下载 `ChipDock-0.2.2-macOS-universal.zip`，解压后将 `ChipDock.app` 拖入“应用程序”。适用于 macOS 14 或更新版本，支持 Apple Silicon 与 Intel。

应用采用 ad-hoc 代码签名，未经过 Apple 公证；自动更新包通过 Sparkle Ed25519 签名验证。
<!-- /platform -->

<!-- platform:windows -->
## Windows

- x64：`ChipDock-0.2.2-windows-x64.exe` 或 `ChipDock-0.2.2-windows-x64.msi`。
- ARM64：`ChipDock-0.2.2-windows-arm64.exe` 或 `ChipDock-0.2.2-windows-arm64.msi`。

适用于 Windows 10 1809 或更新版本。请使用与系统架构一致的安装程序；`ChipDock-0.2.2-win-<arch>-full.nupkg` 供应用内自动更新使用。

安装包未使用代码签名证书；自动更新验证 ECDSA P-256 签名、文件大小和 SHA-256。
<!-- /platform -->

## 升级说明

v0.2.1 用户可通过应用内自动更新升级。v0.2.0 及更早版本仍使用旧安装身份，请先退出并卸载旧版，再使用本版安装包重新安装；Windows 用户还需重新固定任务栏中的 ChipDock。
