import SwiftUI

/// Peer tools share navigation, while their state and workers stay independent.
struct ToolkitView: View {
    @Bindable var toolkit: ToolkitState
    var store: WorkbenchStore
    var serial: SerialStore

    var body: some View {
        Group {
            switch toolkit.selectedTool {
            case .flash: FlashWorkbenchView(store: store)
            case .serial: SerialView(serial: serial)
            }
        }
        .navigationTitle("ChipDock")
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Picker("开发工具", selection: $toolkit.selectedTool) {
                    Text("固件烧录").tag(WorkbenchTool.flash)
                    Text("串口终端").tag(WorkbenchTool.serial)
                }
                .pickerStyle(.segmented)
                .frame(width: 220)
                .help("切换开发工具（⌘1 / ⌘2）；运行中的任务会继续")
            }
        }
        .frame(minWidth: 900, minHeight: 600)
    }
}
