import SwiftUI

struct WorkbenchCommands: Commands {
    var toolkit: ToolkitState
    var store: WorkbenchStore
    var serial: SerialStore
    var updater: UpdateService
    private var serialIsActive: Bool { toolkit.selectedTool == .serial }
    @FocusedValue(\.serialSearchRequested) private var searchRequested

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("打开固件…", systemImage: "doc.badge.plus") { toolkit.selectedTool = .flash; store.showImporter = true }
                .keyboardShortcut("o")
                .disabled(store.isBusy)
            Menu("打开最近使用的") {
                ForEach(store.recentFiles, id: \.path) { file in
                    let isMissing = store.missingRecentPaths.contains(file.path)
                    Button {
                        toolkit.selectedTool = .flash
                        store.openRecent(file)
                    } label: {
                        Text(URL(fileURLWithPath: file.path).lastPathComponent + (isMissing ? "（文件不存在）" : ""))
                    }
                    .disabled(isMissing)
                }
                Divider()
                Menu("移除记录") {
                    ForEach(store.recentFiles, id: \.path) { file in
                        Button(URL(fileURLWithPath: file.path).lastPathComponent) {
                            Task { await store.removeRecentFiles(at: [file.path]) }
                        }
                    }
                }
                Button("清除失效记录") { Task { await store.clearMissingRecentFiles() } }
                    .disabled(store.missingRecentPaths.isEmpty)
                Button("清空最近使用") { Task { await store.clearRecentFiles() } }
            }
            .disabled(store.isBusy || store.recentFiles.isEmpty)
        }
        CommandGroup(after: .appInfo) {
            Button("检查更新…", action: updater.checkForUpdates)
                .disabled(!updater.canCheckForUpdates)
        }
        CommandMenu("目标") {
            if serialIsActive {
                Button("刷新串口", systemImage: "arrow.clockwise") { Task { await serial.refreshPorts() } }
                    .keyboardShortcut("r").disabled(serial.refreshing)
                Button(serial.status.connected ? "断开串口" : "连接串口", systemImage: "cable.connector") { Task { await serial.toggleConnection() } }
                    .keyboardShortcut("k").disabled(serial.busy || (!serial.status.connected && serial.preferences.selectedPath.isEmpty))
                Button("断开串口") { Task { if serial.status.connected { await serial.toggleConnection() } } }
                    .keyboardShortcut("k", modifiers: [.command, .shift]).disabled(!serial.status.connected || serial.busy)
            } else {
                Button("目标配置…", systemImage: "point.3.connected.trianglepath.dotted") { store.showTargetSetup = true }
                Divider()
                Button("扫描探针", systemImage: "arrow.clockwise") { Task { await store.scanProbes() } }
                    .keyboardShortcut("r")
                    .disabled(store.isBusy)
                Button("连接目标", systemImage: "link") { Task { await store.connect() } }
                    .keyboardShortcut("k")
                    .disabled(!store.canConnect)
                Button("断开目标") { store.disconnect() }
                    .keyboardShortcut("k", modifiers: [.command, .shift])
                    .disabled(!store.canDisconnect)
                Divider()
                Button("选择芯片…") { store.showChipPicker = true }
                    .disabled(store.isBusy)
                Button("保存配置…") { store.showSaveProfile = true }
                    .keyboardShortcut("s", modifiers: [.command, .shift])
                    .disabled(store.isBusy)
                if !store.profiles.isEmpty {
                    Menu("载入配置") {
                        ForEach(store.profiles) { profile in
                            Button(profile.name) { store.applyProfile(profile) }
                        }
                    }
                    .disabled(store.isBusy)
                }
            }
        }
        CommandMenu("操作") {
            if serialIsActive {
                Button("发送数据", systemImage: "paperplane") { Task { await serial.send() } }
                    .keyboardShortcut(.return, modifiers: .command).disabled(!serial.canSend)
                Button("清空终端", systemImage: "trash") { serial.clear() }
                    .keyboardShortcut("l").disabled(serial.frames.isEmpty)
                Button("搜索数据", systemImage: "magnifyingglass") { searchRequested?.wrappedValue = true }
                    .keyboardShortcut("f").disabled(searchRequested == nil)
            } else {
                Button(store.dryRun ? "开始预检查" : "烧录固件", systemImage: "arrow.down.to.line") { Task { await store.flash() } }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(!store.canFlash)
                Button("读取内存") { store.showMemory = true; Task { await store.readMemory() } }
                    .keyboardShortcut("r", modifiers: [.command, .shift])
                    .disabled(!store.canReadMemory)
                Divider()
                Button("擦除整个芯片…", role: .destructive) { store.confirmErase = true }
                    .disabled(!store.canErase)
            }
        }
        CommandGroup(after: .toolbar) {
            Button("固件烧录", systemImage: "cpu") { toolkit.selectedTool = .flash }.keyboardShortcut("1")
            Button("串口终端", systemImage: "terminal") { toolkit.selectedTool = .serial }.keyboardShortcut("2")
            Divider()
            Button(store.showLogs ? "隐藏日志" : "显示日志", systemImage: "text.alignleft") { toolkit.selectedTool = .flash; store.showLogs.toggle() }
                .keyboardShortcut("j")
            Button(store.showMemory ? "隐藏内存" : "显示内存", systemImage: "memorychip") { toolkit.selectedTool = .flash; store.showMemory.toggle() }
                .keyboardShortcut("m", modifiers: [.command, .option])
            Divider()
        }
        CommandGroup(replacing: .help) {
            Link("ChipDock 源代码", destination: URL(string: "https://github.com/tangxiangong/chipdock")!)
            Link("probe-rs 文档", destination: URL(string: "https://probe.rs/docs/")!)
        }
    }
}
