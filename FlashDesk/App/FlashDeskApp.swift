import AppKit
import SwiftUI

@main
struct FlashDeskApp: App {
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
        Window("FlashDesk", id: "workbench") {
            ContentView(store: store)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { delegate.store = store }
                .onOpenURL { store.selectFirmware($0) }
                .task { await store.start(); updater.start() }
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
        guard store?.isBusy == true else { return .terminateNow }
        store?.status = "请等待当前操作完成后退出"
        NSSound.beep()
        return .terminateCancel
    }
}
