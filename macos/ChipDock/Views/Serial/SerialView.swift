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
                        .frame(minWidth: 180, idealWidth: 216, maxWidth: 320)
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
        HStack(spacing: 16) {
            Label(serial.busy ? "正在处理…" : serial.status.connected ? "已连接" : "未连接",
                  systemImage: serial.status.connected ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(serial.status.connected ? Color.green : Color.secondary)
            if let path = serial.status.path, serial.status.connected {
                Text(path).lineLimit(1).truncationMode(.middle)
                Text((serial.status.config ?? serial.preferences.config).summary).lineLimit(1)
            }
            Spacer(minLength: 8)
            Label("RX \(serial.status.rxBytes.formatted()) B", systemImage: "arrow.down.left")
            Label("TX \(serial.status.txBytes.formatted()) B", systemImage: "arrow.up.right")
        }
        .font(.system(size: 11, design: .monospaced))
        .foregroundStyle(.secondary)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .accessibilityElement(children: .combine)
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
