import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct LogsView: View {
    @Bindable var store: WorkbenchStore
    @State private var pendingExport: PendingExport?
    @State private var query = ""
    @State private var errorsOnly = false
    @State private var followsLatest = true
    @State private var searchRequested = false

    private var entries: [ActivityEntry] {
        store.logs.filter { entry in
            (!errorsOnly || entry.level == .error)
                && (query.isEmpty || "\(entry.operation) \(entry.message) \(entry.detail)".localizedCaseInsensitiveContains(query))
        }
    }

    var body: some View {
        LogTextView(entries: entries, followsLatest: $followsLatest)
            .overlay {
                if entries.isEmpty {
                    Text(store.logs.isEmpty ? "暂无日志" : "没有匹配的日志").foregroundStyle(.secondary)
                }
            }
            .navigationSubtitle("\(entries.count) 条日志 · \(store.logs.filter { $0.level == .error }.count) 条错误")
            .toolbar {
                ToolbarItem {
                    SearchField(prompt: "筛选日志", text: $query, focusRequested: $searchRequested)
                        .frame(minWidth: 140, idealWidth: 180, maxWidth: 240)
                }
                ToolbarItemGroup {
                    Toggle(isOn: $errorsOnly) { Label("仅显示错误", systemImage: "exclamationmark.circle") }
                        .toggleStyle(.button).help("仅显示错误")
                    Toggle(isOn: $followsLatest) { Label("跟随最新日志", systemImage: "arrow.down.to.line") }
                        .toggleStyle(.button).help("上滚时暂停跟随；点击继续显示最新日志")
                }
                ToolbarItemGroup {
                    Button("复制全部日志", systemImage: "doc.on.doc") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(store.logExportText, forType: .string)
                    }
                    .disabled(store.logs.isEmpty)
                    Button("导出日志…", systemImage: "square.and.arrow.up", action: export)
                        .disabled(store.logs.isEmpty)
                    Button("清空日志", systemImage: "trash") { store.logs.removeAll() }
                        .disabled(store.logs.isEmpty)
                }
            }
            .focusedSceneValue(\.logSearchRequested, $searchRequested)
            .focusedSceneValue(\.logViewActions, LogViewActions(errorsOnly: $errorsOnly, followsLatest: $followsLatest))
            .focusedSceneValue(\.logExport, export)
            .exportPresentation($pendingExport)
    }

    private func export() {
        pendingExport = PendingExport(data: Data(store.logExportText.utf8), contentType: .plainText,
                                      filename: "ChipDock.log", operation: "导出日志")
    }
}

/// AppKit supplies native text selection and scroll notifications, including on macOS 14.
private struct LogTextView: NSViewRepresentable {
    var entries: [ActivityEntry]
    @Binding var followsLatest: Bool

    func makeNSView(context: Context) -> NSScrollView {
        let text = NSTextView()
        text.isEditable = false
        text.isSelectable = true
        text.drawsBackground = false
        text.usesFindBar = true
        text.textContainerInset = NSSize(width: 16, height: 10)
        text.isVerticallyResizable = true
        text.isHorizontallyResizable = false
        text.autoresizingMask = [.width]
        text.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        text.textContainer?.widthTracksTextView = true
        text.setAccessibilityLabel("活动日志，包含日志级别、时间、操作和详细信息")
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.documentView = text
        scroll.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(context.coordinator, selector: #selector(Coordinator.didScroll(_:)),
                                               name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let text = scroll.documentView as? NSTextView else { return }
        let coordinator = context.coordinator
        coordinator.follow = $followsLatest
        let ids = entries.map(\.id)
        let changed = ids != coordinator.ids
        let resume = followsLatest && !coordinator.following
        guard changed || resume else { return }
        coordinator.updating = true
        defer { coordinator.updating = false; coordinator.following = followsLatest }
        let origin = scroll.contentView.bounds.origin
        let selection = text.selectedRanges
        let previousRanges = coordinator.ranges
        let anchor = coordinator.anchor(in: text, at: origin.y)
        if changed {
            let content = NSMutableAttributedString(string: "")
            coordinator.ranges = []
            for entry in entries {
                let start = content.length
                let time = entry.date.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute().second())
                let color: NSColor = entry.level == .error ? .systemRed : entry.level == .success ? .systemGreen : .labelColor
                content.append(NSAttributedString(string: "\(time)  [\(entry.level.rawValue)] [\(entry.operation)]  \(entry.message)\n",
                                                  attributes: [.foregroundColor: color]))
                if !entry.detail.isEmpty {
                    content.append(NSAttributedString(string: "  \(entry.detail)\n", attributes: [.foregroundColor: NSColor.secondaryLabelColor]))
                }
                coordinator.ranges.append((entry.id, NSRange(location: start, length: content.length - start)))
            }
            content.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
                                 range: NSRange(location: 0, length: content.length))
            text.textStorage?.setAttributedString(content)
            if let container = text.textContainer { text.layoutManager?.ensureLayout(for: container) }
            let retainedSelection = LogTextSelection.restore(selection.map(\.rangeValue), from: previousRanges,
                                                              to: coordinator.ranges).map { NSValue(range: $0) }
            text.selectedRanges = retainedSelection.isEmpty ? [NSValue(range: NSRange(location: 0, length: 0))] : retainedSelection
            coordinator.ids = ids
        }
        if followsLatest {
            text.scrollToEndOfDocument(nil)
        } else {
            let y = anchor.flatMap { id, offset in coordinator.y(for: id, in: text).map { $0 + offset } } ?? origin.y
            let maximum = max(0, text.bounds.height - scroll.contentView.bounds.height)
            scroll.contentView.scroll(to: NSPoint(x: origin.x, y: min(max(0, y), maximum)))
            scroll.reflectScrolledClipView(scroll.contentView)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    @MainActor
    final class Coordinator: NSObject {
        var ids: [UUID] = []
        var ranges: [(UUID, NSRange)] = []
        var follow: Binding<Bool> = .constant(true)
        var following = true
        var updating = false

        deinit { NotificationCenter.default.removeObserver(self) }

        func y(for id: UUID, in text: NSTextView) -> CGFloat? {
            guard let range = ranges.first(where: { $0.0 == id })?.1 else { return nil }
            return y(for: range, in: text)
        }

        private func y(for range: NSRange, in text: NSTextView) -> CGFloat? {
            guard let layout = text.layoutManager, let container = text.textContainer else { return nil }
            let glyph = layout.glyphRange(forCharacterRange: NSRange(location: range.location, length: min(1, range.length)), actualCharacterRange: nil)
            return layout.boundingRect(forGlyphRange: glyph, in: container).minY + text.textContainerInset.height
        }

        func anchor(in text: NSTextView, at top: CGFloat) -> (UUID, CGFloat)? {
            guard let entry = ranges.last(where: { (y(for: $0.1, in: text) ?? .greatestFiniteMagnitude) <= top }) ?? ranges.first,
                  let y = y(for: entry.1, in: text) else { return nil }
            return (entry.0, top - y)
        }

        @objc func didScroll(_ notification: Notification) {
            guard !updating, let clip = notification.object as? NSClipView,
                  let document = clip.documentView else { return }
            let atEnd = document.bounds.maxY - clip.bounds.maxY <= 2
            guard atEnd != follow.wrappedValue else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self, !updating else { return }
                follow.wrappedValue = atEnd
            }
        }
    }
}

private struct LogSearchRequestKey: FocusedValueKey { typealias Value = Binding<Bool> }
struct LogViewActions {
    var errorsOnly: Binding<Bool>
    var followsLatest: Binding<Bool>
}
private struct LogViewActionsKey: FocusedValueKey { typealias Value = LogViewActions }

extension FocusedValues {
    var logViewActions: LogViewActions? {
        get { self[LogViewActionsKey.self] }
        set { self[LogViewActionsKey.self] = newValue }
    }
    var logSearchRequested: Binding<Bool>? {
        get { self[LogSearchRequestKey.self] }
        set { self[LogSearchRequestKey.self] = newValue }
    }
}
