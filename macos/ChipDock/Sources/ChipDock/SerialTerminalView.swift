import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct SerialTerminalView: View {
    @Bindable var serial: SerialStore
    @Binding var connectionExpanded: Bool
    @Binding var composerExpanded: Bool
    @State private var searchRequested = false
    @State private var pendingExport: PendingExport?
    @State private var analysis = false
    @State private var colors = false
    @State private var searchPosition: UInt64?

    private var appearance: TerminalAppearance {
        TerminalAppearance(format: serial.preferences.displayFormat, mixed: serial.preferences.mixed,
            timestamps: serial.preferences.timestamps, query: serial.searchText, regex: serial.searchRegex,
            caseSensitive: serial.searchCase, colors: serial.preferences.colors, txColor: serial.preferences.txColor)
    }

    var body: some View {
        let appearance = appearance
        let query = appearance.matcher
        let frames = (serial.pausedFrames ?? serial.frames).filter { frame in
            (serial.direction == .all || frame.isReceive == (serial.direction == .receive))
                && (appearance.query.isEmpty || query.map { appearance.range(in: appearance.display(frame), regex: $0) != nil } == true)
        }
        VStack(spacing: 0) {
            if serial.protocolDisplay != "raw" && !serial.protocolRules.isEmpty {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 6) {
                        ForEach(serial.pausedParsed ?? serial.protocolRecords) { record in
                            VStack(alignment: .leading, spacing: 2) {
                                Text("\(record.name.isEmpty ? "RX" : record.name)  \(record.message)")
                                    .foregroundStyle(record.valid ? Color.primary : Color.orange)
                                Text(record.hex).foregroundStyle(.secondary)
                            }.font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading).padding(12)
                }.frame(maxHeight: serial.protocolDisplay == "both" ? 150 : .infinity)
                Divider()
            }
            if serial.protocolDisplay != "parsed" || serial.protocolRules.isEmpty {
            TerminalTable(frames: frames, appearance: appearance, jump: serial.terminalJump,
                          autoScroll: serial.preferences.autoScroll && serial.pausedFrames == nil, state: serial.terminalTableState)
                .overlay {
                    if frames.isEmpty { emptyState(filtered: !serial.frames.isEmpty) }
                }
            }
            Divider()
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 12) {
                    displayOptions(count: frames.count).fixedSize()
                    Spacer(minLength: 12)
                    terminalActions.fixedSize()
                }
                VStack(alignment: .leading, spacing: 6) {
                    displayOptions(count: frames.count)
                    terminalActions.frame(maxWidth: .infinity, alignment: .trailing)
                }
            }
            .controlSize(.small)
            .padding(.horizontal, 12)
            .padding(.vertical, 5)
            .background(.bar)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .toolbar {
            ToolbarItemGroup(placement: .navigation) {
                Toggle(isOn: $connectionExpanded) { Label("连接区", systemImage: "sidebar.left") }
                    .toggleStyle(.button).help(connectionExpanded ? "收起连接区" : "展开连接区")
            }
            ToolbarItemGroup(placement: .primaryAction) {
                Picker("数据方向", selection: $serial.direction) {
                    ForEach(SerialDirectionFilter.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented).labelsHidden().fixedSize()
                Menu("搜索选项", systemImage: "line.3.horizontal.decrease") {
                    Toggle("区分大小写", isOn: $serial.searchCase)
                    Toggle("正则表达式", isOn: $serial.searchRegex)
                }
                SearchField(prompt: "搜索数据", text: $serial.searchText, focusRequested: $searchRequested)
                    .frame(minWidth: 120, idealWidth: 180, maxWidth: 240)
                    .help("搜索数据（⌘F）")
                Button { navigate(frames, step: -1) } label: { Label("上一条命中", systemImage: "chevron.up") }
                    .disabled(serial.searchText.isEmpty || frames.isEmpty)
                Button { navigate(frames, step: 1) } label: { Label("下一条命中", systemImage: "chevron.down") }
                    .disabled(serial.searchText.isEmpty || frames.isEmpty)
            }
        }
        .focusedSceneValue(\.serialSearchRequested, $searchRequested)
        .exportPresentation($pendingExport)
        .sheet(isPresented: $analysis) { SerialRecordAnalysis(serial: serial) }
        .sheet(isPresented: $colors) { SerialColorEditor(serial: serial) }
    }

    private func displayOptions(count: Int) -> some View {
        HStack(spacing: 10) {
            if !serial.protocolRules.isEmpty {
                Picker("解析显示", selection: $serial.protocolDisplay) {
                    Text("原始").tag("raw"); Text("解析").tag("parsed"); Text("同时").tag("both")
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
                .help("解析显示")
            }
            Picker("接收编码", selection: $serial.preferences.displayFormat) {
                ForEach(SerialDataFormat.allCases) { Text($0.title).tag($0) }
            }
            .labelsHidden()
            .fixedSize()
            .help("接收编码")
            Text(count == serial.frames.count ? "\(count) 条" : "\(count) / \(serial.frames.count) 条")
                .font(.caption).monospacedDigit().foregroundStyle(.secondary)
        }
    }

    private var terminalActions: some View {
        HStack(spacing: 4) {
            Toggle(isOn: $serial.preferences.autoScroll) {
                Label("自动滚动", systemImage: "arrow.down.to.line")
            }
            .toggleStyle(.button)
            .help(serial.preferences.autoScroll ? "关闭自动滚动" : "滚动到最新数据")
            Toggle(isOn: Binding(get: { serial.pausedFrames != nil }, set: { _ in serial.togglePause() })) {
                Label(serial.pausedFrames == nil ? "暂停显示" : "继续显示", systemImage: "pause")
            }
            .toggleStyle(.button)
            .help(serial.pausedFrames == nil ? "暂停显示" : "继续显示")
            Divider().frame(height: 14).padding(.horizontal, 4)
            Menu {
                Toggle("文本与 HEX 混显", isOn: $serial.preferences.mixed)
                Toggle("显示时间戳", isOn: $serial.preferences.timestamps)
                Button("着色规则…") { colors = true }
                Divider()
                Button("重置计数") { Task { await serial.resetCounters() } }
                Toggle("清屏时重置计数", isOn: $serial.preferences.resetOnClear)
            } label: {
                Label("显示选项", systemImage: "slider.horizontal.3")
            }
            .menuIndicator(.hidden)
            .help("显示选项")
            Button("记录表", systemImage: "tablecells") { analysis = true }
                .help("记录表")
            Menu {
                Button("JSON") { exportHistory() }
                ForEach(["txt", "csv", "bin"], id: \.self) { format in
                    Button(format.uppercased()) { exportHistory(format: format) }
                }
            } label: {
                Label("导出", systemImage: "square.and.arrow.up")
            }
            .menuIndicator(.hidden)
            .disabled(serial.frames.isEmpty)
            .help("导出记录")
            Button(action: serial.clear) {
                Label("清屏", systemImage: "trash")
            }
            .disabled(serial.frames.isEmpty)
            .help("清空终端（⌘L）")
            Divider().frame(height: 14).padding(.horizontal, 4)
            Toggle(isOn: $composerExpanded) {
                Label("发送区", systemImage: "rectangle.bottomthird.inset.filled")
            }
            .toggleStyle(.button)
            .help(composerExpanded ? "收起发送区" : "展开发送区")
            .accessibilityValue(composerExpanded ? "已展开" : "已收起")
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.borderless)
        .menuStyle(.borderlessButton)
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

    private func navigate(_ matches: [SerialFrame], step: Int) {
        guard !matches.isEmpty else { return }
        let index = matches.firstIndex { $0.sequence == searchPosition }
        let start = index ?? (step > 0 ? -1 : 0)
        let target = matches[(start + step + matches.count) % matches.count].sequence
        searchPosition = target
        serial.preferences.autoScroll = false
        serial.terminalJump = target
    }

    private func exportHistory(format: String) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "ChipDock-Serial.\(format)"
        panel.begin { result in
            guard result == .OK, let path = panel.url?.path else { return }
            Task { @MainActor in
                do {
                    _ = try await serial.workbench(["action": "export", "path": path,
                                                     "format": format, "after": serial.clearedThrough])
                } catch { serial.error = error.localizedDescription }
            }
        }
    }

    private func emptyState(filtered: Bool) -> some View {
        ContentUnavailableView {
            Label(filtered ? "没有匹配的数据" : serial.status.connected ? "等待设备数据" : "暂无收发记录",
                  systemImage: filtered ? "magnifyingglass" : serial.status.connected ? "antenna.radiowaves.left.and.right" : "cable.connector")
        } description: {
            Text(filtered ? "尝试其他搜索内容，或显示全部方向。" : serial.status.connected ? "设备数据会实时显示在这里。" : "在左侧选择串口并连接，即可开始收发。")
        } actions: {
            if filtered {
                Button("清除筛选") { serial.searchText = ""; serial.direction = .all }
            }
        }
    }
}

private struct TerminalAppearance: Equatable {
    var format = SerialDataFormat.utf8
    var mixed = false
    var timestamps = true
    var query = ""
    var regex = false
    var caseSensitive = false
    var colors: [SerialColorRule] = []
    var txColor = ""

    var matcher: NSRegularExpression? {
        guard !query.isEmpty && query.utf16.count <= 256 else { return nil }
        return try? NSRegularExpression(pattern: regex ? query : NSRegularExpression.escapedPattern(for: query),
                                        options: caseSensitive ? [] : [.caseInsensitive])
    }

    func range(in text: String, regex: NSRegularExpression) -> NSRange? {
        let deadline = ContinuousClock.now.advanced(by: .milliseconds(12))
        var found: NSRange?
        regex.enumerateMatches(in: text, options: [.reportProgress], range: NSRange(location: 0, length: text.utf16.count)) { match, _, stop in
            if let match { found = match.range }
            if found != nil || ContinuousClock.now >= deadline { stop.pointee = true }
        }
        return found
    }

    func display(_ frame: SerialFrame) -> String {
        guard mixed else { return frame.display(format) }
        let text = frame.display(format == .hex ? .utf8 : format)
        return text + "\n" + frame.hex
    }

    func color(_ frame: SerialFrame, text: String) -> NSColor {
        let label = frame.isReceive ? colors.first { rule in
            if rule.regex {
                guard let regex = try? NSRegularExpression(pattern: rule.pattern) else { return false }
                return range(in: text, regex: regex) != nil
            }
            return text.localizedCaseInsensitiveContains(rule.pattern)
        }?.color : txColor
        switch label {
        case "red": return .systemRed
        case "orange": return .systemOrange
        case "green": return .systemGreen
        case "blue": return .systemBlue
        case "purple": return .systemPurple
        default: return .labelColor
        }
    }
}

// AppKit supplies column resizing, multiline rows, selection, copying and precise scrolling.
private struct TerminalTable: NSViewRepresentable {
    let frames: [SerialFrame]
    let appearance: TerminalAppearance
    let jump: UInt64?
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
                return coordinator.appearance.display(coordinator.frames[index])
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
        let changed = coordinator.frames.map(\.id) != ids || coordinator.appearance != appearance
        let initial = !coordinator.didLoad
        let follow = autoScroll && (initial ? state.topFrameID == nil : changed || !coordinator.autoScroll)
        if !initial { coordinator.captureViewport() }
        coordinator.updating = true
        defer { coordinator.updating = false; coordinator.didLoad = true }
        table.tableColumns.first?.isHidden = !appearance.timestamps
        if changed {
            coordinator.frames = frames
            coordinator.appearance = appearance
            coordinator.matcher = appearance.matcher
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
        if let jump, coordinator.jump != jump, let index = frames.firstIndex(where: { $0.sequence == jump }) {
            coordinator.jump = jump
            table.selectRowIndexes(IndexSet(integer: index), byExtendingSelection: false)
            table.scrollRowToVisible(index)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: Coordinator) {
        coordinator.captureViewport()
    }

    @MainActor
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        var frames: [SerialFrame] = []
        var appearance = TerminalAppearance()
        var matcher: NSRegularExpression?
        var jump: UInt64?
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
            let text = appearance.display(frames[row]).trimmingCharacters(in: .newlines)
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
            default:
                let text = appearance.display(frame).trimmingCharacters(in: .newlines)
                let style = NSMutableAttributedString(string: text, attributes: [
                    .foregroundColor: appearance.color(frame, text: text),
                    .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular),
                ])
                if let matcher, let range = appearance.range(in: text, regex: matcher) {
                    style.addAttribute(.backgroundColor, value: NSColor.systemYellow.withAlphaComponent(0.35), range: range)
                }
                cell.textField?.attributedStringValue = style
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
