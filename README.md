# ChipDock

ChipDock 是面向嵌入式开发的桌面工具。用调试探针烧录固件、擦除 Flash 或读取芯片内存，同时通过串口收发数据和解析设备协议。

## 下载与安装

在 [GitHub Releases](https://github.com/tangxiangong/chipdock/releases/latest) 的 **Assets** 中选择安装包。

| 平台                         | 系统要求                                      | 安装包                                                                        |
| ---------------------------- | --------------------------------------------- | ----------------------------------------------------------------------------- |
| [macOS](macos/README.md)     | macOS 14 或更新版本，支持 Apple 芯片和 Intel  | `ChipDock-<版本>-macOS-universal.zip`，解压后将 `ChipDock.app` 拖入“应用程序” |
| [Windows](windows/README.md) | Windows 10 1809 或更新版本，支持 x64 和 ARM64 | `ChipDock-<版本>-windows-<架构>.exe` 或 `.msi`，选择与系统架构一致的安装包    |
| [Linux](linux/README.md) | glibc 2.39+、图形桌面与 Vulkan，支持 x86_64 和 ARM64 | `chipdock-gpui-<版本>-linux-<架构>.deb`、`.rpm` 或 `.tar.gz` |

平台指南包含应用内更新入口和探针连接说明。

[Linux／跨平台 GPUI 客户端](linux/README.md) 提供安装和更新说明。Linux 安装包包含应用图标与桌面入口；旧版本可能没有 Linux 附件。

## 烧录固件

1. 打开“固件烧录”，拖入或打开 ELF、AXF、Intel HEX 或 BIN 文件。使用自定义引导程序或分区表的 ESP-IDF 项目请选择构建生成的合并 BIN。
2. 在“目标配置”中选择探针和芯片，按需设置 SWD 或 JTAG、连接速度及复位下连接。烧录 BIN 时，可将“BIN 基地址”留空尝试自动识别。识别失败时填入构建配置中的 Flash 偏移。
3. 连接目标，核对固件的写入范围，确认擦除、写入校验和复位选项后开始烧录。可先运行预检查来核对连接、地址和 Flash 映射。预检查期间可能暂停目标。

在“操作”菜单中也可执行整片擦除或读取内存。内存视图支持查看芯片的内存映射，按地址读取、查看十六进制与 ASCII，以及导出二进制。

## 串口连接

1. 打开“串口连接”，按设备要求设置端口和通信参数。用标签栏的加号并行连接多台设备。也可分别连接 `VIRTUAL-A` 和 `VIRTUAL-B`，在应用内互相收发。
2. 普通发送支持 HEX、ASCII、GBK、UTF-8 和 Latin-1。发送区可回填历史、追加校验、定时发送，或切换到命令模板。
3. 终端支持暂停显示、正则搜索和着色。记录表可定位到原始记录，并导出 JSON、TXT、CSV 或 BIN。持续采集可独立保存原始 RX。
4. 在发送区的“工具”打开 Modbus RTU、包库、文件发送、宏、自动应答和实时曲线。需要外部访问时，在会话标签栏打开“本地控制”。

用 macOS 的 `⌘1` / `⌘2` 或 Windows 的 `Ctrl+1` / `Ctrl+2` 在烧录和串口工具之间切换，两个工具可同时运行。

## 许可证

项目采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 双许可证。第三方依赖说明见 [NOTICE](NOTICE)。
