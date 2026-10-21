import AppKit
import SwiftUI

@main
struct ChipDockApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var toolkit = ToolkitState()
    @State private var store: WorkbenchStore
    @State private var serial: SerialStore
    @State private var updater: UpdateService
    @AppStorage("appearance") private var appearance = Appearance.system

    init() {
        let store = WorkbenchStore()
        let serial = SerialStore()
        _serial = State(initialValue: serial)
        _store = State(initialValue: store)
        _updater = State(initialValue: UpdateService(isBusy: { [weak store] in store?.isBusy == true || serial.updateBlocked }))
    }

    var body: some Scene {
        Window("ChipDock", id: "toolkit") {
            ToolkitView(toolkit: toolkit, store: store, serial: serial)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { delegate.store = store }
                .onOpenURL { toolkit.selectedTool = .flash; store.selectFirmware($0) }
                .task { await store.start(); await updater.start() }
                .task { await serial.start() }
        }
        .defaultSize(width: 1160, height: 780)
        .commands { WorkbenchCommands(toolkit: toolkit, store: store, serial: serial, updater: updater) }

        Settings {
            SettingsView(coreVersion: store.rust.version, updater: updater)
                .preferredColorScheme(appearance.colorScheme)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var store: WorkbenchStore?

    func applicationWillTerminate(_ notification: Notification) {
        _ = try? serialDisconnect()
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
