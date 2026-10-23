import Accessibility
import SwiftUI
import UniformTypeIdentifiers

struct FlashWorkbenchView: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    private var panels: WorkbenchPanelActions {
        WorkbenchPanelActions(store: store, openWindow: openWindow, dismissWindow: dismissWindow)
    }

    var body: some View {
        document
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle(store.firmwareURL?.lastPathComponent ?? "ChipDock")
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
                                Button(profile.name, role: .destructive) { Task { await store.deleteProfile(profile) } }
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
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { store.refreshRecentFileAvailability() }
        }
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

    /// Auxiliary windows remain available while the main workbench is in use.
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
            Button { panels.toggle(.memory) } label: {
                Label("内存", systemImage: "memorychip")
            }
            .help("内存（⌥⌘M）")
        }
    }

    private func logsButton(icon: String) -> some View {
        Button { panels.toggle(.logs) } label: {
            Label("日志", systemImage: icon)
        }
        .help(store.unseenErrors > 0 ? "\(store.unseenErrors) 条新错误（⌘J）" : "日志（⌘J）")
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

    private var completion: ProgressEvent? {
        guard !store.isBusy, let event = store.latestEvent,
              event.stage == .completed, store.status == event.message else { return nil }
        return event
    }

    var body: some View {
        HStack(spacing: 8) {
            if let event {
                Text(event.progress == nil ? event.message : messageWithoutPercentage(event.message)).lineLimit(1)
                ProgressView(value: event.progress.map(Double.init))
                    .progressViewStyle(.linear)
                    .frame(width: 90)
                    .accessibilityLabel(event.message)
                if let progress = event.progress {
                    Text("\(Int((progress * 100).rounded()))%")
                        .monospacedDigit()
                        .frame(minWidth: 34, alignment: .trailing)
                        .fixedSize()
                }
            } else {
                if store.isBusy { ProgressView().controlSize(.small) }
                if completion != nil {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .accessibilityHidden(true)
                }
                Text(store.status)
                    .fontWeight(completion == nil ? .regular : .semibold)
                    .foregroundStyle(completion == nil ? .secondary : .primary)
                    .lineLimit(1).truncationMode(.middle)
            }
        }
        .font(.callout)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 12)
        .frame(minWidth: 280, maxWidth: 440)
        .accessibilityElement(children: .combine)
        .onChange(of: completion?.id) { _, _ in
            if let completion { AccessibilityNotification.Announcement(completion.message).post() }
        }
    }

    private func messageWithoutPercentage(_ message: String) -> String {
        message.replacingOccurrences(of: #"\s*[（(]\d+%[）)]\s*$"#, with: "", options: .regularExpression)
    }
}
