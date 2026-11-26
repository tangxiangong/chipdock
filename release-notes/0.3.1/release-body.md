# ChipDock v0.3.1

## 本次更新

- 上次扫描未发现调试探针时，插入探针后可直接点击“连接”，应用会重新扫描。
- 拔出已连接的 USB 调试探针时，应用会提示并清除目标连接状态，保留已打开的固件。
- 记住擦除方式、烧录后校验和复位等烧录选项，下次使用时自动恢复。

<!-- platform:macos -->
## macOS

- `ChipDock-0.3.1-macOS-universal.zip` 同时支持 Apple 芯片和 Intel，最低系统版本为 macOS 14。
<!-- /platform -->

<!-- platform:windows -->
## Windows

- 按系统架构选择 `ChipDock-0.3.1-windows-x64` 或 `ChipDock-0.3.1-windows-arm64` 对应的 EXE 或 MSI 安装包。
<!-- /platform -->

<!-- platform:linux -->
## Linux

- 工作台迁移到 Native Shell，使用 GTK 4.14+ 和 GStreamer 基础运行库。支持 Wayland 和 X11 图形桌面。
- 后续签名的界面更新可在后台下载，并在应用下次正常启动时生效。便携版支持签名整包更新，DEB 和 RPM 通过系统包管理器更新。
- `ChipDock-0.3.1-linux-<架构>` 提供 DEB、RPM 和 `.tar.gz` 三种格式。Intel／AMD 64 位选择 `x86_64`，ARM64 选择 `aarch64`。
<!-- /platform -->
