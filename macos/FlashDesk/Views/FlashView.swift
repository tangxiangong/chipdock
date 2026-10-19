import AppKit
import SwiftUI

/// Main area: a centred column reading top to bottom — firmware, settings,
/// then the single primary action.
struct FlashView: View {
    @Bindable var store: WorkbenchStore
    @State private var isDropTargeted = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if let url = store.firmwareURL {
                    FirmwareSummary(store: store, url: url)
                } else {
                    DropZone(store: store, isTargeted: isDropTargeted)
                }
                Divider().padding(.vertical, 24)
                SettingsGrid(store: store)
                FlashAction(store: store).padding(.top, 26)
            }
            .frame(maxWidth: 580)
            .padding(.horizontal, 32)
            .padding(.top, 28)
            .padding(.bottom, 28)
            .frame(maxWidth: .infinity)
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
}

// MARK: - Firmware

private struct FirmwareSummary: View {
    var store: WorkbenchStore
    var url: URL

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
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
                let range = "\(addressLabel(usage.startAddress)) – \(addressLabel(usage.endAddress.map { $0 > 0 ? $0 - 1 : 0 }))"
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 36) {
                        fact("格式", usage.format.title)
                        fact("写入大小", byteCountLabel(usage.usedBytes))
                        fact("区段", String(usage.segments.count))
                        fact("地址范围", range, mono: true)
                        Spacer(minLength: 0)
                    }
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
                }
            }
        }
    }

    private func fact(_ title: String, _ value: String, mono: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value)
                .font(mono ? .system(.body, design: .monospaced) : .body)
                .monospacedDigit()
                .lineLimit(1)
                .textSelection(.enabled)
        }
    }
}

private struct DropZone: View {
    var store: WorkbenchStore
    var isTargeted: Bool

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
            .frame(maxWidth: .infinity)
            .padding(.vertical, 26)
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

    private var files: [RecentFirmware] { Array(store.recentFiles.prefix(5)) }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("最近使用").font(.subheadline).foregroundStyle(.secondary)
            List(files, id: \.path, selection: $selection) { file in
                let url = URL(fileURLWithPath: file.path)
                HStack(spacing: 8) {
                    Image(systemName: "doc.text").foregroundStyle(file.format.tint).frame(width: 16)
                    Text(url.lastPathComponent).lineLimit(1)
                    Text((url.deletingLastPathComponent().path as NSString).abbreviatingWithTildeInPath)
                        .foregroundStyle(.secondary).lineLimit(1).truncationMode(.head)
                    Spacer(minLength: 8)
                    Text(file.usedAt, format: .relative(presentation: .named))
                        .font(.callout).foregroundStyle(.secondary)
                }
                .help(file.path)
            }
            .listStyle(.bordered)
            .alternatingRowBackgrounds()
            .scrollDisabled(true)
            .frame(height: CGFloat(files.count) * 24 + 2)
            .disabled(store.isBusy)
            .contextMenu(forSelectionType: String.self) { paths in
                if let file = files.first(where: { paths.contains($0.path) }) {
                    Button("打开") { store.openRecent(file) }
                    Button("在访达中显示") {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: file.path)])
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

    var body: some View {
        Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 14, verticalSpacing: 16) {
            GridRow {
                label("目标")
                HStack(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Circle()
                                .fill(store.connection == nil ? Color.secondary.opacity(0.4) : .green)
                                .frame(width: 7, height: 7)
                            Text(store.targetName).fontWeight(.medium)
                            if store.connection == nil {
                                Text("未连接").foregroundStyle(.secondary)
                            }
                        }
                        Text(targetDetail).font(.callout).foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 8)
                    if store.connection != nil {
                        Button("断开连接", systemImage: "xmark.circle") { store.disconnect() }
                            .disabled(!store.canDisconnect)
                            .help("断开目标（⇧⌘K）")
                    }
                    Button("更改…", systemImage: "slider.horizontal.3") { store.showTargetSetup = true }
                        .disabled(store.isBusy)
                }
            }
            if store.isBin {
                GridRow {
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
            GridRow {
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
            GridRow {
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
            .gridColumnAlignment(.trailing)
            .frame(minWidth: 52, alignment: .trailing)
    }
}

// MARK: - Action

private struct FlashAction: View {
    var store: WorkbenchStore

    private var needsConnection: Bool { store.connection == nil }

    var body: some View {
        HStack(spacing: 16) {
            Text(hint).font(.callout).foregroundStyle(.secondary)
            Spacer(minLength: 0)
            if store.isBusy { ProgressView().controlSize(.small) }
            EraseButton(store: store)
            Button {
                Task { needsConnection ? await store.connect() : await store.flash() }
            } label: {
                if let icon {
                    Label(title, systemImage: icon)
                } else {
                    Text(title)
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(needsConnection ? !store.canConnect : !store.canFlash)
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
            .controlSize(.large)
            .disabled(!store.canErase)
            .help(store.connection == nil ? "连接后即可擦除" : "擦除整个芯片，不烧录")
    }
}
