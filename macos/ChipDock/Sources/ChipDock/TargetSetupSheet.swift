import SwiftUI

struct TargetSetupSheet: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.dismiss) private var dismiss
    @State private var draft: TargetConfiguration
    @State private var selectingChip = false

    private let speedPresets: [UInt32] = [100, 1000, 4000, 10000, 24000]
    private var canConnect: Bool {
        !store.isBusy && draft.validSpeed != nil && (!store.hasScanned || !store.probes.isEmpty)
    }

    init(store: WorkbenchStore) {
        self.store = store
        _draft = State(initialValue: store.targetConfiguration)
    }

    var body: some View {
        Group {
            if selectingChip {
                ChipSelectionView(rust: store.rust, candidates: store.targetCandidates.map(\.name),
                                  initialChip: draft.chip, cancelTitle: "返回",
                                  confirmTitle: store.targetCandidates.isEmpty ? "选择" : "选择并连接",
                                  onCancel: { selectingChip = false }) { chip in
                    draft.chip = chip
                    selectingChip = false
                    if !store.targetCandidates.isEmpty { connect() }
                }
            } else {
                configurationForm
            }
        }
        .interactiveDismissDisabled(store.isBusy)
        .onChange(of: store.showChipPicker) { _, requested in
            if requested { store.showChipPicker = false; selectingChip = true }
        }
        .focusedSceneValue(\.targetSetupActions, TargetSetupActions(canConnect: canConnect,
                                                                  connect: connect, scan: scan,
                                                                  selectChip: { selectingChip = true }))
    }

    private var configurationForm: some View {
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
                        Picker("调试探针", selection: $draft.selectedProbe) {
                            Text("自动选择").tag("")
                            ForEach(store.probes) { Text($0.displayName).tag($0.identifier) }
                        }
                        Button("扫描探针", systemImage: "arrow.clockwise", action: scan)
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
                        Button { selectingChip = true } label: {
                            HStack(spacing: 4) {
                                Text(draft.chip.isEmpty ? "自动识别" : draft.chip).font(.body.monospaced())
                                Image(systemName: "chevron.up.chevron.down").font(.caption2)
                            }
                        }
                        .buttonStyle(.borderless)
                    }
                    Picker("协议", selection: $draft.wireProtocol) {
                        Text("SWD").tag(WireProtocol.swd)
                        Text("JTAG").tag(WireProtocol.jtag)
                    }
                    .pickerStyle(.segmented)
                    LabeledContent("速度") {
                        HStack(spacing: 6) {
                            TextField("速度", text: $draft.speed)
                                .multilineTextAlignment(.trailing)
                                .monospacedDigit()
                                .labelsHidden()
                                .frame(width: 80)
                            Text("kHz").foregroundStyle(.secondary)
                            Menu {
                                ForEach(speedPresets, id: \.self) { value in
                                    Button("\(value) kHz") { draft.speed = String(value) }
                                }
                            } label: { Image(systemName: "chevron.down") }
                            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                            .accessibilityLabel("常用连接速度")
                        }
                    }
                    if draft.validSpeed == nil {
                        Label("速度须为 1–4294967295 kHz 的整数", systemImage: "exclamationmark.triangle")
                            .font(.caption).foregroundStyle(.red)
                    }
                    Toggle("复位下连接", isOn: $draft.connectUnderReset)
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
                Button("取消", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction).disabled(store.isBusy)
                Button(action: connect) {
                    Label(store.connection == nil ? "连接" : "重新连接", systemImage: "link")
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!canConnect)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 20)
            .padding(.top, 4)
        }
        .frame(width: 480)
    }

    private func scan() {
        Task {
            await store.scanProbes(updateSelection: false)
            if !store.probes.contains(where: { $0.identifier == draft.selectedProbe }) {
                draft.selectedProbe = store.probes.count == 1 ? store.probes[0].identifier : ""
            }
        }
    }

    private func connect() {
        guard let speed = draft.validSpeed, !store.isBusy else { return }
        draft.speed = String(speed)
        store.applyTargetConfiguration(draft)
        Task {
            await store.connect()
            if store.connection != nil { dismiss() }
        }
    }
}

struct TargetSetupActions {
    var canConnect: Bool
    var connect: () -> Void
    var scan: () -> Void
    var selectChip: () -> Void
}

private struct TargetSetupActionsKey: FocusedValueKey { typealias Value = TargetSetupActions }

extension FocusedValues {
    var targetSetupActions: TargetSetupActions? {
        get { self[TargetSetupActionsKey.self] }
        set { self[TargetSetupActionsKey.self] = newValue }
    }
}
