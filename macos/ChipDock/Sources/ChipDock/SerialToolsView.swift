import AppKit
import SwiftUI

struct SerialToolsView: View {
    @Bindable var serial: SerialStore
    @Environment(\.dismiss) private var dismiss
    @State private var panel = "checksum"
    @State private var result = ""
    @State private var error = ""
    @State private var running = false
    @State private var input = ""
    @State private var format = SerialDataFormat.hex
    @State private var algorithm = SerialChecksum.crc16Modbus
    @State private var mode = "read"
    @State private var framingValue = ""
    @State private var station = 1
    @State private var function = 3
    @State private var address = 0
    @State private var quantity = 1
    @State private var values = ""
    @State private var timeout = 1000

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Picker("工具", selection: $panel) {
                Text("发送校验").tag("checksum")
                Text("接收分帧").tag("framing")
                Text("Modbus RTU").tag("modbus")
                Text("持续采集").tag("capture")
            }.pickerStyle(.segmented).labelsHidden()
            Divider()
            Form {
                switch panel {
                case "checksum": checksumForm
                case "framing": framingForm
                case "modbus": modbusForm
                default: SerialCaptureForm(serial: serial)
                }
            }.formStyle(.grouped).frame(minHeight: 280)
            if !result.isEmpty {
                GroupBox {
                    ScrollView {
                        Text(result).font(.callout.monospaced()).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .frame(maxHeight: 110)
                } label: {
                    Text("结果")
                }
            }
            if !error.isEmpty {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red).textSelection(.enabled)
            }
            HStack {
                if running { ProgressView().controlSize(.small) }
                Spacer()
                Button("完成") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(20)
        .frame(width: 540)
        .onChange(of: panel) { _, _ in result = ""; error = "" }
        .onAppear {
            input = serial.input
            format = serial.preferences.sendFormat
            mode = serial.preferences.framingMode
            framingValue = serial.preferences.framingValue
        }
    }

    private var checksumForm: some View {
        Group {
            TextField("试算内容", text: $input, axis: .vertical).lineLimit(2...4)
            Picker("编码", selection: $format) { ForEach(SerialDataFormat.allCases) { Text($0.title).tag($0) } }
            Picker("算法", selection: $algorithm) {
                ForEach(SerialChecksum.allCases.filter { $0 != .none }) { Text($0.title).tag($0) }
            }
            TextField("跳过开头字节", value: $serial.preferences.skipPrefix, format: .number)
            TextField("跳过结尾字节", value: $serial.preferences.skipSuffix, format: .number)
            Toggle("HEX 发送时允许附加行尾", isOn: $serial.preferences.hexLineEnding)
            Button("计算") { run {
                let options = ["skipPrefix": serial.preferences.skipPrefix, "skipSuffix": serial.preferences.skipSuffix]
                return try await serial.workbench(["action": "checksum", "input": input, "format": format.rawValue,
                                                   "algorithm": algorithm.rawValue, "options": options])
            }}.disabled(running || input.isEmpty)
        }
    }

    private var framingForm: some View {
        Group {
            Picker("分帧方式", selection: $mode) {
                Text("读取块").tag("read")
                Text("静默间隔").tag("idle")
                Text("帧头").tag("header")
                Text("帧尾").tag("tail")
                Text("固定长度").tag("fixed")
            }
            if mode != "read" {
                TextField(mode == "idle" ? "间隔 (ms)" : mode == "fixed" ? "字节数" : "标记 (HEX)", text: $framingValue)
            }
            Button("应用") { run {
                try await serial.applyFraming(mode: mode, value: framingValue)
                return "已应用分帧设置"
            }}.disabled(running)
        }
    }

    private var modbusForm: some View {
        Group {
            TextField("站号", value: $station, format: .number)
            Picker("功能码", selection: $function) {
                ForEach([1, 2, 3, 4, 5, 6, 15, 16], id: \.self) { code in
                    Text(String(format: "%02X", code)).tag(code)
                }
            }
            TextField("起始地址", value: $address, format: .number)
            if function <= 4 { TextField("读取数量", value: $quantity, format: .number) }
            else { TextField("写入值（十进制，以逗号分隔）", text: $values) }
            TextField("超时 (ms)", value: $timeout, format: .number)
            HStack {
                Button("生成请求") { modbus(send: false) }
                Button("发送并等待响应") { modbus(send: true) }.disabled(!serial.status.connected)
            }.disabled(running)
        }
    }

    private func modbus(send: Bool) {
        run {
            if send && !(10...60_000).contains(timeout) {
                throw SerialClientError.operation("Modbus 超时须为 10–60000 毫秒")
            }
            let values = try (function <= 4 ? [] : values.split(separator: ",").map { text in
                guard let value = UInt16(text.trimmingCharacters(in: .whitespaces)) else {
                    throw SerialClientError.operation("写入值须为 0–65535 的整数")
                }
                return value
            })
            let request: [String: Any] = ["station": station, "function": function, "address": address,
                                         "quantity": function <= 4 ? quantity : 0, "values": values]
            return try await serial.workbench(["action": "modbus", "request": request, "timeout_ms": send ? timeout : 0])
        }
    }

    private func run(_ operation: @escaping @MainActor () async throws -> String) {
        running = true
        result = ""; error = ""
        Task {
            defer { running = false }
            do { result = try await operation() }
            catch { self.error = error.localizedDescription }
        }
    }
}

private struct SerialCaptureForm: View {
    @Bindable var serial: SerialStore
    @State private var path = ""
    @State private var raw = false
    @State private var format = "csv"
    @State private var rollMiB = 16
    @State private var running = false
    @State private var failure = ""

    var body: some View {
        Toggle("仅捕获原始 RX", isOn: $raw)
        if !raw {
            Picker("日志格式", selection: $format) {
                ForEach(["txt", "csv", "bin"], id: \.self) { Text($0.uppercased()).tag($0) }
            }
        }
        HStack {
            TextField("新文件路径", text: $path)
            Button("选择文件夹") {
                let panel = NSOpenPanel()
                panel.canChooseFiles = false; panel.canChooseDirectories = true; panel.canCreateDirectories = true
                panel.begin { result in
                    if result == .OK, let directory = panel.url {
                        path = directory.appendingPathComponent("ChipDock-\(serial.id)-\(Int(Date().timeIntervalSince1970)).\(raw ? "bin" : format)").path
                    }
                }
            }
        }
        TextField("按大小滚动 (MiB)", value: $rollMiB, format: .number)
        let active = raw ? serial.status.capturing : serial.status.logging
        Button(active ? "停止采集" : "开始采集") {
            running = true
            Task {
                defer { running = false }
                do {
                    var action: [String: Any] = ["action": "capture", "raw": raw]
                    if !active {
                        guard (1...16_384).contains(rollMiB) else { throw SerialClientError.operation("滚动大小须为 1–16384 MiB") }
                        action["settings"] = ["path": path, "format": raw ? "bin" : format, "rollBytes": rollMiB * 1024 * 1024] as [String: Any]
                    }
                    _ = try await serial.workbench(action)
                    failure = ""
                } catch { failure = error.localizedDescription }
            }
        }.disabled(running || (!active && path.isEmpty))
        if !failure.isEmpty { Label(failure, systemImage: "exclamationmark.triangle.fill").foregroundStyle(.red) }
    }
}

struct SerialRecordAnalysis: View {
    var serial: SerialStore
    @Environment(\.dismiss) private var dismiss
    @State private var filter = ""
    @State private var direction = SerialDirectionFilter.all
    @State private var selection: SerialFrame.ID?
    @State private var sort = [KeyPathComparator(\SerialFrame.sequence)]

    var body: some View {
        let rows = serial.frames.filter { frame in
            (direction == .all || frame.isReceive == (direction == .receive)) &&
            (filter.isEmpty || "\(frame.hex) \(frame.source) \(frame.analysisText)".localizedCaseInsensitiveContains(filter))
        }.sorted(using: sort)
        VStack(spacing: 12) {
            HStack {
                SearchField(prompt: "筛选文本、HEX 或来源", text: $filter)
                Picker("方向", selection: $direction) {
                    ForEach(SerialDirectionFilter.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented).labelsHidden().fixedSize()
                Text("\(rows.count) 条").monospacedDigit().foregroundStyle(.secondary)
            }
            Table(rows, selection: $selection, sortOrder: $sort) {
                TableColumn("序号", value: \.sequence) { Text($0.sequence.formatted()) }
                TableColumn("时间", value: \.timestamp) { Text($0.time) }
                TableColumn("方向", value: \.direction) { Text($0.isReceive ? "RX" : "TX") }
                TableColumn("来源", value: \.source)
                TableColumn("长度", value: \.analysisLength) { Text($0.bytes.count.formatted()) }
                TableColumn("HEX", value: \.hex)
                TableColumn("文本", value: \.analysisText)
                TableColumn("校验", value: \.analysisChecksum)
            }
            HStack {
                Button("复制") {
                    if let frame = rows.first(where: { $0.id == selection }) {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString("\(frame.sequence)\t\(frame.timestamp)\t\(frame.direction)\t\(frame.source)\t\(frame.hex)\t\(frame.analysisText)", forType: .string)
                    }
                }.disabled(selection == nil)
                Button("在终端定位") {
                    if let frame = rows.first(where: { $0.id == selection }) {
                        serial.jumpToRecord(frame.sequence); dismiss()
                    }
                }.disabled(selection == nil)
                Spacer()
                Button("完成") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }.padding(20).frame(width: 920, height: 480)
    }
}

private extension SerialFrame {
    var analysisLength: Int { bytes.count }
    var analysisText: String { text["utf-8"] ?? "" }
    var analysisChecksum: String {
        let values = parsed.compactMap(\.checksumValid)
        return values.isEmpty ? "—" : values.contains(false) ? "失败" : "通过"
    }
}

struct SerialColorEditor: View {
    @Bindable var serial: SerialStore
    @Environment(\.dismiss) private var dismiss
    @State private var rules: [SerialColorRule] = []
    @State private var tx = ""
    @State private var failure = ""
    private let palette: [(id: String, title: String, color: Color)] = [
        ("red", "红色", .red), ("orange", "橙色", .orange), ("green", "绿色", .green),
        ("blue", "蓝色", .blue), ("purple", "紫色", .purple)
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Menu("预设") {
                    ForEach(SerialColorRule.presets.indices, id: \.self) { index in
                        Button(SerialColorRule.presets[index].0) { rules = SerialColorRule.presets[index].1 }
                    }
                }
                .fixedSize()
                Button("添加规则", systemImage: "plus") { rules.append(.init(pattern: "", regex: false, color: "orange")) }
                    .disabled(rules.count >= 32)
                Spacer()
                Picker("TX 颜色", selection: $tx) {
                    Text("默认").tag("")
                    ForEach(palette, id: \.id) { swatch($0) }
                }.fixedSize()
            }
            List {
                ForEach(rules.indices, id: \.self) { index in
                    HStack {
                        TextField("匹配关键字", text: $rules[index].pattern)
                            .textFieldStyle(.roundedBorder)
                        Toggle("正则", isOn: $rules[index].regex)
                        Picker("颜色", selection: $rules[index].color) {
                            ForEach(palette, id: \.id) { swatch($0) }
                        }
                        .labelsHidden().fixedSize()
                        Button("上移", systemImage: "chevron.up") { rules.swapAt(index, index - 1) }
                            .labelStyle(.iconOnly).disabled(index == 0).help("上移")
                        Button("删除", systemImage: "trash") { rules.remove(at: index) }
                            .labelStyle(.iconOnly).help("删除")
                    }.buttonStyle(.borderless)
                }
            }
            .listStyle(.bordered)
            .alternatingRowBackgrounds()
            .overlay {
                if rules.isEmpty {
                    ContentUnavailableView("没有着色规则", systemImage: "paintpalette",
                                           description: Text("接收数据匹配关键字时按指定颜色显示。"))
                }
            }
            if !failure.isEmpty { Label(failure, systemImage: "exclamationmark.triangle.fill").foregroundStyle(.red) }
            HStack {
                Spacer()
                Button("取消") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("保存") {
                    guard rules.allSatisfy({ !$0.pattern.isEmpty && $0.pattern.utf16.count <= 256 &&
                        (!$0.regex || (try? NSRegularExpression(pattern: $0.pattern)) != nil) }) else {
                        failure = "规则需要 1–256 字符的关键字或有效正则"; return
                    }
                    serial.preferences.colors = rules; serial.preferences.txColor = tx; dismiss()
                }.keyboardShortcut(.defaultAction)
            }
        }.padding(20).frame(width: 620, height: 420)
        .onAppear { rules = serial.preferences.colors; tx = serial.preferences.txColor }
    }

    private func swatch(_ entry: (id: String, title: String, color: Color)) -> some View {
        Label {
            Text(entry.title)
        } icon: {
            Image(systemName: "circle.fill").foregroundStyle(entry.color)
        }
        .tag(entry.id)
    }
}
