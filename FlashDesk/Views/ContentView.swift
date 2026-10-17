import SwiftUI
import UniformTypeIdentifiers

struct ContentView: View {
    @Bindable var store: WorkbenchStore

    var body: some View {
        // Explicit native splits avoid the NavigationSplitView/inspector
        // constraint loop reproduced on macOS 26+ and in BibCiTeX.
        HSplitView {
            VStack(spacing: 0) {
                VSplitView {
                    FlashView(store: store)
                        .frame(minHeight: 220, maxHeight: .infinity)
                    if store.showMemory {
                        WorkbenchPanel(title: "内存", symbol: "square.grid.3x3", isPresented: $store.showMemory) {
                            MemoryView(store: store)
                        }
                        .frame(minHeight: 240, idealHeight: 300, maxHeight: 500)
                    }
                    if store.showConsole {
                        WorkbenchPanel(title: "活动", symbol: "terminal", isPresented: $store.showConsole) {
                            LogsView(store: store)
                        }
                        .frame(minHeight: 140, idealHeight: 190, maxHeight: 360)
                    }
                }
                StatusBar(store: store)
            }
            .frame(minWidth: 650, maxWidth: .infinity, maxHeight: .infinity)
            if store.showInspector {
                ProgrammingInspector(store: store)
                    .frame(minWidth: 270, idealWidth: 285, maxWidth: 340, maxHeight: .infinity)
                    .background(.bar)
            }
        }
        .navigationTitle("FlashDesk")
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Button("打开固件", systemImage: "doc.badge.plus") { store.showImporter = true }
                    .disabled(store.isBusy).help("打开固件（⌘O）")
            }
            ToolbarItem(placement: .principal) {
                Button {
                    store.showTargetSetup = true
                } label: {
                    Label(store.connection == nil ? "选择目标" : store.targetName,
                          systemImage: "point.3.connected.trianglepath.dotted")
                }
                .help("选择探针和目标芯片")
            }
            ToolbarItemGroup(placement: .primaryAction) {
                Button(store.dryRun ? "预检查" : "烧录", systemImage: store.dryRun ? "checkmark.shield" : "arrow.down.to.line") {
                    Task { await store.flash() }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!store.canFlash)
                .help("\(store.dryRun ? "开始预检查" : "烧录固件")（⌘↩）")
                Menu {
                    Button("保存配置…") { store.showSaveProfile = true }
                    if !store.profiles.isEmpty {
                        Divider()
                        ForEach(store.profiles) { profile in
                            Menu(profile.name) {
                                Button("载入") { store.applyProfile(profile) }
                                Button("删除", role: .destructive) { Task { await store.deleteProfile(profile) } }
                            }
                        }
                    }
                } label: { Label("烧录配置", systemImage: "bookmark") }
                .disabled(store.isBusy)
                Button("内存面板", systemImage: "square.grid.3x3") { store.showMemory.toggle() }
                    .help("显示或隐藏内存面板（⌥⌘M）")
                Button("活动面板", systemImage: "terminal") { store.showConsole.toggle() }
                    .help("显示或隐藏活动面板（⌥⌘Y）")
                Button("烧录设置", systemImage: "slider.horizontal.3") { store.showInspector.toggle() }
                    .help("显示或隐藏烧录设置（⌥⌘I）")
            }
        }
        .frame(minWidth: 960, minHeight: 640)
        .fileImporter(isPresented: $store.showImporter, allowedContentTypes: firmwareTypes) { result in
            switch result {
            case .success(let url): store.selectFirmware(url)
            case .failure(let error): store.report(error, operation: "打开固件")
            }
        }
        .sheet(isPresented: $store.showTargetSetup) { TargetSetupSheet(store: store) }
        .sheet(isPresented: Binding(
            get: { store.showChipPicker && !store.showTargetSetup },
            set: { store.showChipPicker = $0 }
        )) { ChipPicker(store: store) }
        .sheet(isPresented: $store.showSaveProfile) { SaveProfileSheet(store: store) }
        .alert(store.failure?.message ?? "操作失败", isPresented: Binding(
            get: { store.failure != nil }, set: { if !$0 { store.failure = nil } }
        )) {
            Button("确定", role: .cancel) { store.failure = nil }
            Button("查看活动") { store.failure = nil; store.showConsole = true }
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

    private var firmwareTypes: [UTType] {
        ["elf", "axf", "hex", "bin"].compactMap { UTType(filenameExtension: $0) }
    }
}

private struct WorkbenchPanel<Content: View>: View {
    var title: String
    var symbol: String
    @Binding var isPresented: Bool
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Label(title, systemImage: symbol).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Spacer()
                Button("收起\(title)面板", systemImage: "xmark") { isPresented = false }
                    .labelStyle(.iconOnly).buttonStyle(.borderless)
            }
            .padding(.horizontal, 16).frame(height: 30)
            Divider()
            content()
        }
    }
}

private struct StatusBar: View {
    var store: WorkbenchStore

    var body: some View {
        VStack(spacing: 0) {
            Divider()
            HStack(spacing: 8) {
                if store.isBusy { ProgressView().controlSize(.mini) }
                else { Image(systemName: store.connection == nil ? "circle" : "checkmark.circle.fill")
                    .foregroundStyle(store.connection == nil ? Color.secondary : .green) }
                Text(store.status).lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 8)
                if let connection = store.connection {
                    Text("\(connection.protocol.title) · \(store.speed) kHz").monospacedDigit()
                }
            }
            .font(.caption).foregroundStyle(.secondary)
            .padding(.horizontal, 16).frame(height: 30)
        }
    }
}
