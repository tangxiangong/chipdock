import AppKit
import SwiftUI

struct SerialTerminalView: View {
    @Bindable var serial: SerialStore
    @State private var searchRequested = false

    private var visibleFrames: [SerialFrame] {
        serial.frames.filter { frame in
            (serial.direction == .all || frame.isReceive == (serial.direction == .receive))
                && (serial.searchText.isEmpty || frame.display(serial.preferences.displayFormat).localizedStandardContains(serial.searchText))
        }
    }

    var body: some View {
        let frames = visibleFrames
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Label("接收区", systemImage: "text.alignleft")
                    .font(.system(size: 13, weight: .semibold))
                    .fixedSize()
                Text(serial.searchText.isEmpty && serial.direction == .all
                     ? "\(serial.frames.count) 条" : "\(frames.count)/\(serial.frames.count) 条")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Picker("数据方向", selection: $serial.direction) {
                    ForEach(SerialDirectionFilter.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 152)
                TerminalSearchField(text: $serial.searchText, focusRequested: $searchRequested)
                    .frame(minWidth: 120, idealWidth: 180, maxWidth: 220)
                    .help("搜索数据（⌘F）")
            }
            .controlSize(.small)
            .padding(.horizontal, 18)
            .frame(height: 56)
            Divider()
            TerminalTable(frames: frames, format: serial.preferences.displayFormat, autoScroll: serial.preferences.autoScroll)
                .overlay {
                    if frames.isEmpty { emptyState(filtered: !serial.frames.isEmpty) }
                }
            Divider()
            HStack(spacing: 12) {
                Picker("接收编码", selection: $serial.preferences.displayFormat) {
                    ForEach(SerialDataFormat.allCases) { Text($0.title).tag($0) }
                }
                .frame(width: 160)
                Spacer(minLength: 12)
                Toggle(isOn: $serial.preferences.autoScroll) {
                    Label("自动滚动", systemImage: "arrow.down.to.line")
                }
                .toggleStyle(.button)
                .help(serial.preferences.autoScroll ? "关闭自动滚动" : "滚动到最新数据")
                Button(action: serial.clear) {
                    Label("清屏", systemImage: "trash")
                }
                .disabled(serial.frames.isEmpty)
                .help("清空终端（⌘L）")
            }
            .controlSize(.small)
            .padding(.horizontal, 18)
            .frame(height: 44)
            .background(.bar)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.background)
        .focusedSceneValue(\.serialSearchRequested, $searchRequested)
    }

    private func emptyState(filtered: Bool) -> some View {
        VStack(spacing: 12) {
            Image(systemName: filtered ? "magnifyingglass" : "terminal")
                .font(.system(size: 32, weight: .light))
                .foregroundStyle(.tertiary)
                .accessibilityHidden(true)
            Text(filtered ? "没有匹配的数据" : serial.status.connected ? "等待设备数据" : "暂无收发记录")
                .font(.system(size: 14, weight: .medium))
            Text(filtered ? "尝试其他搜索内容，或显示全部方向。" : serial.status.connected ? "设备数据会实时显示在这里。" : "在左侧选择串口并连接，即可开始收发。")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            if filtered {
                Button("清除筛选") { serial.searchText = ""; serial.direction = .all }
                    .controlSize(.small)
            }
        }
        .padding(24)
    }
}

// AppKit supplies column resizing, multiline rows, selection, copying and precise scrolling.
private struct TerminalTable: NSViewRepresentable {
    let frames: [SerialFrame]
    let format: SerialDataFormat
    let autoScroll: Bool

    func makeNSView(context: Context) -> NSScrollView {
        let table = TerminalNSTableView()
        table.delegate = context.coordinator
        table.dataSource = context.coordinator
        table.style = .fullWidth
        table.usesAlternatingRowBackgroundColors = false
        table.usesAutomaticRowHeights = false
        table.rowHeight = 30
        table.intercellSpacing = NSSize(width: 12, height: 2)
        table.allowsMultipleSelection = true
        table.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle
        table.autoresizingMask = [.width]
        table.setAccessibilityLabel("串口数据终端")
        for (name, title, width, minimum) in [
            ("time", "时间", 104.0, 88.0),
            ("direction", "方向", 64.0, 60.0),
            ("size", "字节", 52.0, 40.0),
            ("data", "数据", 260.0, 120.0)
        ] {
            let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier(name))
            column.title = title
            column.width = width
            column.minWidth = minimum
            table.addTableColumn(column)
        }
        table.copySelection = { [weak coordinator = context.coordinator, weak table] in
            guard let coordinator, let table else { return }
            let text = table.selectedRowIndexes.compactMap { index -> String? in
                guard coordinator.frames.indices.contains(index) else { return nil }
                return coordinator.frames[index].display(coordinator.format)
            }.joined(separator: "\n")
            guard !text.isEmpty else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.autohidesScrollers = true
        scroll.documentView = table
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let table = scroll.documentView as? NSTableView else { return }
        let coordinator = context.coordinator
        let changed = coordinator.frames.map(\.id) != frames.map(\.id) || coordinator.format != format
        let follow = autoScroll && (changed || !coordinator.autoScroll)
        if changed {
            let selection = Set(table.selectedRowIndexes.compactMap { index in
                coordinator.frames.indices.contains(index) ? coordinator.frames[index].id : nil
            })
            let topRow = table.row(at: NSPoint(x: 0, y: scroll.contentView.bounds.minY))
            let anchor = coordinator.frames.indices.contains(topRow) ? coordinator.frames[topRow].id : nil
            let offset = topRow >= 0 ? scroll.contentView.bounds.minY - table.rect(ofRow: topRow).minY : 0
            coordinator.frames = frames
            coordinator.format = format
            table.reloadData()
            table.selectRowIndexes(IndexSet(frames.indices.filter { selection.contains(frames[$0].id) }), byExtendingSelection: false)
            if !autoScroll, let anchor, let index = frames.firstIndex(where: { $0.id == anchor }) {
                table.layoutSubtreeIfNeeded()
                scroll.contentView.scroll(to: NSPoint(x: scroll.contentView.bounds.minX, y: max(0, table.rect(ofRow: index).minY + offset)))
                scroll.reflectScrolledClipView(scroll.contentView)
            }
        }
        coordinator.autoScroll = autoScroll
        if follow, !frames.isEmpty { table.scrollRowToVisible(frames.count - 1) }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        var frames: [SerialFrame] = []
        var format = SerialDataFormat.utf8
        var autoScroll = false

        func numberOfRows(in tableView: NSTableView) -> Int { frames.count }

        func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
            guard frames.indices.contains(row), let column = tableView.tableColumns.last else { return 30 }
            let text = frames[row].display(format).trimmingCharacters(in: .newlines)
            let bounds = (text as NSString).boundingRect(
                with: NSSize(width: max(1, column.width - 8), height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading],
                attributes: [.font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)]
            )
            return max(30, ceil(bounds.height) + 6)
        }

        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
            guard frames.indices.contains(row), let column = tableColumn else { return nil }
            let cell: NSTableCellView
            if let reused = tableView.makeView(withIdentifier: column.identifier, owner: nil) as? NSTableCellView {
                cell = reused
            } else {
                cell = NSTableCellView()
                cell.identifier = column.identifier
                let text = NSTextField(wrappingLabelWithString: "")
                text.font = .monospacedSystemFont(ofSize: column.identifier.rawValue == "data" ? 13 : 11, weight: .regular)
                text.isSelectable = true
                text.translatesAutoresizingMaskIntoConstraints = false
                if column.identifier.rawValue != "data" { text.maximumNumberOfLines = 1 }
                if column.identifier.rawValue == "size" { text.alignment = .right }
                cell.addSubview(text)
                cell.textField = text
                NSLayoutConstraint.activate([
                    text.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: column.identifier.rawValue == "direction" ? 20 : 4),
                    text.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -4),
                    text.topAnchor.constraint(equalTo: cell.topAnchor, constant: 3),
                    text.bottomAnchor.constraint(equalTo: cell.bottomAnchor, constant: -3)
                ])
                if column.identifier.rawValue == "direction" {
                    let icon = NSImageView()
                    icon.translatesAutoresizingMaskIntoConstraints = false
                    icon.contentTintColor = .controlAccentColor
                    cell.addSubview(icon)
                    cell.imageView = icon
                    NSLayoutConstraint.activate([
                        icon.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 2),
                        icon.topAnchor.constraint(equalTo: cell.topAnchor, constant: 4),
                        icon.widthAnchor.constraint(equalToConstant: 12),
                        icon.heightAnchor.constraint(equalToConstant: 12)
                    ])
                }
            }
            let frame = frames[row]
            cell.textField?.textColor = column.identifier.rawValue == "data" || column.identifier.rawValue == "direction"
                ? .labelColor : .secondaryLabelColor
            switch column.identifier.rawValue {
            case "time": cell.textField?.stringValue = frame.time
            case "direction":
                cell.textField?.stringValue = frame.isReceive ? "RX" : "TX"
                cell.imageView?.image = NSImage(systemSymbolName: frame.isReceive ? "arrow.down.left" : "arrow.up.right",
                                               accessibilityDescription: frame.isReceive ? "接收" : "发送")
            case "size": cell.textField?.stringValue = String(frame.bytes.count)
            default: cell.textField?.stringValue = frame.display(format).trimmingCharacters(in: .newlines)
            }
            return cell
        }

        func tableViewColumnDidResize(_ notification: Notification) {
            guard let table = notification.object as? NSTableView else { return }
            table.noteHeightOfRows(withIndexesChanged: IndexSet(integersIn: 0..<frames.count))
        }
    }
}

private final class TerminalNSTableView: NSTableView {
    var copySelection: (() -> Void)?

    @objc func copy(_ sender: Any?) { copySelection?() }

    override func menu(for event: NSEvent) -> NSMenu? {
        let row = row(at: convert(event.locationInWindow, from: nil))
        if row >= 0, !selectedRowIndexes.contains(row) {
            selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
        }
        let menu = NSMenu()
        menu.autoenablesItems = false
        let copy = menu.addItem(withTitle: "拷贝数据", action: #selector(copy(_:)), keyEquivalent: "")
        copy.target = self
        copy.isEnabled = !selectedRowIndexes.isEmpty
        return menu
    }
}

private struct TerminalSearchField: NSViewRepresentable {
    @Binding var text: String
    @Binding var focusRequested: Bool

    func makeNSView(context: Context) -> NSSearchField {
        let field = NSSearchField()
        field.placeholderString = "搜索数据"
        field.sendsSearchStringImmediately = true
        field.sendsWholeSearchString = false
        field.delegate = context.coordinator
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.setAccessibilityLabel("搜索数据")
        return field
    }

    func updateNSView(_ field: NSSearchField, context: Context) {
        context.coordinator.parent = self
        if field.stringValue != text { field.stringValue = text }
        if focusRequested, let window = field.window {
            window.makeFirstResponder(field)
            field.currentEditor()?.selectAll(nil)
            let request = $focusRequested
            DispatchQueue.main.async { request.wrappedValue = false }
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSSearchField, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? 180, height: nsView.intrinsicContentSize.height)
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, NSSearchFieldDelegate {
        var parent: TerminalSearchField
        init(_ parent: TerminalSearchField) { self.parent = parent }
        func controlTextDidChange(_ notification: Notification) {
            if let field = notification.object as? NSSearchField { parent.text = field.stringValue }
        }
    }
}
