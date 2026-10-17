import SwiftUI

struct WorkbenchCommands: Commands {
    var store: WorkbenchStore
    var updater: UpdateService

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("打开固件…", systemImage: "doc.badge.plus") { store.showImporter = true }
                .keyboardShortcut("o")
                .disabled(store.isBusy)
            Menu("最近使用") {
                ForEach(store.recentFiles, id: \.path) { file in
                    Button(URL(fileURLWithPath: file.path).lastPathComponent) { store.openRecent(file) }
                }
            }
            .disabled(store.isBusy || store.recentFiles.isEmpty)
        }
        CommandGroup(after: .appInfo) {
            Button("检查更新…", action: updater.checkForUpdates)
                .disabled(!updater.canCheckForUpdates)
        }
        CommandMenu("目标") {
            Button("目标配置…", systemImage: "point.3.connected.trianglepath.dotted") { store.showTargetSetup = true }
            Divider()
            Button("扫描探针", systemImage: "arrow.clockwise") { Task { await store.scanProbes() } }
                .keyboardShortcut("r")
                .disabled(store.isBusy)
            Button("连接目标", systemImage: "cable.connector") { Task { await store.connect() } }
                .keyboardShortcut("k")
                .disabled(!store.canConnect)
            Button("断开目标") { store.disconnect() }
                .keyboardShortcut("k", modifiers: [.command, .shift])
                .disabled(store.isBusy || store.connection == nil)
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
        CommandMenu("操作") {
            Button(store.dryRun ? "开始预检查" : "烧录固件", systemImage: "arrow.down.to.line") { Task { await store.flash() } }
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(!store.canFlash)
            Button("读取内存") { store.showMemory = true; Task { await store.readMemory() } }
                .keyboardShortcut("r", modifiers: [.command, .shift])
                .disabled(!store.canReadMemory)
            Button("擦除整个芯片…", role: .destructive) { store.confirmErase = true }
                .disabled(store.isBusy || store.connection == nil)
        }
        CommandGroup(after: .toolbar) {
            Button(store.showMemory ? "隐藏内存面板" : "显示内存面板", systemImage: "square.grid.3x3") {
                store.showMemory.toggle()
            }
            .keyboardShortcut("m", modifiers: [.command, .option])
            Button(store.showConsole ? "隐藏活动面板" : "显示活动面板", systemImage: "terminal") {
                store.showConsole.toggle()
            }
            .keyboardShortcut("y", modifiers: [.command, .option])
            Button(store.showInspector ? "隐藏烧录设置" : "显示烧录设置", systemImage: "slider.horizontal.3") {
                store.showInspector.toggle()
            }
            .keyboardShortcut("i", modifiers: [.command, .option])
        }
        CommandGroup(replacing: .help) {
            Link("FlashDesk 源代码", destination: URL(string: "https://github.com/tangxiangong/flashdesk")!)
            Link("probe-rs 文档", destination: URL(string: "https://probe.rs/docs/")!)
        }
    }
}
