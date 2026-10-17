import SwiftUI

struct TargetSetupSheet: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            HStack(spacing: 14) {
                Image(systemName: "point.3.connected.trianglepath.dotted")
                    .font(.system(size: 30, weight: .light)).foregroundStyle(Color.accentColor)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 5) {
                    Text("目标配置").font(.title2.weight(.semibold))
                    Text(store.connection == nil ? "未连接" : store.targetName).foregroundStyle(.secondary)
                }
            }
            Form {
                LabeledContent("调试探针") {
                    HStack {
                        Picker("调试探针", selection: $store.selectedProbe) {
                            Text("自动选择").tag("")
                            ForEach(store.probes) { Text($0.displayName).tag($0.identifier) }
                        }
                        .labelsHidden()
                        Button { Task { await store.scanProbes() } } label: { Image(systemName: "arrow.triangle.2.circlepath") }
                            .help("扫描调试探针")
                    }
                }
                LabeledContent("芯片") {
                    Button(store.chip.isEmpty ? "自动识别…" : store.chip) { store.showChipPicker = true }
                        .buttonStyle(.link)
                }
                Picker("协议", selection: $store.wireProtocol) {
                    Text("SWD").tag(WireProtocol.swd)
                    Text("JTAG").tag(WireProtocol.jtag)
                }
                .pickerStyle(.segmented)
                TextField("速度 (kHz)", text: $store.speed).monospacedDigit()
                Toggle("复位下连接", isOn: $store.connectUnderReset)
            }
            .formStyle(.columns)
            .disabled(store.isBusy)
            if store.hasScanned {
                Label("\(store.probes.count) 个可用探针", systemImage: "cable.connector")
                    .font(.caption).foregroundStyle(.secondary)
            }
            if let information = store.targetInformation {
                Divider()
                HStack {
                    if let cpu = information.cpu { Label(cpu, systemImage: "cpu") }
                    Spacer()
                    if let bytes = information.flashSizeBytes { Text("Flash \(byteCountLabel(bytes))") }
                }
                .font(.callout).foregroundStyle(.secondary)
            }
            Divider()
            HStack {
                if store.isBusy { ProgressView().controlSize(.small) }
                if store.connection != nil {
                    Button("断开", role: .destructive) { store.disconnect() }.disabled(store.isBusy)
                }
                Spacer()
                Button("取消", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button("连接") {
                    Task {
                        await store.connect()
                        if store.connection != nil { dismiss() }
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!store.canConnect)
            }
        }
        .padding(28)
        .frame(width: 500)
        .sheet(isPresented: $store.showChipPicker) { ChipPicker(store: store) }
    }
}
