import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct SerialTerminalView: View {
    @Bindable var serial: SerialStore
    @Binding var connectionExpanded: Bool
    @Binding var composerExpanded: Bool
    @State private var searchRequested = false
    @State private var pendingExport: PendingExport?

    private var visibleFrames: [SerialFrame] {
        serial.frames.filter { frame in
            (serial.direction == .all || frame.isReceive == (serial.direction == .receive))
                && (serial.searchText.isEmpty || frame.display(serial.preferences.displayFormat).localizedStandardContains(serial.searchText))
        }
    }

    var body: some View {
        let frames = visibleFrames
        VStack(spacing: 0) {
            TerminalTable(frames: frames, format: serial.preferences.displayFormat,
                          autoScroll: serial.preferences.autoScroll, state: serial.terminalTableState)
                .overlay {
                    if frames.isEmpty { emptyState(filtered: !serial.frames.isEmpty) }
                }
            Divider()
            HStack(spacing: 12) {
                Picker("接收编码", selection: $serial.preferences.displayFormat) {
                    ForEach(SerialDataFormat.allCases) { Text($0.title).tag($0) }
                }
                .frame(width: 160)
                Text("\(frames.count)/\(serial.frames.count) 条").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                Spacer(minLength: 12)
                Toggle(isOn: $serial.preferences.autoScroll) {
                    Label("自动滚动", systemImage: "arrow.down.to.line")
                }
                .toggleStyle(.button)
                .help(serial.preferences.autoScroll ? "关闭自动滚动" : "滚动到最新数据")
                Button(action: exportHistory) {
                    Label("导出", systemImage: "square.and.arrow.up")
                }
                .disabled(serial.frames.isEmpty)
                .help("将当前保留的全部收发记录导出为 JSON（不受筛选影响）")
                Button(action: serial.clear) {
                    Label("清屏", systemImage: "trash")
                }
                .disabled(serial.frames.isEmpty)
                .help("清空终端（⌘L）")
            }
            .controlSize(.small)
            .padding(.horizontal, 18)
            .frame(height: 44)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .toolbar {
            ToolbarItemGroup(placement: .navigation) {
                Toggle(isOn: $connectionExpanded) { Label("连接区", systemImage: "sidebar.left") }
                    .toggleStyle(.button).help(connectionExpanded ? "收起连接区" : "展开连接区")
                Toggle(isOn: $composerExpanded) { Label("发送区", systemImage: "rectangle.bottomthird.inset.filled") }
                    .toggleStyle(.button).help(composerExpanded ? "收起发送区" : "展开发送区")
            }
            ToolbarItemGroup(placement: .primaryAction) {
                Picker("数据方向", selection: $serial.direction) {
                    ForEach(SerialDirectionFilter.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented).labelsHidden().fixedSize()
                SearchField(prompt: "搜索数据", text: $serial.searchText, focusRequested: $searchRequested)
                    .frame(minWidth: 120, idealWidth: 180, maxWidth: 240)
                    .help("搜索数据（⌘F）")
            }
        }
        .focusedSceneValue(\.serialSearchRequested, $searchRequested)
        .exportPresentation($pendingExport)
    }

    private func exportHistory() {
        guard !serial.frames.isEmpty else { return }
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            pendingExport = PendingExport(data: try encoder.encode(serial.frames), contentType: .json,
                                          filename: "ChipDock-Serial.json", operation: "导出串口记录")
        } catch {
            serial.error = "导出串口记录失败：\(error.localizedDescription)"
        }
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
    let state: TerminalTableState

    func makeNSView(context: Context) -> NSScrollView {
        let table = TerminalNSTableView()
        context.coordinator.state = state
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
            column.width = max(minimum, state.columnWidths[name] ?? width)
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
        context.coordinator.scroll = scroll
        scroll.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(context.coordinator, selector: #selector(Coordinator.didScroll(_:)),
                                               name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let table = scroll.documentView as? NSTableView else { return }
        let coordinator = context.coordinator
        coordinator.state = state
        let ids = frames.map(\.id)
        let changed = coordinator.frames.map(\.id) != ids || coordinator.format != format
        let initial = !coordinator.didLoad
        let follow = autoScroll && (initial ? state.topFrameID == nil : changed || !coordinator.autoScroll)
        if !initial { coordinator.captureViewport() }
        coordinator.updating = true
        defer { coordinator.updating = false; coordinator.didLoad = true }
        if changed {
            coordinator.frames = frames
            coordinator.format = format
            table.reloadData()
            table.selectRowIndexes(state.selection(in: ids), byExtendingSelection: false)
        }
        if !follow, (changed || initial), let index = state.anchor(in: ids) {
            table.layoutSubtreeIfNeeded()
            let offset = ids.contains(where: { $0 == state.topFrameID }) ? state.topOffset : 0
            scroll.contentView.scroll(to: NSPoint(x: state.horizontalOffset, y: max(0, table.rect(ofRow: index).minY + offset)))
            scroll.reflectScrolledClipView(scroll.contentView)
        }
        coordinator.autoScroll = autoScroll
        if follow, !frames.isEmpty { table.scrollRowToVisible(frames.count - 1) }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: Coordinator) {
        coordinator.captureViewport()
    }

    @MainActor
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        var frames: [SerialFrame] = []
        var format = SerialDataFormat.utf8
        var autoScroll = false
        var state: TerminalTableState?
        weak var scroll: NSScrollView?
        var updating = false
        var didLoad = false

        deinit { NotificationCenter.default.removeObserver(self) }

        func captureViewport() {
            guard didLoad, let state, let scroll, let table = scroll.documentView as? NSTableView else { return }
            state.selectedIDs = Set(table.selectedRowIndexes.compactMap { frames.indices.contains($0) ? frames[$0].id : nil })
            let row = table.row(at: NSPoint(x: 0, y: scroll.contentView.bounds.minY))
            if frames.indices.contains(row) {
                state.topFrameID = frames[row].id
                state.topOffset = scroll.contentView.bounds.minY - table.rect(ofRow: row).minY
            }
            state.horizontalOffset = scroll.contentView.bounds.minX
            for column in table.tableColumns { state.columnWidths[column.identifier.rawValue] = column.width }
        }

        @objc func didScroll(_ notification: Notification) {
            if !updating { captureViewport() }
        }

        func tableViewSelectionDidChange(_ notification: Notification) {
            if !updating { captureViewport() }
        }

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
            if !updating { captureViewport() }
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
