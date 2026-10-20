## ChipDock 改名过渡版本

- FlashDesk 正式更名为 ChipDock（芯坞），定位为嵌入式开发工具集；本版保留现有固件烧录、目标连接和内存功能。
- 更新应用名称与双向传输图标，沿用原有蓝、青、绿色配色。
- 增加 `tangxiangong/chipdock` 更新源，保留 `tangxiangong/flashdesk`，支持 GitHub 仓库改名前后持续更新。
- 保留原安装标识、签名验证密钥和配置目录，已有用户可直接升级，无需卸载。

<!-- platform:macos -->
### macOS
- 应用改名为 ChipDock.app，启动时优先选择新更新源，未改名时继续使用旧源。
- 保留 Sparkle 更新签名验证与硬件操作期间的安装保护。
<!-- /platform -->

<!-- platform:windows -->
### Windows
- 安装程序与应用界面显示 ChipDock，兼容现有 FlashDesk 安装与设置。
- 更新检查及安装包下载支持新旧仓库，继续严格验证签名、文件大小和 SHA-256。
<!-- /platform -->
