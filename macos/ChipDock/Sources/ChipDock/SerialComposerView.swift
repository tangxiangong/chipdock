import SwiftUI
import UniformTypeIdentifiers

struct SerialComposerView: View {
    @Bindable var serial: SerialStore

    @State private var managing = false
    @State private var tools = false
    @State private var workflows = false
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Picker("发送模式", selection: $serial.commandMode) { Text("文本").tag(false); Text("命令").tag(true) }
                    .pickerStyle(.segmented).labelsHidden().fixedSize()
                    .help("文本：直接发送输入内容；命令：按命令规则生成帧")
                if !serial.commandMode { sendOptions }
                Spacer(minLength: 8)
                Menu {
                    Button("命令与接收规则…", systemImage: "list.bullet.rectangle") { managing = true }
                        .disabled(serial.sending)
                    Divider()
                    Button("校验、分帧、Modbus 与采集…", systemImage: "wrench.and.screwdriver") { tools = true }
                    Button("包库、文件、宏与应答…", systemImage: "square.stack.3d.up") { workflows = true }
                    Divider()
                    Button("打开实时曲线", systemImage: "chart.xyaxis.line") {
                        openWindow(id: "serial-plot", value: SerialPlotKey(session: serial.id))
                    }
                } label: {
                    Label("工具", systemImage: "ellipsis.circle")
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .help("串口工具")
            }
            .controlSize(.small)
            .padding(.horizontal, 12)
            .frame(height: 36)
            Divider()
            if serial.commandMode {
                VStack(alignment: .leading, spacing: 8) {
                    TextField("例如 motor-speed 45", text: $serial.input)
                        .textFieldStyle(.roundedBorder).font(.system(.body, design: .monospaced))
                        .onSubmit { Task { await serial.send() } }
                        .onMoveCommand { direction in
                            if direction == .up { serial.recallCommand(-1) }
                            if direction == .down { serial.recallCommand(1) }
                        }
                    let suggestions = serial.protocolRules.filter { !$0.receive && (serial.input.isEmpty || $0.name.hasPrefix(serial.input)) }
                    if !suggestions.isEmpty {
                        ScrollView(.horizontal) {
                            HStack(spacing: 4) {
                                ForEach(suggestions) { rule in
                                    Button(rule.name) { serial.input = rule.name + " " }
                                        .buttonStyle(.accessoryBar)
                                        .help(rule.template)
                                }
                            }
                        }
                        .scrollIndicators(.never)
                        .controlSize(.small)
                    }
                    Label {
                        Text(serial.commandError.isEmpty ? serial.commandPreview : serial.commandError)
                            .font(.caption.monospaced())
                            .textSelection(.enabled)
                    } icon: {
                        Image(systemName: serial.commandError.isEmpty ? "arrow.up.forward" : "exclamationmark.triangle.fill")
                    }
                    .font(.caption)
                    .foregroundStyle(serial.commandError.isEmpty ? Color.secondary : Color.red)
                    .opacity(serial.commandError.isEmpty && serial.commandPreview.isEmpty ? 0 : 1)
                    .help("待发送的字节")
                    if !serial.feedback.isEmpty { Text(serial.feedback).font(.caption).foregroundStyle(.secondary) }
                }
                .padding(.horizontal, 14).padding(.vertical, 10)
                Spacer(minLength: 0)
            } else {
                TextEditor(text: $serial.input)
                    .font(.system(.body, design: .monospaced))
                    .scrollContentBackground(.hidden)
                    .padding(.horizontal, 9)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .overlay(alignment: .topLeading) {
                        if serial.input.isEmpty {
                            Text(serial.preferences.sendFormat == .hex ? "输入十六进制字节，例如 AA 01 FF" : "输入文本或设备指令…")
                                .font(.system(.body, design: .monospaced))
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 14)
                                .padding(.vertical, 6)
                                .allowsHitTesting(false)
                                .accessibilityHidden(true)
                        }
                    }
                    .background(Color(nsColor: .textBackgroundColor))
                    .accessibilityLabel("要发送的数据")
                    .disabled(serial.autoSending)
                    .help(serial.preferences.sendFormat == .hex ? (serial.preferences.hexLineEnding ? "HEX 可附加选定行尾" : "原始字节，不追加行尾") : "⌘Return 发送，Return 换行")
            }
            Divider()
            HStack(spacing: 10) {
                if !serial.commandMode {
                    Menu {
                        ForEach(Array(serial.preferences.history.reversed().enumerated()), id: \.offset) { _, entry in
                            Button(String(entry.prefix(80))) { serial.input = entry }
                        }
                    } label: {
                        Label("发送历史", systemImage: "clock.arrow.circlepath")
                    }
                    .labelStyle(.iconOnly)
                    .menuStyle(.borderlessButton)
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .disabled(serial.preferences.history.isEmpty || serial.autoSending)
                    .help("发送历史")
                    Divider().frame(height: 14)
                    HStack(spacing: 4) {
                        Text("每")
                        TextField("毫秒", text: $serial.autoSendIntervalInput)
                            .textFieldStyle(.roundedBorder)
                            .multilineTextAlignment(.trailing)
                            .monospacedDigit()
                            .frame(width: 64)
                            .foregroundStyle(serial.autoSendInterval == nil ? Color.red : Color.primary)
                            .disabled(serial.autoSending)
                            .accessibilityLabel("自动发送间隔，毫秒")
                            .help("每次发送完成后等待的时间，范围 10–3600000 毫秒。")
                        Text("ms")
                    }
                    .foregroundStyle(.secondary)
                    Toggle(isOn: Binding(get: { serial.autoSending }, set: { _ in serial.toggleAutoSend() })) {
                        Label("自动发送", systemImage: "repeat")
                    }
                    .toggleStyle(.button)
                    .disabled(!serial.autoSending && !serial.canStartAutoSend)
                    .help(serial.autoSending ? "停止自动发送" : "立即发送一次，之后按间隔重复发送；断开连接或发送失败时停止。")
                }
                if let kind = serial.status.jobKind, kind != "repeat" {
                    if serial.status.jobTotal > 0 {
                        ProgressView(value: Double(serial.status.jobDone), total: Double(serial.status.jobTotal))
                            .frame(width: 80)
                    }
                    Text("\(serial.status.jobDone) / \(serial.status.jobTotal)").monospacedDigit().foregroundStyle(.secondary)
                    Button("取消任务") { Task { await serial.cancelJob() } }
                }
                Spacer(minLength: 4)
                if !serial.input.isEmpty {
                    Text("\(serial.input.count) 字符")
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Button { Task { await serial.send() } } label: {
                    Label(serial.sending ? "发送中…" : "发送", systemImage: "paperplane.fill")
                        .frame(minWidth: 64)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.regular)
                .disabled(!serial.canSend)
                .help("发送数据（⌘Return）")
            }
            .font(.callout)
            .controlSize(.small)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .task(id: "\(serial.commandMode):\(serial.input)") { await serial.previewCommand() }
        .sheet(isPresented: $managing) { SerialProtocolEditor(serial: serial) }
        .sheet(isPresented: $tools) { SerialToolsView(serial: serial) }
        .sheet(isPresented: $workflows) { SerialWorkflowView(serial: serial) }
    }

    private var sendOptions: some View {
        HStack(spacing: 8) {
            Picker("编码", selection: $serial.preferences.sendFormat) {
                ForEach(SerialDataFormat.allCases) { Text($0.title).tag($0) }
            }
            .labelsHidden()
            .fixedSize()
            .help("发送编码")
            Picker("行尾", selection: $serial.preferences.lineEnding) {
                Text("无行尾").tag("none")
                Text("LF").tag("lf")
                Text("CR").tag("cr")
                Text("CRLF").tag("crlf")
            }
            .labelsHidden()
            .fixedSize()
            .help("行尾")
            .disabled(serial.preferences.sendFormat == .hex && !serial.preferences.hexLineEnding)
            Menu {
                Picker("校验算法", selection: $serial.preferences.checksum) {
                    Text("无校验（默认）").tag(SerialChecksum.none)
                    Text("校验和（SUM8）").tag(SerialChecksum.sum8)
                    Section("奇偶校验") {
                        Text("奇校验").tag(SerialChecksum.oddParity)
                        Text("偶校验").tag(SerialChecksum.evenParity)
                    }
                    Text("异或校验（XOR8）").tag(SerialChecksum.xor8)
                    Text("CRC16/MODBUS").tag(SerialChecksum.crc16Modbus)
                    Text("CRC16/CCITT").tag(SerialChecksum.crc16Ccitt)
                    Text("CRC32").tag(SerialChecksum.crc32)
                    Text("LRC").tag(SerialChecksum.lrc)
                }
                .pickerStyle(.inline)
            } label: {
                Label(serial.preferences.checksum.title, systemImage: "checkmark.shield")
            }
            .fixedSize()
            .help("对编码后的内容计算校验码，在行尾之前追加；奇偶校验追加 0 或 1，CRC16 使用 MODBUS 格式。")
        }
        .disabled(serial.autoSending)
    }
}

private struct SerialProtocolEditor: View {
    @Bindable var serial: SerialStore
    @Environment(\.dismiss) private var dismiss
    @State private var rules: [SerialProtocolRule] = []
    @State private var selected = 0
    @State private var failure = ""
    @State private var testInput = ""
    @State private var testOutput = ""
    @State private var layout: [SerialProtocolSpan] = []
    @State private var importing = false
    @State private var imported: [SerialProtocolRule]?
    @State private var pendingExport: PendingExport?
    @State private var saving = false

    @State private var testing = false
    @State private var templateTargeted = false

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: "terminal").font(.title2).foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 4) {
                    Text("命令与接收规则").font(.title3.weight(.semibold))
                    Text("定义发送指令，解析设备反馈").font(.callout).foregroundStyle(.secondary)
                }
                Spacer()
                Text("\(rules.count) 条规则").font(.caption).foregroundStyle(.secondary)
            }.padding(20)
            Divider()
            HStack(spacing: 0) {
                sidebar.frame(width: 210)
                Divider()
                if rules.indices.contains(selected) {
                    GeometryReader { geometry in
                        ScrollView {
                            VStack(alignment: .leading, spacing: 20) {
                                identitySection
                                templateSection
                                optionsSection
                                testSection
                            }
                            .textFieldStyle(.roundedBorder)
                            .controlSize(.regular)
                            .frame(width: max(0, geometry.size.width - 40), alignment: .leading)
                            .padding(20)
                        }.frame(width: geometry.size.width, height: geometry.size.height)
                    }
                } else {
                    ContentUnavailableView {
                        Label("添加第一条规则", systemImage: "text.badge.plus")
                    } description: {
                        Text("为常用指令设置名称，或定义接收帧的解析方式。")
                    } actions: {
                        Button("新增发送命令") { addRule(receive: false) }.buttonStyle(.borderedProminent)
                        Button("新增接收规则") { addRule(receive: true) }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            Divider()
            if !failure.isEmpty {
                HStack(alignment: .top, spacing: 8) {
                    Image(systemName: "exclamationmark.circle.fill").foregroundStyle(.red)
                    ScrollView {
                        Text(failure).font(.callout).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading).fixedSize(horizontal: false, vertical: true)
                    }.frame(maxHeight: 52)
                }.padding(.horizontal, 20).padding(.top, 12)
            }
            footer.padding(.horizontal, 20).padding(.vertical, 14)
        }
        .frame(width: 860, height: 650)
        .background(Color(nsColor: .windowBackgroundColor))
        .disabled(saving)
        .onChange(of: selected) { _, _ in testInput = ""; testOutput = ""; failure = "" }
        .onChange(of: rules) { _, _ in testOutput = ""; failure = "" }
        .onAppear { rules = serial.protocolRules }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
            do {
                let url = try result.get(); let access = url.startAccessingSecurityScopedResource()
                defer { if access { url.stopAccessingSecurityScopedResource() } }
                let data = try Data(contentsOf: url)
                guard data.count <= 1024 * 1024 else { throw SerialClientError.operation("配置超过 1 MiB") }
                let values = try JSONDecoder().decode([SerialProtocolRule].self, from: data)
                imported = values
            } catch { failure = error.localizedDescription }
        }
        .confirmationDialog("用导入文件替换当前编辑的全部规则？保存后才会生效。", isPresented: Binding(get: { imported != nil }, set: { if !$0 { imported = nil } })) {
            Button("替换规则", role: .destructive) { rules = imported ?? []; imported = nil; selected = 0 }
            Button("取消", role: .cancel) { imported = nil }
        }
        .exportPresentation($pendingExport)
    }

    private var sidebar: some View {
        VStack(spacing: 0) {
            List(selection: $selected) {
                ForEach(rules.indices, id: \.self) { index in
                    Label {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(rules[index].name).font(.body.weight(.medium)).lineLimit(1).truncationMode(.middle)
                            Text(rules[index].receive ? "接收解析" : "发送命令").font(.caption).foregroundStyle(.secondary)
                        }
                    } icon: {
                        Image(systemName: rules[index].receive ? "arrow.down.circle" : "arrow.up.circle")
                    }.padding(.vertical, 3).help(rules[index].name).tag(index)
                }
            }.listStyle(.sidebar)
            Divider()
            HStack {
                Menu {
                    Button("发送命令", systemImage: "arrow.up.circle") { addRule(receive: false) }
                    Button("接收规则", systemImage: "arrow.down.circle") { addRule(receive: true) }
                } label: { Label("新增", systemImage: "plus") }
                Spacer()
                Button {
                    if rules.indices.contains(selected) { rules.remove(at: selected); selected = max(0, selected - 1) }
                } label: { Image(systemName: "trash") }
                .disabled(!rules.indices.contains(selected)).help("删除所选规则").accessibilityLabel("删除所选规则")
            }.controlSize(.small).padding(12)
        }
    }

    private var identitySection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Picker("规则类型", selection: $rules[selected].receive) {
                Text("发送命令").tag(false)
                Text("接收解析").tag(true)
            }.pickerStyle(.segmented).labelsHidden().frame(width: 220)
                .onChange(of: rules[selected].receive) { _, receive in
                    guard rules.indices.contains(selected) else { return }
                    if receive { rules[selected].response = "" }
                    else { rules[selected].delimiter = "" }
                }
            field("Alias / 名称") {
                TextField("例如 motor-speed", text: $rules[selected].name)
                    .font(.system(.body, design: .monospaced)).accessibilityLabel("Alias / 名称")
            }
        }
    }

    private var templateSection: some View {
        section("帧配置", symbol: "curlybraces") {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 110), alignment: .leading)], alignment: .leading, spacing: 8) {
                ForEach(SerialFrameFragment.allCases) { fragment in
                    Button(fragment.title) { insertFragment(fragment) }
                        .font(.caption.monospaced()).draggable(SerialFrameDrag(fragment: fragment.rawValue))
                        .help(fragment.token)
                }
            }
            Text("拖入下方帧模板，或点击插入。帧头放在开头，参数放在校验前；选择校验会替换已有校验。")
                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            field("帧模板") {
                TextField("AA BB {speed:u16be} {checksum:sum8}", text: $rules[selected].template, axis: .vertical)
                    .lineLimit(2...4).font(.system(.body, design: .monospaced)).accessibilityLabel("帧模板")
            }
            .padding(6)
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(templateTargeted ? Color.accentColor : Color.clear, lineWidth: 2))
            .dropDestination(for: SerialFrameDrag.self) { values, _ in
                let fragments = values.compactMap { SerialFrameFragment(rawValue: $0.fragment) }
                guard !fragments.isEmpty else { return false }
                for fragment in fragments { insertFragment(fragment) }
                return true
            } isTargeted: { templateTargeted = $0 }
            field("显示模板 · 可选") {
                TextField("例如 电机速度：{speed} rpm", text: $rules[selected].display)
                    .accessibilityLabel("显示模板")
            }
            DisclosureGroup("模板语法与示例") {
                VStack(alignment: .leading, spacing: 6) {
                    Text("AA BB {speed:u16be:0:3000} {checksum:sum8}").font(.caption.monospaced()).textSelection(.enabled)
                    Text("整数：u8 / i8，u16 / i16 / u32 / i32 配合 be（大端）或 le（小端）。参数顺序与模板一致。")
                    Text("校验：sum8、xor8、crc16modbus，放在帧尾。")
                }.font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true).padding(.top, 6)
            }.font(.callout)
        }
    }

    private var optionsSection: some View {
        section(rules[selected].receive ? "解析选项" : "校验与反馈", symbol: "checkmark.shield") {
            HStack(alignment: .top, spacing: 16) {
                field("校验起点") {
                    TextField("0", value: $rules[selected].checksum_start, format: .number.grouping(.never))
                        .accessibilityLabel("校验起点，从 0 开始")
                }
                field("校验终点 · 不包含") {
                    TextField("自动，直到校验位前", text: Binding(get: { rules[selected].checksum_end.map(String.init) ?? "" }, set: { value in rules[selected].checksum_end = value.isEmpty ? nil : Int(value) ?? -1 }))
                        .accessibilityLabel("校验终点，不包含，空表示校验位之前")
                }
            }
            HStack {
                field("长度字段 · 可选") { TextField("例如 len", text: $rules[selected].length_field) }
                field("固定帧尾 HEX · 可选") { TextField("例如 0D 0A", text: $rules[selected].trailer) }
            }
            Toggle("长度表示整帧字节数", isOn: $rules[selected].length_includes_frame)
                .disabled(rules[selected].length_field.isEmpty)
            if rules[selected].receive {
                field("文本分隔符 HEX · 可选") {
                    TextField("例如 0D 0A", text: $rules[selected].delimiter)
                        .font(.system(.body, design: .monospaced)).accessibilityLabel("文本分隔符 HEX")
                }
                Text("文本解析时清空帧模板，显示模板使用 {text}。单帧最多 4096 字节。")
                    .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            } else {
                HStack(alignment: .top, spacing: 16) {
                    field("关联反馈") {
                        Picker("关联反馈", selection: $rules[selected].response) {
                            Text("无").tag("")
                            ForEach(rules.filter(\.receive)) { Text($0.name).tag($0.name) }
                        }.labelsHidden().frame(maxWidth: .infinity, alignment: .leading)
                    }
                    field("等待超时 · ms") {
                        TextField("1000", value: $rules[selected].timeout_ms, format: .number.grouping(.never))
                            .accessibilityLabel("反馈超时，毫秒")
                    }
                }
            }
        }
    }

    private var testSection: some View {
        section("离线试算", symbol: "play.circle") {
            HStack(alignment: .bottom, spacing: 12) {
                field(rules[selected].receive ? "输入接收数据 · HEX" : "输入命令 · 包含 alias") {
                    TextField(rules[selected].receive ? "CC DD 00 2D D6" : "motor-speed 45", text: $testInput)
                        .font(.system(.body, design: .monospaced)).accessibilityLabel("离线试算输入")
                }
                if rules[selected].receive, let latest = serial.frames.last(where: { $0.isReceive && !$0.bytes.isEmpty }) {
                    Button("用最新 RX") { testInput = latest.parsed.last(where: { $0.valid })?.hex ?? latest.hex }
                }
                Button(testing ? "处理中…" : rules[selected].receive ? "解析" : "试算", action: runTest)
                    .disabled(testing || testInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            if !testOutput.isEmpty, !layout.isEmpty {
                ScrollView(.horizontal) {
                    HStack {
                        ForEach(Array(layout.enumerated()), id: \.offset) { index, span in
                            VStack(alignment: .leading) {
                                Text("\(span.name) [\(span.start)..<\(span.start + span.bytes)]").font(.caption)
                                Text(span.hex).font(.caption.monospaced()).textSelection(.enabled)
                            }
                            .padding(6)
                            .background([Color.blue, .teal, .orange, .purple][index % 4].opacity(0.15),
                                        in: RoundedRectangle(cornerRadius: 4))
                        }
                    }
                }
            }
            if !testOutput.isEmpty {
                Text(testOutput).font(.callout.monospaced()).textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true).frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12).background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
            } else {
                Text("检查生成的字节或解析结果，不会向设备发送数据。")
                    .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 12) {
            Button { importing = true } label: { Label("导入", systemImage: "square.and.arrow.down") }.help("导入 JSON 规则")
            Button {
                do {
                    let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    pendingExport = PendingExport(data: try encoder.encode(rules), contentType: .json, filename: "ChipDock-Protocol.json", operation: "导出协议")
                } catch { failure = error.localizedDescription }
            } label: { Label("导出", systemImage: "square.and.arrow.up") }.disabled(rules.isEmpty).help("导出 JSON 规则")
            Spacer()
            Button("取消") { dismiss() }.keyboardShortcut(.cancelAction)
            Button(saving ? "保存中…" : "保存规则") {
                saving = true
                Task {
                    do { try await serial.saveRules(rules); dismiss() }
                    catch { failure = error.localizedDescription }
                    saving = false
                }
            }.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
        }
    }

    private func field<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.callout).foregroundStyle(.secondary)
            content()
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private func section<Content: View>(_ title: String, symbol: String, @ViewBuilder content: () -> Content) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                content()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(8)
        } label: {
            Label(title, systemImage: symbol).font(.headline)
        }
    }

    private func insertFragment(_ fragment: SerialFrameFragment) {
        guard rules.indices.contains(selected) else { return }
        rules[selected].template = fragment.inserting(into: rules[selected].template)
    }

    private func addRule(receive: Bool) {
        var rule = SerialProtocolRule(name: (receive ? "receive-" : "command-") + UUID().uuidString.prefix(8))
        rule.receive = receive
        rules.append(rule)
        selected = rules.count - 1
    }

    private func runTest() {
        let snapshot = rules
        let index = selected
        let input = testInput
        let rule = rules[index]
        testing = true
        Task {
            defer { testing = false }
            do {
                let result = try await serial.testRules(snapshot, input: input, rule: rule)
                guard rules == snapshot && selected == index && testInput == input else { return }
                layout = result.layout ?? []
                var lines = [result.hex]
                lines.append(contentsOf: result.records.map { "\($0.name): \($0.message) [\($0.hex)]" })
                if !result.buffer.isEmpty { lines.append("等待剩余字节：" + result.buffer) }
                testOutput = lines.filter { !$0.isEmpty }.joined(separator: "\n")
                failure = ""
            } catch {
                if rules == snapshot && selected == index && testInput == input { failure = error.localizedDescription }
            }
        }
    }

}

private struct SerialFrameDrag: Codable, Transferable {
    let fragment: String
    static var transferRepresentation: some TransferRepresentation {
        CodableRepresentation(contentType: UTType(exportedAs: "com.xiaoyu.chipdock.frame-fragment"))
    }
}
