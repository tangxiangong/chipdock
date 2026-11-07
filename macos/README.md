# ChipDock for macOS

适用于 macOS 14 或更新版本，支持 Apple 芯片和 Intel Mac。

## 安装

1. 打开 [GitHub 发布页](https://github.com/tangxiangong/chipdock/releases/latest)，在 **Assets** 中下载 `ChipDock-<版本>-macOS-universal.zip`。
2. 解压，把 `ChipDock.app` 拖到「应用程序」，再从那里打开。
3. 若首次打开时 macOS 提示无法验证应用，请确认文件来自项目发布页，再到「系统设置 → 隐私与安全性」为 ChipDock 选择「仍要打开」。[Apple 首次打开应用说明](https://support.apple.com/zh-cn/102445)

## 常用快捷键

| 操作 | 快捷键 |
| --- | --- |
| 切换到「固件烧录」 | ⌘1 |
| 切换到「串口连接」 | ⌘2 |
| 打开固件 | ⌘O |
| 扫描探针或刷新串口 | ⌘R |
| 连接目标或切换串口连接状态 | ⌘K |
| 烧录、开始预检查或发送串口数据 | ⌘Return |

固件烧录与串口连接可以同时使用。操作步骤见 [ChipDock 使用指南](../README.md)，命令模板和接收规则见 [串口命令与接收解析](../docs/serial-protocol.md)。

## 找不到探针或无法连接

1. 检查 USB 线是否支持数据传输，重新插拔探针，再扫描。
2. 使用搭载 Apple 芯片的 Mac 笔记本时，先解锁 Mac，在系统弹窗中允许 USB 配件连接。[Apple USB 配件连接说明](https://support.apple.com/zh-cn/102282)
3. 如果提示接口被占用，关闭正在使用探针的 IDE、OpenOCD 或厂商工具，再重试。若仍被占用，检查是否有 USB 或 FTDI 驱动绑定了调试接口。

## 应用内更新

在「ChipDock → 检查更新…」或「设置 → 关于」手动检查。应用默认每 6 小时自动检查一次，可在「关于」分别调整自动检查和自动下载并安装。如果更新入口暂时不可用，请等硬件操作结束并断开串口后再试。
