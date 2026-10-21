import SwiftUI
import UniformTypeIdentifiers

/// Peer tools share navigation, while their state and workers stay independent.
struct ToolkitView: View {
    @Bindable var toolkit: ToolkitState
    @Bindable var store: WorkbenchStore
    var serial: SerialStore
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    var body: some View {
        Group {
            switch toolkit.selectedTool {
            case .flash: FlashWorkbenchView(store: store)
            case .serial: SerialView(serial: serial)
            }
        }
        .navigationTitle(toolkit.selectedTool == .flash ? store.firmwareURL?.lastPathComponent ?? "ChipDock" : "串口连接")
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Picker("开发工具", selection: $toolkit.selectedTool) {
                    Text("固件烧录").tag(WorkbenchTool.flash)
                    Text("串口连接").tag(WorkbenchTool.serial)
                }
                .pickerStyle(.segmented)
                .frame(width: 220)
                .help("切换开发工具（⌘1 / ⌘2）；运行中的任务会继续")
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .exportPresentation($store.pendingExport)
        .alert(store.failure?.message ?? "操作失败", isPresented: Binding(
            get: { store.failure != nil }, set: { if !$0 { store.failure = nil } }
        )) {
            Button("确定", role: .cancel) { store.failure = nil }
            Button("查看日志") {
                store.failure = nil
                WorkbenchPanelActions(store: store, openWindow: openWindow, dismissWindow: dismissWindow).show(.logs)
            }
        } message: {
            Text([store.failure?.detail, store.failure?.recovery].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: "\n\n"))
        }
        .confirmationDialog("擦除 \(store.targetName) 的全部 Flash？", isPresented: $store.confirmErase, titleVisibility: .visible) {
            Button("擦除全部 Flash", role: .destructive) { Task { await store.erase() } }
            Button("取消", role: .cancel) {}
        } message: {
            Text("芯片中的固件与数据将被永久删除，此操作无法撤销。")
        }
    }
}
