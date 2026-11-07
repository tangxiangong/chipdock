import AppKit
import Charts
import SwiftUI

struct SerialWorkflowView: View {
    @Bindable var serial: SerialStore
    @Environment(\.dismiss) private var dismiss
    @State private var page = "packets"
    @State private var packets = ""
    @State private var selected = ""
    @State private var path = ""
    @State private var chunk = 4096
    @State private var interval = 50
    @State private var cycles = 1
    @State private var macro = """
    [{"send":{"text":"PING","format":"utf-8"},"wait":{"kind":"text","pattern":"OK"},"timeoutMs":1000}]
    """
    @State private var replies = ""
    @State private var message = ""
    @State private var error = false
    @State private var running = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Picker("工作流", selection: $page) {
                Text("常用包库").tag("packets")
                Text("发送文件").tag("file")
                Text("宏命令").tag("macro")
                Text("自动应答").tag("replies")
            }.pickerStyle(.segmented).labelsHidden()
            Form {
                switch page {
                case "packets": packetForm
                case "file": fileForm
                case "macro": macroForm
                default: replyForm
                }
            }.formStyle(.grouped).frame(minHeight: 280)
            if let kind = serial.status.jobKind {
                HStack {
                    if serial.status.jobTotal > 0 {
                        ProgressView(value: Double(serial.status.jobDone), total: Double(serial.status.jobTotal))
                    }
                    Text("\(kind) · \(serial.status.jobDone) / \(serial.status.jobTotal)").font(.caption.monospacedDigit())
                    Button("取消任务") { Task { await serial.cancelJob() } }
                }
            } else if serial.status.jobTotal > 0 {
                Text("\(serial.status.jobDone) / \(serial.status.jobTotal) · \(serial.status.jobError ?? "已完成")").font(.caption)
            }
            if !message.isEmpty {
                Label(message, systemImage: error ? "exclamationmark.triangle.fill" : "checkmark.circle.fill")
                    .foregroundStyle(error ? Color.red : .secondary).textSelection(.enabled)
            }
            HStack {
                if running { ProgressView().controlSize(.small) }
                Spacer()
                Button("完成") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(20).frame(width: 660)
        .onAppear { packets = serial.preferences.packets; replies = serial.preferences.replies }
        .onChange(of: page) { _, _ in message = "" }
    }

    private var canStart: Bool { serial.status.connected && serial.status.jobKind == nil && !running }

    private var packetForm: some View {
        Group {
            Text("每个包可设置 name、group、note、order、enabled、text、format。format 使用 hex 或 utf-8。")
                .font(.caption).foregroundStyle(.secondary)
            editor($packets)
            TextField("按发送顺序填写包名（逗号分隔，留空发送全部启用包）", text: $selected)
            TextField("包间隔 (ms)", value: $interval, format: .number)
            HStack {
                Button("示例") { packets = """
                [{"name":"读状态","group":"调试","note":"","order":0,"enabled":true,"text":"AA 01","format":"hex"}]
                """ }
                Button("导入 JSON…", action: importPackets)
                Button("保存") { run { try await savePackets(packets); return "已保存包库" } }
                Button("导出 JSON…") { saveText(serial.preferences.packets, as: "ChipDock-Packets.json") }
                Button("批量发送") {
                    run {
                        let entries = try await savePackets(packets)
                        return try await start(["kind": "batch", "packets": entries, "selected":
                            selected.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }, "intervalMs": interval])
                    }
                }.disabled(!canStart)
            }.disabled(running)
        }
    }

    private var fileForm: some View {
        Group {
            HStack {
                TextField("文件路径", text: $path)
                Button("选择文件…") {
                    let panel = NSOpenPanel()
                    panel.begin { result in if result == .OK, let url = panel.url { path = url.path } }
                }
            }
            TextField("块大小 (字节)", value: $chunk, format: .number)
            TextField("块间隔 (ms)", value: $interval, format: .number)
            Button("发送文件") { run { try await start(["kind": "file", "path": path, "chunkBytes": chunk, "intervalMs": interval]) } }
                .disabled(!canStart || path.isEmpty)
        }
    }

    private var macroForm: some View {
        Group {
            Text("步骤可包含 send、delayMs、wait 和 timeoutMs。匹配方式为 text、hex 或 regex。")
                .font(.caption).foregroundStyle(.secondary)
            editor($macro)
            TextField("循环次数", value: $cycles, format: .number)
            HStack {
                Button("运行宏") { run { try await start(["kind": "macro", "steps": array(macro), "cycles": cycles]) } }
                    .disabled(!canStart)
                Button("导出结果 CSV…") {
                    let panel = NSSavePanel(); panel.nameFieldStringValue = "ChipDock-Macro.csv"
                    panel.begin { result in
                        if result == .OK, let url = panel.url {
                            run {
                                _ = try await serial.workbench(["action": "exportMacro", "path": url.path])
                                return "已导出宏结果"
                            }
                        }
                    }
                }.disabled(running)
            }
        }
    }

    private var replyForm: some View {
        Group {
            Text("按顺序匹配，支持 text、hex 或 regex。delayMs 用于延时，enabled 控制是否启用。")
                .font(.caption).foregroundStyle(.secondary)
            editor($replies)
            HStack {
                Button("示例") { replies = """
                [{"name":"pong","enabled":true,"trigger":{"kind":"text","pattern":"PING"},"reply":{"text":"OK","format":"utf-8"},"delayMs":20}]
                """ }
                Button("应用应答规则") { run {
                    _ = try await serial.workbench(["action": "replies", "rules": array(replies)])
                    serial.preferences.replies = replies
                    return "已应用应答规则"
                }}.disabled(running)
            }
        }
    }

    private func editor(_ text: Binding<String>) -> some View {
        TextEditor(text: text)
            .font(.body.monospaced())
            .scrollContentBackground(.hidden)
            .padding(4)
            .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(.separator))
            .frame(minHeight: 132, idealHeight: 160)
    }

    private func array(_ text: String) throws -> [Any] {
        guard text.utf8.count <= 1024 * 1024,
              let value = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [Any] else {
            throw SerialClientError.operation("请输入不超过 1 MiB 的 JSON 数组")
        }
        return value
    }

    @discardableResult
    private func savePackets(_ text: String) async throws -> [Any] {
        let entries = try array(text)
        _ = try await serial.workbench(["action": "validatePackets", "packets": entries])
        serial.preferences.packets = text
        return entries
    }

    private func importPackets() {
        let panel = NSOpenPanel(); panel.allowedContentTypes = [.json]
        panel.begin { result in
            guard result == .OK, let url = panel.url else { return }
            run {
                guard (try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? Int.max) <= 1024 * 1024 else {
                    throw SerialClientError.operation("包库超过 1 MiB")
                }
                let text = try String(contentsOf: url, encoding: .utf8)
                try await savePackets(text); packets = text
                return "已导入包库"
            }
        }
    }

    private func saveText(_ text: String, as name: String) {
        let panel = NSSavePanel(); panel.nameFieldStringValue = name
        panel.begin { result in
            if result == .OK, let url = panel.url {
                run { try text.write(to: url, atomically: true, encoding: .utf8); return "已导出" }
            }
        }
    }

    private func start(_ job: [String: Any]) async throws -> String {
        _ = try await serial.workbench(["action": "startJob", "job": job])
        return "发送任务已启动"
    }

    private func run(_ work: @escaping @MainActor () async throws -> String) {
        running = true; message = ""; error = false
        Task {
            defer { running = false }
            do { message = try await work() }
            catch { self.error = true; message = error.localizedDescription }
        }
    }
}

struct SerialPlotKey: Codable, Hashable {
    let session: UInt64
    var window = UUID()
}

struct SerialPlotView: View {
    var serial: SerialStore
    @State private var mode = "numbers"
    @State private var fields = ""
    @State private var template = ""
    @State private var binary = #"[{"name":"value","offset":0,"type":"u16","littleEndian":false,"scale":1,"bias":0}]"#
    @State private var sample = ""
    @State private var result = ""
    @State private var paused = false
    @State private var seriesFilter = ""
    @State private var points: [Sample] = []
    @State private var cursor: UInt64 = 0
    @State private var nextIndex = 0
    @State private var gaps: UInt64 = 0
    @State private var settings = #"{"mode":"numbers"}"#

    private struct Reading: Decodable {
        let seq: UInt64
        let timestamp: String
        let values: [String: Double]
    }
    private struct Batch: Decodable {
        let nextCursor: UInt64
        let droppedFrames: UInt64
        let points: [Reading]
    }
    private struct Sample: Identifiable {
        let id = UUID()
        let index: Int
        let reading: Reading
        let field: String
        let value: Double
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(serial.status.path ?? "会话 \(serial.id)").font(.headline)
                Spacer()
                Toggle(isOn: $paused) { Label("暂停", systemImage: "pause") }.toggleStyle(.button)
                Button("清空", systemImage: "trash") { points.removeAll(); gaps = 0 }
                Button("导出 CSV…", systemImage: "square.and.arrow.up", action: export).disabled(points.isEmpty)
            }
            HStack {
                Picker("输入", selection: $mode) {
                    Text("数值 / 分隔值").tag("numbers"); Text("键值对").tag("kv")
                    Text("JSON").tag("json"); Text("二进制").tag("binary")
                }.frame(width: 180)
                TextField("字段名，逗号分隔", text: $fields).disabled(mode == "binary")
                TextField("协议规则名，可选", text: $template)
                Button("应用") { Task { await configure(preview: false) } }
            }
            if mode == "binary" {
                TextField("字段配置 JSON", text: $binary, axis: .vertical).lineLimit(2...4).font(.caption.monospaced())
            }
            HStack {
                TextField(mode == "binary" ? "样例 HEX" : "样例文本", text: $sample)
                Button("试算") { Task { await configure(preview: true) } }.disabled(sample.isEmpty)
                Picker("显示字段", selection: $seriesFilter) {
                    Text("全部").tag("")
                    ForEach(Set(points.map(\.field)).sorted(), id: \.self) { Text($0).tag($0) }
                }.frame(width: 145)
            }
            if !result.isEmpty { Text(result).font(.caption.monospaced()).foregroundStyle(.secondary).textSelection(.enabled) }
            if gaps > 0 { Label("已超出保留范围 \(gaps) 条记录", systemImage: "exclamationmark.triangle.fill").foregroundStyle(.orange).font(.caption) }
            Chart(points.filter { seriesFilter.isEmpty || $0.field == seriesFilter }) { item in
                LineMark(x: .value("样本", item.index), y: .value("值", item.value))
                    .foregroundStyle(by: .value("字段", item.field))
            }
            .chartXAxisLabel("样本")
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .overlay {
                if points.isEmpty {
                    ContentUnavailableView("暂无曲线数据", systemImage: "chart.xyaxis.line",
                                           description: Text(paused ? "已暂停采样。" : "收到可解析的数值后会实时绘制。"))
                }
            }
        }
        .padding(16).frame(minWidth: 720, minHeight: 420)
        .task(id: settings) { await poll() }
    }

    private func configure(preview: Bool) async {
        do {
            var config: [String: Any] = ["mode": mode, "template": template]
            if mode == "binary" { config["binary"] = try JSONSerialization.jsonObject(with: Data(binary.utf8)) }
            else { config["fields"] = fields.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty } }
            let request: [String: Any] = ["action": "plot", "config": config, "sample": preview ? sample : "", "hex": mode == "binary"]
            let value = try await serial.workbench(request)
            if preview { result = value }
            else {
                let data = try JSONSerialization.data(withJSONObject: config, options: .sortedKeys)
                points.removeAll(); seriesFilter = ""; settings = String(decoding: data, as: UTF8.self)
                result = ""
            }
        } catch { result = error.localizedDescription }
    }

    private func poll() async {
        while !Task.isCancelled {
            if !paused {
                do {
                    let config = try JSONSerialization.jsonObject(with: Data(settings.utf8))
                    let response = try await serial.workbench(["action": "plot", "config": config, "cursor": cursor])
                    try Task.checkCancellation()
                    if paused { continue }
                    let batch = try JSONDecoder().decode(Batch.self, from: Data(response.utf8))
                    cursor = batch.nextCursor; gaps += batch.droppedFrames
                    for reading in batch.points {
                        nextIndex += 1
                        points.append(contentsOf: reading.values.map { Sample(index: nextIndex, reading: reading, field: $0.key, value: $0.value) })
                    }
                    if points.count > 4096 { points.removeFirst(points.count - 4096) }
                } catch is CancellationError { return }
                catch { result = error.localizedDescription; return }
            }
            do { try await Task.sleep(for: .milliseconds(120)) } catch { return }
        }
    }

    private func export() {
        let panel = NSSavePanel(); panel.nameFieldStringValue = "ChipDock-Plot.csv"
        let snapshot = points
        panel.begin { reply in
            guard reply == .OK, let url = panel.url else { return }
            let csv = "sample,sequence,timestamp,field,value\r\n" + snapshot.map {
                "\($0.index),\($0.reading.seq),\($0.reading.timestamp),\"\($0.field.replacingOccurrences(of: "\"", with: "\"\""))\",\($0.value)\r\n"
            }.joined()
            do { try csv.write(to: url, atomically: true, encoding: .utf8) }
            catch { result = error.localizedDescription }
        }
    }
}
