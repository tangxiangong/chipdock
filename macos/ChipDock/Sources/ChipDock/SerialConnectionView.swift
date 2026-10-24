import AppKit
import SwiftUI

/// Communication settings stay in the rail; devices are selected from a menu.
struct SerialConnectionView: View {
    @Bindable var serial: SerialStore

    private var devices: [ParameterChoice] {
        var choices = serial.ports.map { ParameterChoice(value: $0.path, title: $0.displayName) }
        if serial.status.connected, serial.selectedPort == nil, let path = serial.status.path {
            choices.append(ParameterChoice(value: path, title: "\(path)（已移除）"))
        }
        return choices.isEmpty ? [ParameterChoice(value: "", title: "未发现串口")] : choices
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("串口设置", systemImage: "cable.connector")
                .font(.system(size: 14, weight: .semibold))
                .padding(.horizontal, 18)
                .frame(height: 56)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    field("串口设备") {
                        HStack(spacing: 6) {
                            SerialParameterPicker(title: "串口设备", selection: $serial.preferences.selectedPath, choices: devices)
                                .disabled(serial.locked || serial.ports.isEmpty)
                                .help(serial.preferences.selectedPath.isEmpty ? "接入串口设备后自动检测" : serial.preferences.selectedPath)
                            Button { Task { await serial.refreshPorts() } } label: {
                                Label("刷新串口", systemImage: "arrow.clockwise").labelStyle(.iconOnly)
                            }
                            .buttonStyle(.borderless)
                            .disabled(serial.refreshing)
                            .help("刷新串口（⌘R）")
                        }
                    }
                    field("波特率") {
                        HStack(spacing: 6) {
                            TextField("自定义波特率", text: $serial.baudRateInput)
                                .textFieldStyle(.roundedBorder)
                                .accessibilityLabel("波特率")
                                .help("输入 1–4294967295 的整数，或从右侧菜单选择常用值。")
                            Menu {
                                ForEach(SerialBaudRate.presets, id: \.self) { rate in
                                    Button(String(rate)) { serial.baudRateInput = String(rate) }
                                }
                            } label: {
                                Image(systemName: "chevron.down")
                            }
                            .menuStyle(.borderlessButton)
                            .menuIndicator(.hidden)
                            .fixedSize()
                            .accessibilityLabel("常用波特率")
                            .help("选择常用波特率")
                        }
                        .disabled(serial.locked)
                        if !serial.validBaudRate {
                            Text("请输入有效的正整数").font(.caption).foregroundStyle(.red)
                        }
                    }
                    HStack(spacing: 12) {
                        field("数据位") {
                            numberPicker("数据位", selection: $serial.preferences.config.dataBits, values: Array(5...8))
                        }
                        field("停止位") {
                            numberPicker("停止位", selection: $serial.preferences.config.stopBits, values: [1, 2])
                        }
                    }
                    .disabled(serial.locked)
                    field("校验位") {
                        SerialParameterPicker(title: "校验位", selection: $serial.preferences.config.parity, choices: [
                            ParameterChoice(value: "none", title: "无 None"),
                            ParameterChoice(value: "even", title: "偶校验 Even"),
                            ParameterChoice(value: "odd", title: "奇校验 Odd")
                        ])
                        .disabled(serial.locked)
                    }
                    field("流控制") {
                        SerialParameterPicker(title: "流控制", selection: $serial.preferences.config.flowControl, choices: [
                            ParameterChoice(value: "none", title: "无"),
                            ParameterChoice(value: "software", title: "XON / XOFF"),
                            ParameterChoice(value: "hardware", title: "RTS / CTS")
                        ])
                        .disabled(serial.locked)
                    }
                    Button { Task { await serial.toggleConnection() } } label: {
                        HStack(spacing: 8) {
                            if serial.busy {
                                ProgressView().controlSize(.mini)
                            } else {
                                Image(systemName: serial.status.connected ? "stop.fill" : "link")
                            }
                            Text(serial.busy ? "正在处理…" : serial.status.connected ? "断开连接" : "连接串口")
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 4)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(serial.busy || (!serial.status.connected && !serial.canConnect))
                    .help("连接或断开串口（⌘K）")
                }
                .controlSize(.regular)
                .padding(18)
            }
        }
        .frame(maxHeight: .infinity)
    }

    private func field<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title).font(.system(size: 12, weight: .medium)).foregroundStyle(.secondary)
            content().frame(maxWidth: .infinity)
        }
    }

    private func numberPicker(_ title: String, selection: Binding<Int>, values: [Int]) -> some View {
        SerialParameterPicker(
            title: title,
            selection: Binding(
                get: { String(selection.wrappedValue) },
                set: { if let value = Int($0) { selection.wrappedValue = value } }
            ),
            choices: values.map { ParameterChoice(value: String($0), title: String($0)) }
        )
    }
}

private struct ParameterChoice: Equatable {
    let value: String
    let title: String
}

/// NSPopUpButton keeps every parameter field full-width, including long USB names.
private struct SerialParameterPicker: NSViewRepresentable {
    let title: String
    @Binding var selection: String
    let choices: [ParameterChoice]
    @Environment(\.isEnabled) private var isEnabled

    func makeNSView(context: Context) -> NSPopUpButton {
        let button = NSPopUpButton(frame: .zero, pullsDown: false)
        button.cell?.lineBreakMode = .byTruncatingMiddle
        button.setContentHuggingPriority(.defaultLow, for: .horizontal)
        button.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        button.target = context.coordinator
        button.action = #selector(Coordinator.choose(_:))
        button.setAccessibilityLabel(title)
        return button
    }

    func updateNSView(_ button: NSPopUpButton, context: Context) {
        context.coordinator.parent = self
        if context.coordinator.choices != choices {
            button.removeAllItems()
            // Add actual menu items so identical friendly names remain distinct ports.
            for choice in choices {
                let item = NSMenuItem(title: choice.title, action: nil, keyEquivalent: "")
                item.toolTip = choice.value
                button.menu?.addItem(item)
            }
            context.coordinator.choices = choices
        }
        button.selectItem(at: choices.firstIndex { $0.value == selection } ?? -1)
        button.isEnabled = isEnabled
        button.setAccessibilityValue(button.titleOfSelectedItem ?? "")
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSPopUpButton, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? 180, height: nsView.intrinsicContentSize.height)
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    @MainActor
    final class Coordinator: NSObject {
        var parent: SerialParameterPicker
        var choices: [ParameterChoice] = []
        init(_ parent: SerialParameterPicker) { self.parent = parent }

        @objc func choose(_ button: NSPopUpButton) {
            let index = button.indexOfSelectedItem
            guard parent.isEnabled, choices.indices.contains(index) else { return }
            parent.selection = choices[index].value
        }
    }
}
