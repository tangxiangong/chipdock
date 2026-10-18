import SwiftUI
import UniformTypeIdentifiers

struct ContentView: View {
    @Bindable var store: WorkbenchStore
    var body: some View {
        document
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle(store.firmwareURL?.lastPathComponent ?? "FlashDesk")
        .toolbar {
            ToolbarItem(placement: .principal) {
                ActivityView(store: store)
            }
            ToolbarItemGroup(placement: .primaryAction) {
                Menu {
                    Button("存储当前配置…", systemImage: "square.and.arrow.down") { store.showSaveProfile = true }
                    if !store.profiles.isEmpty {
                        Divider()
                        ForEach(store.profiles, id: \.id) { profile in
                            Button(profile.name, systemImage: "bookmark") { store.applyProfile(profile) }
                        }
                        Divider()
                        Menu("删除配置", systemImage: "trash") {
                            ForEach(store.profiles, id: \.id) { profile in
                                Button(profile.name) { Task { await store.deleteProfile(profile) } }
                            }
                        }
                    }
                } label: {
                    Label("配置", systemImage: "bookmark")
                }
                .disabled(store.isBusy)
                .help("存储或载入配置")
            }
            panelToggles
        }
        .frame(minWidth: 900, minHeight: 600)
        .fileImporter(isPresented: $store.showImporter, allowedContentTypes: firmwareTypes) { result in
            switch result {
            case .success(let url): store.selectFirmware(url)
            case .failure(let error): store.report(error, operation: "打开固件")
            }
        }
        .fileExporter(isPresented: Binding(
            get: { store.pendingExport != nil }, set: { if !$0 { store.pendingExport = nil } }
        ), document: ExportDocument(data: store.pendingExport?.data ?? Data()),
           contentType: store.pendingExport?.contentType ?? .data,
           defaultFilename: store.pendingExport?.filename) { result in
            if case .failure(let error) = result { store.report(error, operation: store.pendingExport?.operation ?? "导出") }
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
            Button("查看日志") { store.failure = nil; store.showLogs = true }
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

    /// The open firmware becomes the window's document, giving the title bar
    /// its proxy icon and path menu.
    @ViewBuilder
    private var document: some View {
        if let url = store.firmwareURL {
            FlashView(store: store).navigationDocument(url)
        } else {
            FlashView(store: store)
        }
    }

    /// Logs and memory float as popovers anchored to their toolbar buttons,
    /// the same pattern BibCiTeX uses for its inspector.
    @ToolbarContentBuilder
    private var panelToggles: some ToolbarContent {
        if #available(macOS 26.0, *) {
            ToolbarSpacer(.fixed, placement: .primaryAction)
            ToolbarItem(placement: .primaryAction) {
                logsButton(icon: "text.alignleft").badge(store.unseenErrors)
            }
        } else {
            ToolbarItem(placement: .primaryAction) {
                logsButton(icon: store.unseenErrors > 0 ? "exclamationmark.bubble" : "text.alignleft")
            }
        }
        ToolbarItem(placement: .primaryAction) {
            Button { store.showMemory.toggle() } label: {
                Label("内存", systemImage: "memorychip")
            }
            .help("内存（⌥⌘M）")
            .popover(isPresented: $store.showMemory, arrowEdge: .bottom) {
                MemoryView(store: store)
                    .frame(width: 380, height: 540)
            }
        }
    }

    private func logsButton(icon: String) -> some View {
        Button { store.showLogs.toggle() } label: {
            Label("日志", systemImage: icon)
        }
        .help(store.unseenErrors > 0 ? "\(store.unseenErrors) 条新错误（⌘J）" : "日志（⌘J）")
        .popover(isPresented: $store.showLogs, arrowEdge: .bottom) {
            LogsView(store: store)
                .frame(width: 620, height: 420)
        }
    }

    private var firmwareTypes: [UTType] {
        ["elf", "axf", "hex", "bin"].compactMap { UTType(filenameExtension: $0) }
    }
}

/// Xcode-style activity display in the toolbar centre: the latest status, or
/// live progress while a hardware job runs. The target itself is shown and
/// edited in the main area.
private struct ActivityView: View {
    var store: WorkbenchStore

    private var event: ProgressEvent? {
        guard store.isBusy, let event = store.latestEvent, !event.stage.isTerminal else { return nil }
        return event
    }

    var body: some View {
        HStack(spacing: 8) {
            if let event {
                Text(event.message).lineLimit(1)
                if let progress = event.progress {
                    ProgressView(value: Double(progress)).progressViewStyle(.linear).frame(width: 90)
                    Text("\(Int(progress * 100))%").monospacedDigit()
                }
            } else {
                if store.isBusy { ProgressView().controlSize(.small) }
                Text(store.status).lineLimit(1).truncationMode(.middle)
            }
        }
        .font(.callout)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 12)
        .frame(minWidth: 280, maxWidth: 440)
        .accessibilityElement(children: .combine)
    }
}
