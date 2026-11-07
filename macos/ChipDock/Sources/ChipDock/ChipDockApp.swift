import AppKit
import SwiftUI

@main
struct ChipDockApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var toolkit = ToolkitState()
    @State private var store: WorkbenchStore
    @State private var serial: SerialWorkbench
    @State private var updater: UpdateService
    @AppStorage("appearance") private var appearance = Appearance.system

    init() {
        let store = WorkbenchStore()
        let serial = SerialWorkbench()
        _serial = State(initialValue: serial)
        _store = State(initialValue: store)
        _updater = State(initialValue: UpdateService(isBusy: { [weak store] in store?.isBusy == true || serial.updateBlocked }))
    }

    var body: some Scene {
        Window("ChipDock", id: "toolkit") {
            ToolkitView(toolkit: toolkit, store: store, serial: serial)
                .modifier(SerialLifecycle(workbench: serial))
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { delegate.store = store }
                .onOpenURL { toolkit.selectedTool = .flash; store.selectFirmware($0) }
                .task {
                    guard ProcessInfo.processInfo.environment["CHIPDOCK_TEST_HOST"] != "1" else { return }
                    await store.start()
                    await updater.start()
                }
        }
        .defaultSize(width: 1160, height: 780)
        .commands { WorkbenchCommands(toolkit: toolkit, store: store, serial: serial.active, updater: updater) }

        Window("活动日志", id: WorkbenchPanel.logs.rawValue) {
            LogsView(store: store)
                .frame(minWidth: 500, minHeight: 240)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { store.showLogs = true }
                .onDisappear { store.showLogs = false }
        }
        .defaultSize(width: 720, height: 440)

        Window("目标内存", id: WorkbenchPanel.memory.rawValue) {
            MemoryView(store: store)
                .frame(minWidth: 420, minHeight: 360)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { store.showMemory = true }
                .onDisappear { store.showMemory = false }
        }
        .defaultSize(width: 560, height: 600)

        WindowGroup("实时曲线", id: "serial-plot", for: SerialPlotKey.self) { $key in
            if let key, let session = serial.sessions.first(where: { $0.id == key.session }) {
                SerialPlotView(serial: session).preferredColorScheme(appearance.colorScheme)
            } else {
                ContentUnavailableView("会话已关闭", systemImage: "waveform.path")
            }
        }
        .defaultSize(width: 820, height: 520)

        Settings {
            SettingsView(coreVersion: store.rust.version, updater: updater)
                .preferredColorScheme(appearance.colorScheme)
        }
    }
}

private struct SerialLifecycle: ViewModifier {
    var workbench: SerialWorkbench
    @Environment(\.openWindow) private var openWindow

    func body(content: Content) -> some View {
        content.task {
            guard ProcessInfo.processInfo.environment["CHIPDOCK_TEST_HOST"] != "1" else { return }
            await workbench.start { id in openWindow(id: "serial-plot", value: SerialPlotKey(session: id)) }
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var store: WorkbenchStore?

    func applicationWillTerminate(_ notification: Notification) {
        _ = try? serialShutdown()
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let store, let operation = store.operation else { return .terminateNow }
        let alert = NSAlert()
        alert.messageText = "“\(operation.rawValue)”正在进行"
        alert.informativeText = "中断可能使芯片处于不完整的状态。请等待操作完成后再退出 ChipDock。"
        alert.addButton(withTitle: "好")
        alert.runModal()
        return .terminateCancel
    }
}
