import AppKit
import SwiftUI

struct FlashView: View {
    @Bindable var store: WorkbenchStore
    @State private var isDropTargeted = false
    @State private var displaySegments = false

    private var documents: [FirmwareDocumentRow] {
        var rows = store.recentFiles.map { FirmwareDocumentRow(path: $0.path, format: $0.format.title) }
        if let url = store.firmwareURL, !rows.contains(where: { $0.path == url.path }) {
            rows.insert(FirmwareDocumentRow(path: url.path, format: url.pathExtension.uppercased()), at: 0)
        }
        return rows
    }

    private var selection: Binding<String?> {
        Binding(get: { store.firmwareURL?.path }, set: { path in
            guard let path, path != store.firmwareURL?.path else { return }
            if let recent = store.recentFiles.first(where: { $0.path == path }) { store.openRecent(recent) }
            else { store.selectFirmware(URL(fileURLWithPath: path)) }
        })
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Label("固件", systemImage: "shippingbox").font(.headline)
                if !documents.isEmpty { Text("\(documents.count)").foregroundStyle(.tertiary) }
                Spacer()
                if store.firmwareUsage != nil {
                    Picker("预览方式", selection: $displaySegments) {
                        Image(systemName: "rectangle.3.group").help("地址布局").tag(false)
                        Image(systemName: "list.number").help("写入区段").tag(true)
                    }
                    .pickerStyle(.segmented).labelsHidden().frame(width: 76)
                }
                Button("添加固件", systemImage: "plus") { store.showImporter = true }
                    .labelStyle(.iconOnly).help("添加固件（⌘O）").disabled(store.isBusy)
            }
            .padding(.horizontal, 22).frame(height: 48)
            Divider()
            if documents.isEmpty {
                ContentUnavailableView {
                    Label("添加固件", systemImage: "shippingbox")
                } description: {
                    Text("ELF · AXF · HEX · BIN")
                } actions: {
                    Button("添加文件…", systemImage: "plus") { store.showImporter = true }
                        .buttonStyle(.borderedProminent).controlSize(.large).disabled(store.isBusy)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                VSplitView {
                    Table(documents, selection: selection) {
                        TableColumn("名称") { item in
                            Label(item.name, systemImage: "doc.text")
                                .lineLimit(1).truncationMode(.middle)
                        }
                        TableColumn("格式", value: \.format).width(60)
                        TableColumn("实际写入") { item in
                            Text(item.path == store.firmwareURL?.path ? store.firmwareUsage.map { byteCountLabel($0.usedBytes) } ?? "—" : "—")
                                .monospacedDigit().foregroundStyle(.secondary)
                        }.width(95)
                    }
                    .frame(minHeight: 110, idealHeight: 160, maxHeight: 260)
                    .contextMenu(forSelectionType: String.self) { paths in
                        if let path = paths.first {
                            Button("在访达中显示", systemImage: "arrow.up.forward.app") {
                                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                            }
                        }
                    }
                    preview.frame(minHeight: 240, maxHeight: .infinity)
                }
            }
        }
        .overlay {
            if isDropTargeted {
                Rectangle().fill(Color.accentColor.opacity(0.08))
                    .overlay { Rectangle().strokeBorder(Color.accentColor, lineWidth: 2) }
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

    @ViewBuilder
    private var preview: some View {
        if store.analyzingFirmware {
            VStack(spacing: 12) { ProgressView(); Text("正在分析固件…").foregroundStyle(.secondary) }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let error = store.firmwareError {
            ContentUnavailableView {
                Label("无法分析固件", systemImage: "exclamationmark.triangle")
            } description: { Text(error) } actions: {
                if store.isBin {
                    Button("设置基地址", systemImage: "slider.horizontal.3") { store.showInspector = true }
                }
            }
        } else if let usage = store.firmwareUsage, let url = store.firmwareURL {
            if displaySegments { segmentTable(usage) }
            else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 28) {
                        VStack(alignment: .leading, spacing: 7) {
                            Text(url.lastPathComponent).font(.title3.weight(.semibold)).lineLimit(1).truncationMode(.middle)
                            Text(url.deletingLastPathComponent().path).font(.caption).foregroundStyle(.secondary)
                                .lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                        }
                        HStack(alignment: .top, spacing: 30) {
                            metric("实际写入", value: byteCountLabel(usage.usedBytes))
                            metric("地址跨度", value: byteCountLabel(usage.spanBytes))
                            metric("区段", value: String(usage.segments.count))
                        }
                        Divider()
                        FirmwareLayoutView(usage: usage, regions: store.memoryRegions)
                        if let event = store.latestEvent, event.kind == .flash {
                            JobProgressView(event: event, isBusy: store.isBusy)
                        }
                    }
                    .padding(28)
                }
            }
        } else {
            ContentUnavailableView("选择固件以查看布局", systemImage: "rectangle.3.group")
        }
    }

    private func metric(_ title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.system(size: 18, weight: .medium, design: .monospaced)).textSelection(.enabled)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func segmentTable(_ usage: FirmwareUsage) -> some View {
        Table(Array(usage.segments.enumerated()).map { FirmwareSegmentRow(index: $0.offset, segment: $0.element) }) {
            TableColumn("区段") { Text(String(format: "%02d", $0.index + 1)).foregroundStyle(.secondary) }.width(45)
            TableColumn("起始地址") { Text(addressLabel($0.segment.start)).monospaced() }
            TableColumn("结束地址") { Text(addressLabel($0.segment.end > 0 ? $0.segment.end - 1 : 0)).monospaced() }
            TableColumn("大小") { Text(byteCountLabel($0.segment.size)).monospacedDigit() }.width(80)
        }
    }
}

private struct FirmwareDocumentRow: Identifiable {
    var path: String
    var format: String
    var id: String { path }
    var name: String { URL(fileURLWithPath: path).lastPathComponent }
}

private struct FirmwareSegmentRow: Identifiable {
    var index: Int
    var segment: FirmwareUsageSegment
    var id: Int { index }
}

struct JobProgressView: View {
    var event: ProgressEvent
    var isBusy: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Image(systemName: event.stage == .failed ? "exclamationmark.triangle.fill" : (event.stage == .completed ? "checkmark.circle.fill" : "arrow.down.to.line.circle"))
                    .foregroundStyle(event.stage == .failed ? Color.red : (event.stage == .completed ? .green : Color.accentColor))
                Text(event.message).font(.callout.weight(.medium))
                Spacer()
                if let progress = event.progress { Text("\(Int(progress * 100))%").font(.caption.monospacedDigit()).foregroundStyle(.secondary) }
            }
            if isBusy && !event.stage.isTerminal { ProgressView(value: event.progress.map(Double.init)) }
        }
    }
}
