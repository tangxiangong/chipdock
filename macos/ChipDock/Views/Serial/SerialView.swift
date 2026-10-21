import SwiftUI

struct SerialView: View {
    @Bindable var serial: SerialStore
    @SceneStorage("composerHeight") private var composerHeight = 190.0
    @SceneStorage("serial.connectionExpanded") private var connectionExpanded = true
    @SceneStorage("serial.composerExpanded") private var composerExpanded = true
    @State private var dragOrigin: Double?

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                if connectionExpanded {
                    SerialConnectionView(serial: serial)
                        .frame(width: 216)
                    Divider()
                }
                VStack(spacing: 0) {
                    if let error = serial.error {
                        errorBanner(error)
                        Divider()
                    }
                    GeometryReader { geometry in
                        let maximum = min(320.0, max(164.0, geometry.size.height - 188))
                        let height = min(max(164.0, composerHeight), maximum)
                        VStack(spacing: 0) {
                            SerialTerminalView(serial: serial,
                                               connectionExpanded: $connectionExpanded,
                                               composerExpanded: $composerExpanded)
                                .frame(minHeight: 180, maxHeight: .infinity)
                            if composerExpanded {
                                resizeHandle(height: height, maximum: maximum)
                                SerialComposerView(serial: serial)
                                    .frame(height: height)
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
    }

    private func resizeHandle(height: Double, maximum: Double) -> some View {
        Rectangle()
            .fill(.quaternary.opacity(0.5))
            .frame(height: 8)
            .overlay { Capsule().fill(.secondary).frame(width: 28, height: 2) }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { value in
                    let origin = dragOrigin ?? height
                    if dragOrigin == nil { dragOrigin = origin }
                    composerHeight = min(max(164, origin - value.translation.height), maximum)
                }
                .onEnded { _ in dragOrigin = nil })
            .focusable()
            .onKeyPress(.upArrow) {
                composerHeight = min(height + 16, maximum)
                return .handled
            }
            .onKeyPress(.downArrow) {
                composerHeight = max(164, height - 16)
                return .handled
            }
            .accessibilityElement()
            .accessibilityLabel("调整发送区高度")
            .accessibilityValue("\(Int(height)) 点")
            .accessibilityAdjustableAction { direction in
                switch direction {
                case .increment: composerHeight = min(height + 16, maximum)
                case .decrement: composerHeight = max(164, height - 16)
                @unknown default: break
                }
            }
            .help("拖动调整收发区域；方向键微调")
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
        .background(.bar)
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

extension FocusedValues {
    var serialSearchRequested: Binding<Bool>? {
        get { self[SerialSearchRequestKey.self] }
        set { self[SerialSearchRequestKey.self] = newValue }
    }
}
