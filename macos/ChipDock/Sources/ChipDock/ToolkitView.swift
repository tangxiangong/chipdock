import SwiftUI
import UniformTypeIdentifiers

/// Peer tools share navigation, while their state and workers stay independent.
struct ToolkitView: View {
    @Bindable var toolkit: ToolkitState
    @Bindable var store: WorkbenchStore
    var serial: SerialWorkbench

    var body: some View {
        Group {
            switch toolkit.selectedTool {
            case .flash: FlashWorkbenchView(store: store)
            case .serial: SerialWorkbenchView(workbench: serial)
            }
        }
        .navigationTitle(toolkit.selectedTool == .flash ? "固件烧录" : "串口连接")
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
        .modifier(WorkbenchFailureAlert(store: store))
        .confirmationDialog("擦除 \(store.targetName) 的全部 Flash？", isPresented: $store.confirmErase, titleVisibility: .visible) {
            Button("擦除全部 Flash", role: .destructive) { Task { await store.erase() } }
            Button("取消", role: .cancel) {}
        } message: {
            Text("芯片中的固件与数据将被永久删除，此操作无法撤销。")
        }
    }
}

struct WorkbenchFailureAlert: ViewModifier {
    @Bindable var store: WorkbenchStore
    var inTargetSetup = false
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    var isPresented: Binding<Bool> {
        Binding(
            get: { store.failure != nil && store.showTargetSetup == inTargetSetup },
            set: { if !$0 && store.showTargetSetup == inTargetSetup { store.failure = nil } }
        )
    }

    func body(content: Content) -> some View {
        content.alert(store.failure?.message ?? "操作失败", isPresented: isPresented, presenting: store.failure) { _ in
            Button("确定", role: .cancel) { store.failure = nil }
            Button("查看日志") {
                store.failure = nil
                WorkbenchPanelActions(store: store, openWindow: openWindow, dismissWindow: dismissWindow).show(.logs)
            }
        } message: { failure in
            Text([failure.detail, failure.recovery].filter { !$0.isEmpty }.joined(separator: "\n\n"))
        }
    }
}
