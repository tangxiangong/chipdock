import SwiftUI

struct SerialView: View {
    @Bindable var serial: SerialStore
    @SceneStorage("composerHeight") private var composerHeight = 190.0
    @SceneStorage("serial.connectionExpanded") private var connectionExpanded = true
    @SceneStorage("serial.composerExpanded") private var composerExpanded = true

    var body: some View {
        VStack(spacing: 0) {
            HSplitView {
                if connectionExpanded {
                    SerialConnectionView(serial: serial)
                        .frame(minWidth: 220, idealWidth: 252, maxWidth: 340)
                }
                VStack(spacing: 0) {
                    if let error = serial.error {
                        errorBanner(error)
                        Divider()
                    }
                    VSplitView {
                        SerialTerminalView(serial: serial,
                                           connectionExpanded: $connectionExpanded,
                                           composerExpanded: $composerExpanded)
                            .frame(minHeight: 180, maxHeight: .infinity)
                        if composerExpanded {
                            SerialComposerView(serial: serial)
                                .frame(minHeight: 164, idealHeight: composerHeight, maxHeight: 320)
                                .background {
                                    GeometryReader { geometry in
                                        Color.clear.onDisappear {
                                            if geometry.size.height >= 164 { composerHeight = min(320, geometry.size.height) }
                                        }
                                    }
                                }
                        }
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            Divider()
            statusBar
        }
        .frame(minWidth: 860, minHeight: 600)
        .focusedSceneValue(\.serialConnectionExpanded, $connectionExpanded)
        .focusedSceneValue(\.serialComposerExpanded, $composerExpanded)
    }

    private var statusBar: some View {
        HStack(spacing: 10) {
            HStack(spacing: 6) {
                Circle()
                    .fill(serial.status.connected ? Color.green : serial.status.reconnecting ? Color.orange : Color.secondary.opacity(0.4))
                    .frame(width: 7, height: 7)
                    .accessibilityHidden(true)
                Text(serial.busy ? "正在处理…" : serial.status.reconnecting ? "正在重连…" : serial.status.connected ? "已连接" : "未连接")
                    .foregroundStyle(.primary)
            }
            .fixedSize()
            if let path = serial.status.path, serial.status.connected {
                separator
                Text(path).lineLimit(1).truncationMode(.middle).help(path)
                separator
                Text((serial.status.config ?? serial.preferences.config).summary).lineLimit(1)
            }
            Spacer(minLength: 8)
            if serial.status.logging || serial.status.capturing {
                Label("采集中", systemImage: "record.circle").foregroundStyle(.red).fixedSize()
                separator
            }
            if serial.status.connected {
                Text(Duration.milliseconds(serial.status.uptimeMs), format: .time(pattern: .hourMinuteSecond))
                    .help("连接时长")
                separator
                Text("\(serial.status.rxPerSecond) / \(serial.status.txPerSecond) B/s")
                    .help("RX / TX 速率")
                separator
            }
            Label("\(serial.status.rxBytes.formatted()) B", systemImage: "arrow.down")
                .help("累计接收")
            Label("\(serial.status.txBytes.formatted()) B", systemImage: "arrow.up")
                .help("累计发送")
        }
        .font(.caption)
        .monospacedDigit()
        .labelStyle(.titleAndIcon)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 14)
        .frame(height: 26)
        .background(.bar)
        .accessibilityElement(children: .combine)
    }

    private var separator: some View {
        Divider().frame(height: 12)
    }

    private func errorBanner(_ message: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
                .accessibilityHidden(true)
            Text(message).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
            Button { serial.error = nil } label: {
                Label("关闭错误提示", systemImage: "xmark").labelStyle(.iconOnly)
            }
            .buttonStyle(.borderless)
            .help("关闭错误提示")
        }
        .font(.callout)
        .padding(12)
        .background(.orange.opacity(0.08))
    }
}

private struct SerialSearchRequestKey: FocusedValueKey {
    typealias Value = Binding<Bool>
}
private struct SerialConnectionExpandedKey: FocusedValueKey { typealias Value = Binding<Bool> }
private struct SerialComposerExpandedKey: FocusedValueKey { typealias Value = Binding<Bool> }

extension FocusedValues {
    var serialConnectionExpanded: Binding<Bool>? {
        get { self[SerialConnectionExpandedKey.self] }
        set { self[SerialConnectionExpandedKey.self] = newValue }
    }
    var serialComposerExpanded: Binding<Bool>? {
        get { self[SerialComposerExpandedKey.self] }
        set { self[SerialComposerExpandedKey.self] = newValue }
    }
    var serialSearchRequested: Binding<Bool>? {
        get { self[SerialSearchRequestKey.self] }
        set { self[SerialSearchRequestKey.self] = newValue }
    }
}

struct SerialWorkbenchView: View {
    var workbench: SerialWorkbench

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                ScrollView(.horizontal) {
                    HStack(spacing: 4) {
                        ForEach(workbench.sessions) { session in
                            sessionTab(session)
                        }
                    }
                    .padding(.vertical, 1)
                }
                .scrollIndicators(.never)
                Menu {
                    ForEach(Array(["关闭", "只读", "允许控制"].enumerated()), id: \.offset) { index, label in
                        Button {
                            Task { await workbench.setLocalControl(UInt8(index)) }
                        } label: {
                            if workbench.localControl == UInt8(index) { Label(label, systemImage: "checkmark") }
                            else { Text(label) }
                        }
                    }
                } label: {
                    Label("本地控制", systemImage: "terminal")
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .help("为当前登录用户的 Rust CLI 和 MCP 设置访问权限")
                .disabled(workbench.settingControl)
                Divider().frame(height: 16)
                Button { Task { await workbench.add() } } label: {
                    Label("新建串口会话", systemImage: "plus").labelStyle(.iconOnly)
                }
                .buttonStyle(.borderless)
                .help("新建串口会话")
                .disabled(workbench.changing || workbench.sessions.count >= 16)
            }
            .controlSize(.small)
            .padding(.horizontal, 10)
            .frame(height: 32)
            .background(.bar)
            Divider()
            SerialView(serial: workbench.active).id(workbench.selected)
        }
    }

    /// Scope-bar style tabs, matching Finder and Mail's accessory bars.
    private func sessionTab(_ session: SerialStore) -> some View {
        let selected = workbench.selected == session.id
        let title = session.status.path ?? "会话 \(session.id)"
        return HStack(spacing: 0) {
            Toggle(isOn: Binding(get: { selected }, set: { if $0 { workbench.selected = session.id } })) {
                HStack(spacing: 5) {
                    Circle()
                        .fill(session.status.connected ? Color.green : Color.secondary.opacity(0.4))
                        .frame(width: 6, height: 6)
                        .accessibilityHidden(true)
                    Text(title).lineLimit(1).truncationMode(.middle)
                }
                .frame(maxWidth: 200)
            }
            .toggleStyle(.button)
            .buttonStyle(.accessoryBar)
            .help(title)
            .accessibilityValue(session.status.connected ? "已连接" : "未连接")
            if session.id != 1 && selected {
                Button { Task { await workbench.close(session) } } label: {
                    Label("关闭会话 \(session.id)", systemImage: "xmark").labelStyle(.iconOnly)
                        .imageScale(.small)
                }
                .buttonStyle(.borderless)
                .padding(.horizontal, 4)
                .disabled(workbench.changing)
                .help("关闭会话")
            }
        }
        .contextMenu {
            if session.id != 1 {
                Button("关闭会话", role: .destructive) { Task { await workbench.close(session) } }
                    .disabled(workbench.changing)
            }
        }
    }
}
