import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct WorkbenchCommands: Commands {
    var toolkit: ToolkitState
    var store: WorkbenchStore
    var serial: SerialStore
    var updater: UpdateService
    private var serialIsActive: Bool { toolkit.selectedTool == .serial && memoryActions == nil }
    @FocusedValue(\.serialSearchRequested) private var searchRequested
    @FocusedValue(\.logSearchRequested) private var logSearchRequested
    @FocusedValue(\.logViewActions) private var logActions
    @FocusedValue(\.serialConnectionExpanded) private var connectionExpanded
    @FocusedValue(\.serialComposerExpanded) private var composerExpanded
    @FocusedValue(\.targetSetupActions) private var targetActions
    @FocusedValue(\.logExport) private var logExportAction
    @FocusedValue(\.memoryExport) private var memoryExportAction
    @FocusedValue(\.memoryViewActions) private var memoryActions
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    private var panels: WorkbenchPanelActions {
        WorkbenchPanelActions(store: store, openWindow: openWindow, dismissWindow: dismissWindow)
    }
    private var mainIsModal: Bool {
        store.showTargetSetup || store.showChipPicker || store.showSaveProfile || store.showImporter
    }

    private func activateFlash() {
        toolkit.selectedTool = .flash
        openWindow(id: "toolkit")
    }

    private func exportLogs() {
        if let logExportAction { logExportAction(); return }
        openWindow(id: "toolkit")
        store.requestExport(PendingExport(data: Data(store.logExportText.utf8), contentType: .plainText,
                                          filename: "ChipDock.log", operation: "导出日志"))
    }

    private func exportMemory() {
        if let memoryExportAction { memoryExportAction(); return }
        guard let result = store.memoryResult else { return }
        openWindow(id: "toolkit")
        store.requestExport(PendingExport(data: result.data, contentType: .data,
                                          filename: "memory-\(addressLabel(result.address)).bin", operation: "导出内存"))
    }

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("打开固件…", systemImage: "doc.badge.plus") { activateFlash(); store.showImporter = true }
                .keyboardShortcut("o")
                .disabled(store.isBusy || mainIsModal)
            Menu("打开最近使用的") {
                ForEach(store.recentFiles, id: \.path) { file in
                    let isMissing = store.missingRecentPaths.contains(file.path)
                    Button {
                        activateFlash()
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
            .disabled(store.isBusy || mainIsModal || store.recentFiles.isEmpty)
            Divider()
            Button("关闭固件") { store.clearFirmware() }
                .disabled(store.firmwareURL == nil || store.isBusy || mainIsModal)
            Button("在访达中显示固件") {
                if let url = store.firmwareURL { NSWorkspace.shared.activateFileViewerSelecting([url]) }
            }
            .disabled(store.firmwareURL == nil)
        }
        CommandGroup(after: .importExport) {
            Button("导出日志…", action: exportLogs)
                .disabled(store.logs.isEmpty || (mainIsModal && logExportAction == nil))
            Button("导出内存…", action: exportMemory)
                .disabled(store.memoryResult == nil || store.isBusy || (mainIsModal && memoryExportAction == nil))
        }
        CommandGroup(after: .textEditing) {
            Button("查找…") { (logSearchRequested ?? searchRequested)?.wrappedValue = true }
                .keyboardShortcut("f").disabled(logSearchRequested == nil && searchRequested == nil)
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
                    .keyboardShortcut("k").disabled(serial.busy || (!serial.status.connected && !serial.canConnect))
                Button("断开串口") { Task { if serial.status.connected { await serial.toggleConnection() } } }
                    .keyboardShortcut("k", modifiers: [.command, .shift]).disabled(!serial.status.connected || serial.busy)
            } else {
                Button("目标配置…", systemImage: "point.3.connected.trianglepath.dotted") { activateFlash(); store.showTargetSetup = true }
                    .disabled(store.isBusy || mainIsModal)
                Divider()
                Button("扫描探针", systemImage: "arrow.clockwise") {
                    if let targetActions { targetActions.scan() } else { Task { await store.scanProbes() } }
                }
                    .keyboardShortcut("r")
                    .disabled(store.isBusy)
                Button("连接目标", systemImage: "link") {
                    if let targetActions { targetActions.connect() } else { Task { await store.connect() } }
                }
                    .keyboardShortcut("k")
                    .disabled(!(targetActions?.canConnect ?? store.canConnect) || (mainIsModal && targetActions == nil))
                Button("断开目标") { store.disconnect() }
                    .keyboardShortcut("k", modifiers: [.command, .shift])
                    .disabled(!store.canDisconnect)
                Divider()
                Button("选择芯片…") {
                    if let targetActions { targetActions.selectChip() } else { activateFlash(); store.showChipPicker = true }
                }
                    .disabled(store.isBusy || (mainIsModal && targetActions == nil))
                Button("保存配置…") { activateFlash(); store.showSaveProfile = true }
                    .keyboardShortcut("s", modifiers: [.command, .shift])
                    .disabled(store.isBusy || mainIsModal)
                Menu("删除配置") {
                    ForEach(store.profiles) { profile in
                        Button(profile.name, role: .destructive) { Task { await store.deleteProfile(profile) } }
                    }
                }
                .disabled(store.isBusy || mainIsModal || store.profiles.isEmpty)
                if !store.profiles.isEmpty {
                    Menu("载入配置") {
                        ForEach(store.profiles) { profile in
                            Button(profile.name) { store.applyProfile(profile) }
                        }
                    }
                    .disabled(store.isBusy || mainIsModal)
                }
            }
        }
        CommandMenu("操作") {
            if let logActions {
                Toggle("仅显示错误", isOn: logActions.errorsOnly)
                Toggle("跟随最新日志", isOn: logActions.followsLatest)
                Button("复制全部日志") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(store.logExportText, forType: .string)
                }
                .disabled(store.logs.isEmpty)
                Button("清空日志") { store.logs.removeAll() }.disabled(store.logs.isEmpty)
            } else if let memoryActions {
                Button("读取内存") { Task { await store.readMemory() } }
                    .keyboardShortcut("r", modifiers: [.command, .shift])
                    .disabled(!store.canReadMemory)
                Button("擦除整个芯片…", role: .destructive, action: memoryActions.requestErase)
                    .disabled(!store.canErase)
            } else if serialIsActive {
                Button("发送数据", systemImage: "paperplane") { Task { await serial.send() } }
                    .keyboardShortcut(.return, modifiers: .command).disabled(!serial.canSend)
                Button("清空终端", systemImage: "trash") { serial.clear() }
                    .keyboardShortcut("l").disabled(serial.frames.isEmpty)
                Button(serial.autoSending ? "停止自动发送" : "开始自动发送", systemImage: serial.autoSending ? "stop.fill" : "repeat") {
                    serial.toggleAutoSend()
                }
                .keyboardShortcut(.return, modifiers: [.command, .option])
                .disabled(!serial.autoSending && !serial.canStartAutoSend)
                Toggle("自动滚动", isOn: Binding(get: { serial.preferences.autoScroll }, set: { serial.preferences.autoScroll = $0 }))
            } else {
                Button(store.dryRun ? "开始预检查" : "烧录固件", systemImage: "arrow.down.to.line") { Task { await store.flash() } }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(!store.canFlash || mainIsModal)
                Button("读取内存") { panels.show(.memory); Task { await store.readMemory() } }
                    .keyboardShortcut("r", modifiers: [.command, .shift])
                    .disabled(!store.canReadMemory)
                Divider()
                Button("擦除整个芯片…", role: .destructive) { openWindow(id: "toolkit"); store.confirmErase = true }
                    .disabled(!store.canErase || mainIsModal)
            }
        }
        CommandGroup(after: .toolbar) {
            Button("固件烧录", systemImage: "cpu") { activateFlash() }.keyboardShortcut("1").disabled(mainIsModal)
            Button("串口连接", systemImage: "terminal") { toolkit.selectedTool = .serial; openWindow(id: "toolkit") }.keyboardShortcut("2").disabled(mainIsModal)
            if serialIsActive {
                if let connectionExpanded { Toggle("连接区", isOn: connectionExpanded) }
                if let composerExpanded { Toggle("发送区", isOn: composerExpanded) }
                Picker("数据方向", selection: Binding(get: { serial.direction }, set: { serial.direction = $0 })) {
                    ForEach(SerialDirectionFilter.allCases) { Text($0.rawValue).tag($0) }
                }
            }
            Divider()
            Button(store.showLogs ? "隐藏日志" : "显示日志", systemImage: "text.alignleft") { panels.toggle(.logs) }
                .keyboardShortcut("j")
            Button(store.showMemory ? "隐藏内存" : "显示内存", systemImage: "memorychip") { panels.toggle(.memory) }
                .keyboardShortcut("m", modifiers: [.command, .option])
            Divider()
        }
        CommandGroup(replacing: .help) {
            Link("ChipDock 源代码", destination: URL(string: "https://github.com/tangxiangong/chipdock")!)
            Link("probe-rs 文档", destination: URL(string: "https://probe.rs/docs/")!)
        }
    }
}
