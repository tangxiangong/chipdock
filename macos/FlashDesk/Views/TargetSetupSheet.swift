import SwiftUI

struct TargetSetupSheet: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.dismiss) private var dismiss

    private let speedPresets: [UInt32] = [100, 1000, 4000, 10000, 24000]

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 2) {
                Text("目标配置").font(.headline)
                Text(store.connection == nil ? "未连接" : "已连接 \(store.targetName)")
                    .font(.caption).foregroundStyle(store.connection == nil ? Color.secondary : Color.green)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding([.horizontal, .top], 20)

            Form {
                Section {
                    HStack {
                        Picker("调试探针", selection: $store.selectedProbe) {
                            Text("自动选择").tag("")
                            ForEach(store.probes) { Text($0.displayName).tag($0.identifier) }
                        }
                        Button("扫描探针", systemImage: "arrow.clockwise") { Task { await store.scanProbes() } }
                            .labelStyle(.iconOnly)
                            .help("扫描调试探针（⌘R）")
                    }
                } header: {
                    Text("探针")
                } footer: {
                    if store.hasScanned {
                        Text(store.probes.isEmpty
                             ? "未发现调试探针。请检查 USB 数据线；使用 Apple 芯片的 Mac 笔记本时，请先解锁并允许此配件连接。关闭其他调试工具后重新扫描。"
                             : "已发现 \(store.probes.count) 个调试探针。")
                            .font(.caption).foregroundStyle(store.probes.isEmpty ? Color.orange : Color.secondary)
                        if store.probes.isEmpty {
                            Link("USB 配件授权说明", destination: URL(string: "https://support.apple.com/zh-cn/102282")!)
                                .font(.caption)
                        }
                    }
                }

                Section("目标芯片") {
                    LabeledContent("芯片") {
                        Button { store.showChipPicker = true } label: {
                            HStack(spacing: 4) {
                                Text(store.chip.isEmpty ? "自动识别" : store.chip).font(.body.monospaced())
                                Image(systemName: "chevron.up.chevron.down").font(.caption2)
                            }
                        }
                        .buttonStyle(.borderless)
                    }
                    Picker("协议", selection: $store.wireProtocol) {
                        Text("SWD").tag(WireProtocol.swd)
                        Text("JTAG").tag(WireProtocol.jtag)
                    }
                    .pickerStyle(.segmented)
                    LabeledContent("速度") {
                        HStack(spacing: 6) {
                            TextField("速度", text: $store.speed)
                                .multilineTextAlignment(.trailing)
                                .monospacedDigit()
                                .labelsHidden()
                                .frame(width: 80)
                            Text("kHz").foregroundStyle(.secondary)
                            Menu {
                                ForEach(speedPresets, id: \.self) { value in
                                    Button("\(value) kHz") { store.speed = String(value) }
                                }
                            } label: { Image(systemName: "chevron.down") }
                            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                        }
                    }
                    Toggle("复位下连接", isOn: $store.connectUnderReset)
                }

                if let information = store.targetInformation {
                    Section("已识别") {
                        LabeledContent("型号", value: information.deviceType)
                        if let cpu = information.cpu { LabeledContent("内核", value: cpu) }
                        if let bytes = information.flashSizeBytes { LabeledContent("Flash", value: byteCountLabel(bytes)) }
                    }
                }
            }
            .formStyle(.grouped)
            .scrollDisabled(true)
            .disabled(store.isBusy)
            .fixedSize(horizontal: false, vertical: true)

            HStack {
                if store.connection != nil {
                    Button("断开", systemImage: "xmark.circle", role: .destructive) { store.disconnect() }.disabled(!store.canDisconnect)
                }
                Spacer()
                if store.isBusy { ProgressView().controlSize(.small).padding(.trailing, 4) }
                Button("取消", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button {
                    Task {
                        await store.connect()
                        if store.connection != nil { dismiss() }
                    }
                } label: {
                    Label(store.connection == nil ? "连接" : "重新连接", systemImage: "link")
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!store.canConnect)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 20)
            .padding(.top, 4)
        }
        .frame(width: 480)
        .sheet(isPresented: $store.showChipPicker) { ChipPicker(store: store) }
    }
}
