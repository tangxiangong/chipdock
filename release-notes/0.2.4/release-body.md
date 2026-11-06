# ChipDock v0.2.4

本版新增 ESP 芯片的 probe-rs 烧录支持、改进 BIN 地址识别，并完善 Windows USB-JTAG 驱动修复。

## 本次更新

- **ESP 烧录支持**：接入 Espressif 目标与 USB-JTAG，自动使用 JTAG 协议，并通过 IDF 加载器处理 ESP ELF 固件。
- **BIN 地址识别**：从构建清单、有效分区表和引导镜像识别烧录偏移，拒绝不兼容或截断的 ESP 镜像。
- **目标配置**：macOS 和 Windows 的 BIN 地址配置统一移入目标设置，恢复自动地址时清除旧的手动覆盖值。
- **烧录稳定性**：延长 ESP 压缩页写入超时，改善包含大段擦除区域的合并镜像烧录。
- **Windows 驱动修复**：支持 ESP USB-JTAG 的 WinUSB 安装与修复，仅处理 JTAG 接口，保留 CDC 串口接口。

<!-- platform:macos -->
## macOS

- 下载 `ChipDock-0.2.4-macOS-universal.zip`，解压后将 `ChipDock.app` 拖入“应用程序”。适用于 macOS 14 或更新版本，支持 Apple Silicon 与 Intel。

应用采用 ad-hoc 代码签名，未经过 Apple 公证；自动更新包通过 Sparkle Ed25519 签名验证。
<!-- /platform -->

<!-- platform:windows -->
## Windows

- x64：`ChipDock-0.2.4-windows-x64.exe` 或 `ChipDock-0.2.4-windows-x64.msi`。
- ARM64：`ChipDock-0.2.4-windows-arm64.exe` 或 `ChipDock-0.2.4-windows-arm64.msi`。

适用于 Windows 10 1809 或更新版本。请使用与系统架构一致的安装程序；`ChipDock-0.2.4-win-<arch>-full.nupkg` 供应用内自动更新使用。

安装包未使用代码签名证书；自动更新验证 ECDSA P-256 签名、文件大小和 SHA-256。
<!-- /platform -->

## 升级说明

v0.2.1 至 v0.2.3 用户可通过应用内自动更新升级。v0.2.0 及更早版本仍使用旧安装身份，请先退出并卸载旧版，再使用本版安装包重新安装；Windows 用户还需重新固定任务栏中的 ChipDock。
