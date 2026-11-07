# ChipDock for Windows

适用于 Windows 10 1809（内部版本 17763）及更新版本，支持 x64 和 ARM64。

## 下载与安装

1. 在 Windows「设置 → 系统 → 关于」查看系统类型。
2. 打开 [ChipDock 最新版本](https://github.com/tangxiangong/chipdock/releases/latest)，在 **Assets** 中选择对应架构的 EXE 或 MSI 安装包。

   | 系统类型 | 安装包 |
   | --- | --- |
   | x64 | `ChipDock-<版本>-windows-x64.exe` 或 `ChipDock-<版本>-windows-x64.msi` |
   | ARM64 | `ChipDock-<版本>-windows-arm64.exe` 或 `ChipDock-<版本>-windows-arm64.msi` |

3. 运行安装包，然后从开始菜单打开 ChipDock。首次启动时，若缺少兼容的 Windows App Runtime，ChipDock 会下载并安装，请保持联网。

## 常用操作

用顶部导航或 `Ctrl+1`、`Ctrl+2` 在「固件烧录」和「串口连接」之间切换。切换时烧录和串口收发会继续。

| 快捷键 | 操作 |
| --- | --- |
| `Ctrl+O` | 打开固件 |
| `Ctrl+R` | 扫描探针或刷新串口列表 |
| `Ctrl+K` | 连接目标或切换串口连接状态 |
| `Ctrl+Shift+K` | 断开目标或串口 |
| `Ctrl+Enter` | 执行当前烧录操作或发送串口数据 |
| `Ctrl+J` / `Ctrl+Alt+M` | 在烧录页打开日志或内存面板 |
| `Ctrl+,` | 打开设置 |

## 探针与串口驱动

连接调试探针时，ChipDock 会检查选中的 USB 调试接口。需要安装或修复 WinUSB 时，Windows 会弹出管理员授权提示。按提示授权后，等待设备重新识别。

- 自动修复覆盖常见的 ST-Link、J-Link、CMSIS-DAP v2、FTDI、WCH-Link、CH347、ESP USB-JTAG 和已枚举的 Glasgow 调试接口。ESP USB-JTAG 的调试接口是 `MI_02`，同一设备的 CDC 串口仍通过 COM 端口使用。
- 多个同型号探针无法区分时，暂时断开其他同型号探针，再重新扫描。如果安装后提示重启，请重启再连接。需要手动处理调试驱动时，参阅 [probe-rs 探针配置指南](https://probe.rs/docs/getting-started/probe-setup/)。
- CMSIS-DAP v1 使用 HID。USB 转串口、Black Magic Probe 和 SiFli UART 使用串口驱动。找不到 COM 端口时，先在设备管理器确认设备是否出现，再通过 Windows Update 或设备厂商安装相应驱动。

## 检查更新

在「帮助 → 检查更新…」手动检查，或在设置的「更新」中调整自动检查和自动下载安装。默认每 6 小时检查一次，后台下载的更新会在退出应用时安装。更新选项暂不可用时，先结束硬件操作并断开串口。

## 排查启动问题

如果 ChipDock 在打开窗口前退出，可查看 `%LOCALAPPDATA%\ChipDock\logs\crash.log`。目标配置和最近使用记录位于 `%APPDATA%\ChipDock`，应用设置位于 `%LOCALAPPDATA%\ChipDock\settings.json`。

更多操作见 [ChipDock 使用指南](../README.md) 和 [串口命令与接收解析](../docs/serial-protocol.md)。
