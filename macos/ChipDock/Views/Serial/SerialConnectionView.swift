import AppKit
import SwiftUI

/// Communication settings stay in the rail; devices are selected from a menu.
struct SerialConnectionView: View {
    @Bindable var serial: SerialStore
    private let baudRates = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600]

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
                        numberPicker("波特率", selection: $serial.preferences.config.baudRate, values: baudRates)
                            .disabled(serial.locked)
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
                    .tint(serial.status.connected ? Color.secondary : Color.accentColor)
                    .disabled(serial.busy || (!serial.status.connected && serial.preferences.selectedPath.isEmpty))
                    .help("连接或断开串口（⌘K）")
                    Text(serial.status.connected ? "断开连接后可修改参数。" : serial.ports.isEmpty ? "接入设备后自动检测串口。" : "选择串口，连接后开始收发。")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    if let port = serial.selectedPort {
                        Divider()
                        DisclosureGroup {
                            VStack(alignment: .leading, spacing: 10) {
                                detail("路径", value: port.path)
                                if let manufacturer = port.manufacturer, !manufacturer.isEmpty {
                                    detail("厂商", value: manufacturer)
                                }
                                if let vid = port.vid, let pid = port.pid {
                                    detail("VID / PID", value: "\(vid) / \(pid)")
                                }
                                if let number = port.serialNumber, !number.isEmpty {
                                    detail("序列号", value: number)
                                }
                            }
                            .padding(.top, 10)
                        } label: {
                            Label("设备信息", systemImage: "info.circle").font(.caption)
                        }
                    }
                }
                .controlSize(.regular)
                .padding(18)
            }
            Spacer(minLength: 0)
            HStack {
                SettingsLink {
                    Label("设置", systemImage: "gearshape").labelStyle(.iconOnly)
                }
                .buttonStyle(.borderless)
                .help("设置（⌘,）")
                Spacer()
                Text("ChipDock")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(18)
        }
        .frame(maxHeight: .infinity)
        .background(.bar)
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

    private func detail(_ title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).foregroundStyle(.secondary)
            Text(value).textSelection(.enabled)
        }
        .font(.caption)
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
        button.font = .systemFont(ofSize: 13)
        button.bezelStyle = .rounded
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
        CGSize(width: proposal.width ?? 180, height: 30)
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

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
