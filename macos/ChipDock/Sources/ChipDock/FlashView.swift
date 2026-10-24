import AppKit
import SwiftUI

/// The drop zone fills the empty state. With a firmware open, its data viewer
/// fills spare height while metadata, settings and actions stay compact.
struct FlashView: View {
    @Bindable var store: WorkbenchStore
    @State private var isDropTargeted = false

    var body: some View {
        GeometryReader { geometry in
            let compact = geometry.size.height < 560
            let margin: CGFloat = compact ? 20 : 24
            VSplitView {
                Group {
                    if let url = store.firmwareURL {
                        FirmwareSummary(store: store, url: url, spacing: compact ? 16 : 20)
                    } else {
                        DropZone(store: store, isTargeted: isDropTargeted, compact: compact)
                    }
                }
                .frame(width: Self.columnWidth(in: geometry.size.width))
                .padding(.vertical, margin)
                .frame(maxWidth: .infinity, minHeight: store.firmwareURL == nil ? 260 : 280, maxHeight: .infinity)

                ScrollView {
                    VStack(spacing: compact ? 20 : 24) {
                        SettingsGrid(store: store, spacing: compact ? 12 : 16)
                        FlashAction(store: store)
                    }
                    .frame(width: Self.columnWidth(in: geometry.size.width))
                    .padding(.vertical, margin)
                    .frame(maxWidth: .infinity)
                }
                .scrollBounceBehavior(.basedOnSize)
                .frame(minHeight: 180, idealHeight: 240, maxHeight: 300)
            }
        }
        .overlay {
            if isDropTargeted && store.firmwareURL != nil {
                Rectangle()
                    .strokeBorder(Color.accentColor, lineWidth: 2)
                    .background(Color.accentColor.opacity(0.05))
                    .allowsHitTesting(false)
            }
        }
        .dropDestination(for: URL.self) { urls, _ in
            guard let url = urls.first, !store.isBusy else { return false }
            store.selectFirmware(url)
            return true
        } isTargeted: { isDropTargeted = $0 }
        .onChange(of: store.binBaseAddress) { store.scheduleFirmwareAnalysis() }
    }

    /// The window width less side margins of a twentieth of it (at least 32 pt).
    nonisolated static func columnWidth(in width: CGFloat) -> CGFloat {
        max(width - 2 * max(32, width * 0.05), 0)
    }
}

// MARK: - Firmware

private struct FirmwareSummary: View {
    var store: WorkbenchStore
    var url: URL
    var spacing: CGFloat

    var body: some View {
        WorkbenchColumnLayout(spacing: spacing) {
            HStack(spacing: 14) {
                let tint = store.firmwareUsage?.format.tint ?? .secondary
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(tint.opacity(0.14))
                    .frame(width: 44, height: 44)
                    .overlay {
                        Image(systemName: "doc.text").font(.system(size: 20, weight: .medium)).foregroundStyle(tint)
                    }
                VStack(alignment: .leading, spacing: 3) {
                    Text(url.lastPathComponent)
                        .font(.title2.weight(.semibold))
                        .lineLimit(1).truncationMode(.middle)
                    Text((url.deletingLastPathComponent().path as NSString).abbreviatingWithTildeInPath)
                        .font(.callout).foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.head)
                        .help(url.path)
                }
                .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
                Spacer(minLength: 12)
                Menu {
                    Button("更换固件…", systemImage: "arrow.triangle.2.circlepath") { store.showImporter = true }
                    Button("在访达中显示", systemImage: "folder") { NSWorkspace.shared.activateFileViewerSelecting([url]) }
                    Divider()
                    Button("关闭固件", systemImage: "xmark.circle") { store.clearFirmware() }
                } label: {
                    Image(systemName: "ellipsis.circle").font(.system(size: 15))
                }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                .disabled(store.isBusy)
                .help("固件操作")
            }

            if store.analyzingFirmware {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("正在分析固件…").foregroundStyle(.secondary)
                }
            } else if let error = store.firmwareError {
                Label {
                    Text(error).textSelection(.enabled)
                } icon: {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                }
                .font(.callout)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.orange.opacity(0.08), in: RoundedRectangle(cornerRadius: 6))
            } else if let usage = store.firmwareUsage {
                FirmwareLayoutView(usage: usage, regions: store.memoryRegions)
                if let preview = store.firmwarePreview {
                    FirmwareDataView(preview: preview)
                        .id(url)
                        .layoutValue(key: WorkbenchExpansion.self, value: 1)
                }
                let range = "\(addressLabel(usage.startAddress)) – \(addressLabel(usage.endAddress.map { $0 > 0 ? $0 - 1 : 0 }))"
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 36) {
                        fact("格式", usage.format.title)
                        fact("写入大小", byteCountLabel(usage.usedBytes))
                        fact("区段", String(usage.segments.count))
                        fact("地址范围", range, mono: true)
                        Spacer(minLength: 0)
                    }
                    .fixedSize(horizontal: true, vertical: false)
                    Grid(alignment: .leading, horizontalSpacing: 36, verticalSpacing: 14) {
                        GridRow {
                            fact("格式", usage.format.title)
                            fact("写入大小", byteCountLabel(usage.usedBytes))
                        }
                        GridRow {
                            fact("区段", String(usage.segments.count))
                            fact("地址范围", range, mono: true)
                        }
                    }
                    .fixedSize(horizontal: true, vertical: false)
                    VStack(alignment: .leading, spacing: 14) {
                        fact("格式", usage.format.title)
                        fact("写入大小", byteCountLabel(usage.usedBytes))
                        fact("区段", String(usage.segments.count))
                        fact("地址范围", range, mono: true)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func fact(_ title: String, _ value: String, mono: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value)
                .font(mono ? .system(.body, design: .monospaced) : .body)
                .monospacedDigit()
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
    }
}

private struct DropZone: View {
    var store: WorkbenchStore
    var isTargeted: Bool
    var compact: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            VStack(spacing: 8) {
                Image(systemName: "arrow.down.doc")
                    .font(.system(size: 30, weight: .light))
                    .foregroundStyle(isTargeted ? Color.accentColor : Color.secondary)
                    .padding(.bottom, 4)
                Text("将固件拖到这里").font(.title3.weight(.medium))
                Text("支持 ELF、AXF、HEX 与 BIN 文件").font(.callout).foregroundStyle(.secondary)
                Button("选择文件…", systemImage: "folder") { store.showImporter = true }
                    .disabled(store.isBusy)
                    .padding(.top, 8)
            }
            .padding(.vertical, compact ? 12 : 26)
            .frame(maxWidth: .infinity, minHeight: compact ? 134 : 170, maxHeight: .infinity)
            .background(isTargeted ? Color.accentColor.opacity(0.06) : Color.clear,
                        in: RoundedRectangle(cornerRadius: 10))
            .overlay {
                RoundedRectangle(cornerRadius: 10)
                    .strokeBorder(isTargeted ? Color.accentColor : Color.secondary.opacity(0.35),
                                  style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))
            }

            if !store.recentFiles.isEmpty {
                RecentFilesList(store: store)
            }
        }
    }
}

/// Recent firmware as a system list: select to highlight, double-click or
/// Return to open, right-click for more.
private struct RecentFilesList: View {
    var store: WorkbenchStore
    @State private var selection: String?

    private var files: [RecentFirmware] { store.recentFiles }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("最近使用").font(.subheadline).foregroundStyle(.secondary)
                Spacer()
                if !store.missingRecentPaths.isEmpty {
                    Button("清除失效记录") { Task { await store.clearMissingRecentFiles() } }
                }
                Button("清空列表") { Task { await store.clearRecentFiles() } }
            }
            .buttonStyle(.borderless)
            .disabled(store.isBusy)
            List(files, id: \.path, selection: $selection) { file in
                let url = URL(fileURLWithPath: file.path)
                let isMissing = store.missingRecentPaths.contains(file.path)
                HStack(spacing: 8) {
                    Image(systemName: "doc.text").foregroundStyle(file.format.tint).frame(width: 16)
                    Text(url.lastPathComponent).lineLimit(1)
                        .foregroundStyle(isMissing ? .secondary : .primary)
                    Text((url.deletingLastPathComponent().path as NSString).abbreviatingWithTildeInPath)
                        .foregroundStyle(.secondary).lineLimit(1).truncationMode(.head)
                    Spacer(minLength: 8)
                    if isMissing {
                        Label("文件不存在", systemImage: "exclamationmark.triangle.fill")
                            .font(.callout).foregroundStyle(.orange).fixedSize()
                    } else {
                        Text(file.usedAt, format: .relative(presentation: .named))
                            .font(.callout).foregroundStyle(.secondary).fixedSize()
                    }
                    Button {
                        Task { await store.removeRecentFiles(at: [file.path]) }
                    } label: {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(.borderless)
                    .help("从最近使用中移除")
                    .accessibilityLabel("移除 \(url.lastPathComponent) 的最近使用记录")
                }
                .help(file.path)
            }
            .listStyle(.bordered)
            .alternatingRowBackgrounds()
            .environment(\.defaultMinListRowHeight, 30)
            .frame(minHeight: 30, idealHeight: max(32, CGFloat(min(files.count, 5)) * 30 + 2),
                   maxHeight: max(32, CGFloat(min(files.count, 5)) * 30 + 2))
            .disabled(store.isBusy)
            .onAppear { store.refreshRecentFileAvailability() }
            .onDeleteCommand {
                if let selection { Task { await store.removeRecentFiles(at: [selection]) } }
            }
            .contextMenu(forSelectionType: String.self) { paths in
                if let file = files.first(where: { paths.contains($0.path) }) {
                    Button("打开") { store.openRecent(file) }
                        .disabled(store.missingRecentPaths.contains(file.path))
                    Button("在访达中显示") {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: file.path)])
                    }
                    .disabled(store.missingRecentPaths.contains(file.path))
                    Divider()
                    Button("从最近使用中移除", role: .destructive) {
                        Task { await store.removeRecentFiles(at: paths) }
                    }
                }
            } primaryAction: { paths in
                if let file = files.first(where: { paths.contains($0.path) }) { store.openRecent(file) }
            }
        }
    }
}

// MARK: - Settings

/// Classic Mac preferences layout: trailing-aligned labels, leading controls.
private struct SettingsGrid: View {
    @Bindable var store: WorkbenchStore
    var spacing: CGFloat

    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            HStack(alignment: .firstTextBaseline, spacing: 14) {
                label("目标")
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .firstTextBaseline, spacing: 16) {
                        targetSummary
                        Spacer(minLength: 0)
                        targetButtons
                    }
                    VStack(alignment: .leading, spacing: 10) {
                        targetSummary
                        targetButtons
                    }
                }
            }
            if store.isBin {
                HStack(alignment: .firstTextBaseline, spacing: 14) {
                    label("基地址")
                    HStack(spacing: 8) {
                        TextField("基地址", text: $store.binBaseAddress)
                            .textFieldStyle(.roundedBorder)
                            .labelsHidden()
                            .font(.body.monospaced())
                            .frame(width: 130)
                            .disabled(store.isBusy)
                        if parseAddress(store.binBaseAddress) == nil {
                            Label("无效的十六进制地址", systemImage: "exclamationmark.triangle.fill")
                                .font(.callout).foregroundStyle(.red)
                        }
                    }
                }
            }
            HStack(alignment: .firstTextBaseline, spacing: 14) {
                label("擦除")
                VStack(alignment: .leading, spacing: 4) {
                    Picker("擦除", selection: $store.eraseStrategy) {
                        ForEach(EraseStrategy.allCases) { Text($0.title).tag($0) }
                    }
                    .labelsHidden().fixedSize()
                    Text(store.eraseStrategy.detail).font(.callout).foregroundStyle(.secondary)
                }
                .disabled(store.isBusy)
            }
            HStack(alignment: .firstTextBaseline, spacing: 14) {
                label("完成后")
                VStack(alignment: .leading, spacing: 7) {
                    Toggle("校验写入的数据", isOn: $store.verify)
                    Toggle("复位并运行程序", isOn: $store.resetAfter)
                    Toggle("仅预检查，不写入芯片", isOn: $store.dryRun)
                }
                .disabled(store.isBusy)
            }
        }
    }

    private var targetSummary: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Circle()
                    .fill(store.connection == nil ? Color.secondary.opacity(0.4) : .green)
                    .frame(width: 7, height: 7)
                Text(store.targetName).fontWeight(.medium)
                    .lineLimit(1).truncationMode(.middle).help(store.targetName)
                if store.connection == nil {
                    Text("未连接").foregroundStyle(.secondary).fixedSize()
                }
            }
            Text(targetDetail).font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var targetButtons: some View {
        HStack(spacing: 8) {
            if store.connection != nil {
                Button("断开连接", systemImage: "xmark.circle") { store.disconnect() }
                    .disabled(!store.canDisconnect)
                    .help("断开目标（⇧⌘K）")
            }
            Button("更改…", systemImage: "slider.horizontal.3") { store.showTargetSetup = true }
                .disabled(store.isBusy)
        }
        .fixedSize()
    }

    private var targetDetail: String {
        if let connection = store.connection {
            let speed = connection.speedKhz.map { "\($0) kHz" } ?? "\(store.speed) kHz"
            return "\(connection.protocol.title) · \(speed) · \(connection.probe)"
        }
        let probe = store.probes.first { $0.identifier == store.selectedProbe }?.displayName ?? "自动选择探针"
        return "\(store.wireProtocol.title) · \(store.speed) kHz · \(probe)"
    }

    private func label(_ text: String) -> some View {
        Text(text)
            .foregroundStyle(.secondary)
            .frame(width: 52, alignment: .trailing)
    }
}

// MARK: - Action

private struct FlashAction: View {
    var store: WorkbenchStore

    private var needsConnection: Bool { store.connection == nil }

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 16) {
                hintLabel.fixedSize()
                Spacer(minLength: 0)
                buttons.fixedSize()
            }
            VStack(alignment: .leading, spacing: 12) {
                hintLabel
                buttons.frame(maxWidth: .infinity, alignment: .trailing)
            }
        }
    }

    private var hintLabel: some View {
        Text(hint).font(.callout).foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private var buttons: some View {
        HStack(spacing: 12) {
            if store.isBusy { ProgressView().controlSize(.small) }
            EraseButton(store: store).fixedSize()
            Button {
                Task { needsConnection ? await store.connect() : await store.flash() }
            } label: {
                if let icon {
                    Label(title, systemImage: icon)
                } else {
                    Text(title)
                }
            }
            .lineLimit(1)
            .truncationMode(.middle)
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(needsConnection ? !store.canConnect : !store.canFlash)
            .help(title)
        }
    }

    private var title: String {
        if store.operation == .connect { return "正在连接…" }
        if store.operation == .erase { return "正在擦除…" }
        if store.operation == .flash { return store.dryRun ? "正在预检查…" : "正在烧录…" }
        if needsConnection { return "连接目标" }
        return store.dryRun ? "开始预检查" : "烧录到 \(store.targetName)"
    }

    private var icon: String? {
        if store.isBusy { return nil }
        if needsConnection { return "link" }
        return store.dryRun ? "checkmark.shield" : "bolt.fill"
    }

    private var hint: String {
        if store.isBusy { return "" }
        if needsConnection { return "连接后即可烧录（⌘K）" }
        if store.firmwareURL == nil { return "选择固件后即可烧录" }
        return "按 ⌘↩ 开始"
    }
}

/// Erases the whole chip without programming, after confirmation.
private struct EraseButton: View {
    var store: WorkbenchStore

    var body: some View {
        Button("擦除芯片…", systemImage: "eraser", role: .destructive) { store.confirmErase = true }
            .buttonStyle(.bordered)
            .controlSize(.regular)
            .disabled(!store.canErase)
            .help(store.connection == nil ? "连接后即可擦除" : "擦除整个芯片，不烧录")
    }
}
