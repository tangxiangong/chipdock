import SwiftUI

struct SerialComposerView: View {
    @Bindable var serial: SerialStore

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Label("发送区", systemImage: "square.and.pencil")
                    .font(.system(size: 13, weight: .semibold))
                Spacer(minLength: 12)
                Picker("编码", selection: $serial.preferences.sendFormat) {
                    ForEach(SerialDataFormat.allCases) { Text($0.title).tag($0) }
                }
                .frame(width: 132)
                Picker("行尾", selection: $serial.preferences.lineEnding) {
                    Text("无").tag("none")
                    Text("LF").tag("lf")
                    Text("CR").tag("cr")
                    Text("CRLF").tag("crlf")
                }
                .frame(width: 112)
                .disabled(serial.preferences.sendFormat == .hex)
            }
            .controlSize(.small)
            .padding(.horizontal, 18)
            .frame(height: 44)
            TextEditor(text: $serial.input)
                .font(.system(size: 13, design: .monospaced))
                .scrollContentBackground(.hidden)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .overlay(alignment: .topLeading) {
                    if serial.input.isEmpty {
                        Text(serial.preferences.sendFormat == .hex ? "输入十六进制字节，例如 AA 01 FF" : "输入文本或设备指令…")
                            .font(.system(size: 13, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 19)
                            .padding(.vertical, 9)
                            .allowsHitTesting(false)
                            .accessibilityHidden(true)
                    }
                }
                .accessibilityLabel("要发送的数据")
            HStack(spacing: 12) {
                Text(serial.preferences.sendFormat == .hex ? "原始字节 · 不追加行尾" : "⌘Return 发送 · Return 换行")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer(minLength: 4)
                if !serial.input.isEmpty {
                    Text("\(serial.input.count) 字符")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
                Button { Task { await serial.send() } } label: {
                    Label(serial.sending ? "发送中…" : "发送", systemImage: "paperplane.fill")
                        .frame(minWidth: 64)
                }
                .buttonStyle(.borderedProminent)
                .disabled(!serial.canSend)
                .help("发送数据（⌘Return）")
            }
            .padding(.horizontal, 18)
            .padding(.bottom, 14)
        }
        .background(.background)
    }
}
