# ChipDock for Linux

ChipDock 的 GPUI 客户端提供固件烧录、芯片与内存查看、串口调试、协议模板、工作流、采集和曲线工具。界面采用侧栏、工具栏和独立工作区，支持浅色、深色与系统主题。

同一套 Rust／GPUI 源码适用于 Linux、Windows 和 macOS。正式发布流程将它作为 Linux 客户端构建；已有 macOS 和 Windows 原生客户端仍分别发布。

## 安装

在 [发布页面](https://github.com/tangxiangong/chipdock/releases) 查找与你的架构对应的 GPUI 文件：

- Intel／AMD 64 位：`chipdock-gpui-linux-x86_64`
- ARM64：`chipdock-gpui-linux-aarch64`

如果当前版本没有这些文件，说明该版本尚未发布 GPUI 客户端。不要使用 macOS ZIP 或 Windows 安装包替代。

下载后赋予执行权限，将它放入当前用户可写的目录，例如：

```sh
mkdir -p ~/.local/bin
install -m 755 chipdock-gpui-linux-x86_64 ~/.local/bin/chipdock-gpui
~/.local/bin/chipdock-gpui
```

运行需要图形桌面、可用的 Vulkan 驱动，以及发行版的 Wayland 或 X11 运行库。建议使用 Ubuntu 24.04 或更新版本。请勿以 root 身份运行桌面应用。

可将本目录的 `ChipDock.desktop` 放入 `~/.local/share/applications/`；请确保 `chipdock-gpui` 在桌面会话的 PATH 中。

## 设备访问

串口设备通常需要当前用户属于 `dialout` 组；不同发行版可能使用 `uucp` 等其他组。修改组成员后重新登录。

调试探针需要匹配的 udev 规则。参照 [probe-rs 的设备权限说明](https://probe.rs/docs/getting-started/probe-setup/)，只为调试设备配置访问权限。不要修改磁盘、HID 或串口接口的驱动。

Windows 版的“检查／修复所选探针驱动”使用 Rust 调用系统 WinUSB 安装接口，在系统 UAC 对话框中确认后运行。无需 .NET 辅助程序。

## 使用

- **固件烧录**：选择或拖入 ELF、AXF、HEX、BIN，选择探针与芯片，设置 SWD／JTAG、速度、复位、擦除与校验选项。分析固件不访问硬件；连接、预检查与烧录会访问目标。
- **目标内存**：读取指定地址与长度，以 HEX／ASCII 查看并导出 BIN。
- **串口调试**：创建多个会话，配置通信参数，选择编码、行尾与校验。终端支持过滤、暂停、复制和发送历史；记录页支持 JSON 导出与正则着色规则。
- **高级串口功能**：包库、文件分块发送、宏、自动应答、协议模板、Modbus RTU、接收分帧、日志轮转、原始采集与实时曲线。JSON 编辑器提供导入、导出和保存。曲线支持样例试算、字段筛选与 CSV 导出。
- **本地控制**：在设置中选择关闭、只读或允许控制，供仓库中的 CLI／MCP 客户端访问串口会话。
- **配置与历史**：保存和应用烧录配置，重新打开最近固件。

内存、日志和曲线可以在独立窗口打开；不同会话的曲线可同时采样。终端和记录页共享搜索与方向过滤，支持按原始序号定位记录。

快捷键在 macOS 使用 ⌘，其他平台使用 Ctrl：`R` 刷新、`K` 连接／断开、`Shift+K` 断开、`Enter` 烧录／发送、`Shift+R` 读取内存、`L` 清空终端、`F` 查找、`,` 打开设置。

设置保存在 Linux 的 `$XDG_CONFIG_HOME/ChipDock/GPUI/preferences.json`（默认 `~/.config/ChipDock/GPUI/preferences.json`）。macOS 使用 `~/Library/Application Support/ChipDock/GPUI/`，Windows 使用 `%APPDATA%\ChipDock\GPUI\`。

## 更新

在“设置”中检查更新，也可以开启每天自动检查、自动下载，以及空闲时安装重启。

GPUI 使用独立的签名更新清单。客户端先访问 COS，网络请求失败时回退到 GitHub；验证 Ed25519 签名、应用身份、平台、版本、文件长度与 SHA-256 后才能安装。取消下载不会改写当前程序；活动串口连接或任务会阻止安装。便携程序必须位于当前用户可写的目录。

源码运行的 macOS／Windows GPUI 客户端只接受各自平台的 GPUI 文件；没有对应发布文件时会明确报错。应用不会将其替换成现有原生客户端。打包进 macOS `.app` 的程序需要完整应用包更新，不能使用便携二进制替换。

构建、检查和发布维护说明见仓库根目录的 [AGENTS.md](../AGENTS.md)。
