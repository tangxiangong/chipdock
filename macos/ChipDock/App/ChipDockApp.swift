import AppKit
import SwiftUI

@main
struct ChipDockApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var store: WorkbenchStore
    @State private var updater: UpdateService
    @AppStorage("appearance") private var appearance = Appearance.system

    init() {
        let store = WorkbenchStore()
        _store = State(initialValue: store)
        _updater = State(initialValue: UpdateService(isBusy: { [weak store] in store?.isBusy == true }))
    }

    var body: some Scene {
        Window("ChipDock", id: "workbench") {
            ContentView(store: store)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { delegate.store = store }
                .onOpenURL { store.selectFirmware($0) }
                .task { await store.start(); await updater.start() }
        }
        .defaultSize(width: 1160, height: 780)
        .commands { WorkbenchCommands(store: store, updater: updater) }

        Settings {
            SettingsView(coreVersion: store.rust.version, updater: updater)
                .preferredColorScheme(appearance.colorScheme)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var store: WorkbenchStore?

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
