import SwiftUI

struct SerialComposerView: View {
    @Bindable var serial: SerialStore

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Label("发送区", systemImage: "square.and.pencil")
                    .font(.system(size: 13, weight: .semibold))
                Spacer(minLength: 12)
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
                    }
                    .pickerStyle(.inline)
                } label: {
                    Label(serial.preferences.checksum.title, systemImage: "checkmark.shield")
                }
                .fixedSize()
                .help("对编码后的内容计算校验码，在行尾之前追加；奇偶校验追加 0 或 1，CRC16 使用 MODBUS 格式。")
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
            .disabled(serial.autoSending)
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
                .disabled(serial.autoSending)
                .help(serial.preferences.sendFormat == .hex ? "原始字节 · 不追加行尾" : "⌘Return 发送 · Return 换行")
            HStack(spacing: 12) {
                HStack(spacing: 6) {
                    Text("间隔").font(.caption).foregroundStyle(.secondary)
                    TextField("毫秒", text: $serial.autoSendIntervalInput)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 72)
                        .foregroundStyle(serial.autoSendInterval == nil ? Color.red : Color.primary)
                        .disabled(serial.autoSending)
                        .accessibilityLabel("自动发送间隔，毫秒")
                        .help("每次发送完成后等待的时间，范围 10–3600000 毫秒。")
                    Text("ms").font(.caption).foregroundStyle(.secondary)
                }
                Button { serial.toggleAutoSend() } label: {
                    Label(serial.autoSending ? "停止自动发送" : "自动发送", systemImage: serial.autoSending ? "stop.fill" : "repeat")
                }
                .buttonStyle(.bordered)
                .disabled(!serial.autoSending && !serial.canStartAutoSend)
                .help(serial.autoSending ? "停止自动发送" : "立即发送一次，之后按间隔重复发送；断开连接或发送失败时停止。")
                Spacer(minLength: 4)
                if !serial.input.isEmpty {
                    Text("\(serial.input.count) 字符")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
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
    }
}
