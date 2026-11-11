# ChipDock for Linux

ChipDock 提供固件烧录、芯片与内存查看、串口调试、协议模板、工作流、采集和曲线工具。界面采用顶部工作区切换、可拖动分栏和独立工具窗口，支持浅色、深色与系统主题。

## 安装

在 [发布页面](https://github.com/tangxiangong/chipdock/releases) 下载 `ChipDock-<版本>-linux-<架构>` 安装包。Intel／AMD 64 位选择 `x86_64`，ARM64 选择 `aarch64`。

| 格式 | 安装方式 |
| --- | --- |
| `.deb` | Debian／Ubuntu：`sudo apt install ./ChipDock-<版本>-linux-<架构>.deb` |
| `.rpm` | Fedora 等 RPM 发行版：`sudo dnf install ./ChipDock-<版本>-linux-<架构>.rpm` |
| `.tar.gz` | 解压后进入目录，运行 `bash install.sh`，安装到当前用户的 `~/.local/bin/ChipDock` 与 XDG 数据目录 |

三种格式都包含应用图标和桌面入口，安装后从应用菜单打开 ChipDock。系统包的启动命令为 `ChipDock`，DEB 的包管理器登记名为 `chipdock`，RPM 的登记名为 `ChipDock`。便携包的桌面入口使用绝对路径，桌面会话无需额外配置 PATH。DEB／RPM 不会安装驱动或修改串口用户组。

运行需要 glibc 2.39 或更新版本、图形桌面、可用的 Vulkan 驱动，以及发行版的 Wayland 或 X11 运行库；Ubuntu 24.04 满足 glibc 基线。DEB／RPM 会声明运行库依赖。便携包需自行安装对应运行库。请勿以 root 身份运行桌面应用。

Release 中无扩展名的 `ChipDock-<版本>-linux-<架构>` 文件供便携版应用内更新使用。首次安装请选择上述安装包。

## 设备访问

串口设备通常需要当前用户属于 `dialout` 组；不同发行版可能使用 `uucp` 等其他组。修改组成员后重新登录。

调试探针需要匹配的 udev 规则。参照 [probe-rs 的设备权限说明](https://probe.rs/docs/getting-started/probe-setup/)，只为调试设备配置访问权限。不要修改磁盘、HID 或串口接口的驱动。

Windows 版的“检查／修复所选探针驱动”使用 Rust 调用系统 WinUSB 安装接口，在系统 UAC 对话框中确认后运行。无需 .NET 辅助程序。

## 使用

- **固件烧录**：选择或拖入 ELF、AXF、HEX、BIN，点击目标旁的“更改…”选择探针与芯片，并设置 SWD／JTAG、速度和 BIN 地址；写入选项位于底部。分析固件不访问硬件；连接、预检查与烧录会访问目标。
- **目标内存**：读取指定地址与长度，以 HEX／ASCII 查看并导出 BIN。
- **串口调试**：在左侧配置通信参数，右侧查看收发记录，下方输入多行发送内容并选择编码、行尾与校验。可拖动分隔线或收起连接区和发送区。终端支持过滤、暂停、复制和发送历史；记录页支持 JSON 导出与正则着色规则。
- **高级串口功能**：包库、文件分块发送、宏、自动应答、协议模板、Modbus RTU、接收分帧、日志轮转、原始采集与实时曲线。通过发送区的“工具”菜单打开相应面板；JSON 编辑器提供导入、导出和保存。曲线支持样例试算、字段筛选与 CSV 导出。
- **本地控制**：在串口会话栏的“本地控制”菜单中选择关闭、只读或允许控制，供仓库中的 CLI／MCP 客户端访问串口会话。
- **配置与历史**：保存和应用烧录配置，重新打开最近固件。

内存、日志和曲线可以在独立窗口打开；不同会话的曲线可同时采样。终端和记录页共享搜索与方向过滤，支持按原始序号定位记录。

快捷键在 macOS 使用 ⌘，其他平台使用 Ctrl：`R` 刷新、`K` 连接／断开、`Shift+K` 断开、`Enter` 烧录／发送、`Shift+R` 读取内存、`L` 清空终端、`F` 查找、`,` 打开设置。

设置保存在 Linux 的 `$XDG_CONFIG_HOME/ChipDock/GPUI/preferences.json`（默认 `~/.config/ChipDock/GPUI/preferences.json`）。macOS 使用 `~/Library/Application Support/ChipDock/GPUI/`，Windows 使用 `%APPDATA%\ChipDock\GPUI\`。

## 更新

在“设置”中检查更新，也可以开启每天自动检查、自动下载，以及空闲时安装重启。

Linux 使用签名更新清单，覆盖便携更新文件及 DEB、RPM、压缩包的长度和 SHA-256。客户端先访问 COS，更新源不可用时回退到 GitHub。安装前验证 Ed25519 签名、应用身份、平台、版本、文件长度与 SHA-256。取消下载不会改写当前程序，活动串口连接或任务会阻止安装。便携程序必须位于当前用户可写的目录。

DEB／RPM 安装版可以检查新版本；请下载新版安装包，再用相同的包管理器安装。此类安装关闭应用内二进制替换与自动下载安装，保留包管理器的文件所有权。这里提供的是 Release 附件，不是 APT／DNF 软件仓库。

如果已安装重发前的 Linux v0.3.0，请下载对应的 `ChipDock` 安装包重新安装。便携版可重新运行 `bash install.sh`。重新安装会保留用户设置。

源码运行的 macOS／Windows GPUI 客户端只接受各自平台的 GPUI 文件；没有对应发布文件时会明确报错。应用不会将其替换成现有原生客户端。打包进 macOS `.app` 的程序需要完整应用包更新，不能使用便携二进制替换。

构建、检查和发布维护说明见仓库根目录的 [AGENTS.md](../AGENTS.md)。
