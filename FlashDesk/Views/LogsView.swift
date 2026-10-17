import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct LogsView: View {
    @Bindable var store: WorkbenchStore
    @State private var query = ""
    @State private var errorsOnly = false
    @State private var selection: UUID?
    @State private var showExporter = false

    private var entries: [ActivityEntry] {
        store.logs.filter { entry in
            (!errorsOnly || entry.level == .error)
                && (query.isEmpty || "\(entry.operation) \(entry.message) \(entry.detail)".localizedCaseInsensitiveContains(query))
        }
    }

    private var exportData: Data {
        Data(store.logs.map { entry in
            "\(entry.date.ISO8601Format())\t\(entry.level.rawValue)\t\(entry.operation)\t\(entry.message)\(entry.detail.isEmpty ? "" : "\n\(entry.detail)")"
        }.joined(separator: "\n").utf8)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                TextField("搜索日志", text: $query).textFieldStyle(.roundedBorder)
                    .accessibilityLabel("搜索日志")
                Toggle("仅错误", isOn: $errorsOnly).toggleStyle(.checkbox)
                Button("导出…", systemImage: "square.and.arrow.up") { showExporter = true }.disabled(store.logs.isEmpty)
                Button("清空", systemImage: "trash") { store.logs.removeAll(); selection = nil }.disabled(store.logs.isEmpty)
            }
            .padding(20)
            Divider()
            if entries.isEmpty {
                ContentUnavailableView(query.isEmpty ? "暂无日志" : "无匹配日志", systemImage: "list.bullet.rectangle")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(entries, selection: $selection) {
                    TableColumn("时间") { entry in
                        Text(entry.date, format: .dateTime.hour().minute().second()).monospacedDigit().foregroundStyle(.secondary)
                    }
                    .width(78)
                    TableColumn("操作", value: \.operation).width(70)
                    TableColumn("内容") { entry in
                        Label(entry.message, systemImage: entry.level == .error ? "exclamationmark.circle" : (entry.level == .success ? "checkmark.circle" : "info.circle"))
                            .foregroundStyle(entry.level == .error ? Color.red : .primary)
                            .lineLimit(2)
                            .help(entry.detail.isEmpty ? entry.message : entry.detail)
                    }
                }
                .contextMenu(forSelectionType: UUID.self) { ids in
                    Button("复制") {
                        let text = store.logs.filter { ids.contains($0.id) }.map { "\($0.message)\n\($0.detail)" }.joined(separator: "\n")
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(text, forType: .string)
                    }
                    .disabled(ids.isEmpty)
                }
            }
            if let selected = store.logs.first(where: { $0.id == selection }), !selected.detail.isEmpty {
                Divider()
                ScrollView {
                    Text(selected.detail).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading).padding(16)
                }
                .frame(maxHeight: 140)
            }
        }
        .fileExporter(isPresented: $showExporter, document: ExportDocument(data: exportData),
                      contentType: .plainText, defaultFilename: "FlashDesk.log") { result in
            if case .failure(let error) = result { store.report(error, operation: "导出日志") }
        }
    }
}
