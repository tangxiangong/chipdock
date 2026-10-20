import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct LogsView: View {
    @Bindable var store: WorkbenchStore
    @State private var query = ""
    @State private var errorsOnly = false

    private var entries: [ActivityEntry] {
        store.logs.filter { entry in
            (!errorsOnly || entry.level == .error)
                && (query.isEmpty || "\(entry.operation) \(entry.message) \(entry.detail)".localizedCaseInsensitiveContains(query))
        }
    }

    private var errorCount: Int { store.logs.filter { $0.level == .error }.count }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text("日志").font(.title3)
                if errorCount > 0 {
                    Text("\(errorCount)")
                        .font(.system(size: 10, weight: .semibold)).monospacedDigit()
                        .foregroundStyle(.white)
                        .padding(.horizontal, 5).padding(.vertical, 1)
                        .background(Color.red, in: Capsule())
                        .help("\(errorCount) 条错误")
                }
                Spacer()
                SearchField(prompt: "筛选", text: $query)
                    .frame(width: 170)
                    .accessibilityLabel("筛选日志")
                Toggle(isOn: $errorsOnly) { Image(systemName: "exclamationmark.circle") }
                    .toggleStyle(.button).buttonStyle(.borderless)
                    .help("仅显示错误")
                Group {
                    Button("复制全部", systemImage: "doc.on.doc") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(store.logExportText, forType: .string)
                    }
                    .help("复制全部日志")
                    Button("导出…", systemImage: "square.and.arrow.up") {
                        store.requestExport(PendingExport(data: Data(store.logExportText.utf8), contentType: .plainText,
                                                          filename: "ChipDock.log", operation: "导出日志"))
                    }
                        .help("导出日志")
                    Button("清空", systemImage: "trash") { store.logs.removeAll() }
                        .help("清空日志")
                }
                .labelStyle(.iconOnly).buttonStyle(.borderless)
                .disabled(store.logs.isEmpty)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            Divider()
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 1) {
                        ForEach(entries) { entry in LogLine(entry: entry).id(entry.id) }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .overlay {
                    if entries.isEmpty {
                        Text(store.logs.isEmpty ? "暂无日志" : "没有匹配的日志").foregroundStyle(.secondary)
                    }
                }
                .onAppear { if let last = entries.last { proxy.scrollTo(last.id, anchor: .bottom) } }
                .onChange(of: store.logs.count) {
                    if let last = entries.last { proxy.scrollTo(last.id, anchor: .bottom) }
                }
            }
        }
    }
}

private struct LogLine: View {
    var entry: ActivityEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            (Text(entry.date, format: .dateTime.hour(.twoDigits(amPM: .omitted)).minute().second()).foregroundStyle(.tertiary)
                + Text("  [\(entry.operation)]  ").foregroundStyle(.secondary)
                + Text(entry.message).foregroundStyle(entry.level == .info ? Color.primary : entry.level.tint))
            if !entry.detail.isEmpty {
                Text(entry.detail).foregroundStyle(.secondary).padding(.leading, 76)
            }
        }
        .font(Theme.mono)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
