import SwiftUI

enum WorkbenchPanel: String {
    case logs = "activity-log"
    case memory = "target-memory"
}

@MainActor
struct WorkbenchPanelActions {
    var store: WorkbenchStore
    var openWindow: OpenWindowAction
    var dismissWindow: DismissWindowAction

    func show(_ panel: WorkbenchPanel) {
        if panel == .logs { store.showLogs = true } else { store.showMemory = true }
        openWindow(id: panel.rawValue)
    }

    func toggle(_ panel: WorkbenchPanel) {
        let visible = panel == .logs ? store.showLogs : store.showMemory
        if visible {
            dismissWindow(id: panel.rawValue)
            if panel == .logs { store.showLogs = false } else { store.showMemory = false }
        } else { show(panel) }
    }
}
