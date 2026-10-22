# ChipDock v0.2.1

本版统一 macOS 与 Windows 的 ChipDock 安装身份，修复旧名称在安装位置、快捷方式和系统应用列表中残留的问题，并增加更新主备源与串口记录导出。

## 安装说明

此前发布的版本（包括 v0.2.0）仍使用 FlashDesk 安装身份。本版不再兼容该身份，也不自动迁移旧设置。请先退出并卸载旧版，再使用本版安装包重新安装；Windows 用户还需移除旧的任务栏固定项，再固定 ChipDock。后续 ChipDock 版本之间可正常原位升级。

## 本次更新

- **统一名称**：macOS 使用 `com.xiaoyu.chipdock` 和 `ChipDock.app`；Windows 的包 ID、安装程序、快捷方式、系统应用名称与配置目录统一为 ChipDock。打包时校验应用身份，拒绝旧名称安装包。
- **更新主备源**：优先使用腾讯云 COS，传输失败时尝试 GitHub 备用源；保留签名、大小和哈希验证。新身份使用独立更新源，避免读取旧身份的安装包和更新状态。
- **串口记录导出**：可将当前保留的全部收发记录导出为格式化 JSON，包含时间、方向、原始字节、HEX 与各编码文本；导出内容不受当前搜索或方向筛选影响。

<!-- platform:macos -->
## macOS

下载 `ChipDock-0.2.1-macOS-universal.zip`，解压后将 `ChipDock.app` 拖入“应用程序”。适用于 macOS 14 或更新版本，支持 Apple Silicon 与 Intel。

应用采用 ad-hoc 代码签名，未经过 Apple 公证；自动更新包通过 Sparkle Ed25519 签名验证。
<!-- /platform -->

<!-- platform:windows -->
## Windows

- x64：`ChipDock-0.2.1-windows-x64.exe` 或 `ChipDock-0.2.1-windows-x64.msi`。
- ARM64：`ChipDock-0.2.1-windows-arm64.exe` 或 `ChipDock-0.2.1-windows-arm64.msi`。

适用于 Windows 10 1809 或更新版本。请使用与系统架构一致的安装程序；`ChipDock-0.2.1-win-<arch>-full.nupkg` 供应用内自动更新使用。

安装包未使用代码签名证书；自动更新验证 ECDSA P-256 签名、文件大小和 SHA-256。
<!-- /platform -->
