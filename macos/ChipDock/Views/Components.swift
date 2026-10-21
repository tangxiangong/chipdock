import AppKit
import SwiftUI

enum Theme {
    static let well = Color.primary.opacity(0.05)
    static let mono = Font.system(size: 12, design: .monospaced)
}

/// Only explicitly flexible surfaces, such as the drop zone, absorb spare height.
struct WorkbenchExpansion: LayoutValueKey {
    static let defaultValue: CGFloat = 0
}

/// Keep internal gaps fixed and content aligned to the top. Only explicitly
/// flexible surfaces consume spare height.
struct WorkbenchColumnLayout: Layout {
    var spacing: CGFloat
    var minimumHeight: CGFloat = 0

    private func measurements(_ subviews: Subviews, width: CGFloat?) -> [CGSize] {
        subviews.map { $0.sizeThatFits(ProposedViewSize(width: width, height: nil)) }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil }
        let sizes = measurements(subviews, width: width)
        let naturalHeight = sizes.reduce(0) { $0 + $1.height }
            + spacing * CGFloat(max(subviews.count - 1, 0))
        let proposedHeight = proposal.height.flatMap { $0.isFinite ? $0 : nil } ?? 0
        return CGSize(width: width ?? sizes.map(\.width).max() ?? 0,
                      height: max(naturalHeight, minimumHeight, proposedHeight))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let sizes = measurements(subviews, width: bounds.width)
        let gaps = max(subviews.count - 1, 0)
        let naturalHeight = sizes.reduce(0) { $0 + $1.height } + spacing * CGFloat(gaps)
        let surplus = max(bounds.height - naturalHeight, 0)
        let weights = subviews.map { max($0[WorkbenchExpansion.self], 0) }
        let totalWeight = weights.reduce(0, +)
        var y = bounds.minY
        for index in subviews.indices {
            let height = sizes[index].height + (totalWeight > 0 ? surplus * weights[index] / totalWeight : 0)
            subviews[index].place(at: CGPoint(x: bounds.minX, y: y), anchor: .topLeading,
                                  proposal: ProposedViewSize(width: bounds.width, height: height))
            y += height + spacing
        }
    }
}

/// The system search field, which SwiftUI only offers through `.searchable`
/// in a toolbar.
struct SearchField: NSViewRepresentable {
    var prompt: String
    @Binding var text: String
    var focusRequested: Binding<Bool> = .constant(false)

    func makeNSView(context: Context) -> NSSearchField {
        let field = NSSearchField()
        field.placeholderString = prompt
        field.sendsSearchStringImmediately = true
        field.sendsWholeSearchString = false
        field.delegate = context.coordinator
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setAccessibilityLabel(prompt)
        return field
    }

    func updateNSView(_ field: NSSearchField, context: Context) {
        context.coordinator.text = $text
        if field.stringValue != text { field.stringValue = text }
        if focusRequested.wrappedValue, let window = field.window {
            window.makeFirstResponder(field)
            field.currentEditor()?.selectAll(nil)
            let request = focusRequested
            DispatchQueue.main.async { request.wrappedValue = false }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(text: $text) }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSSearchField, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? 180, height: nsView.intrinsicContentSize.height)
    }

    @MainActor
    final class Coordinator: NSObject, NSSearchFieldDelegate {
        var text: Binding<String>
        init(text: Binding<String>) { self.text = text }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSSearchField else { return }
            text.wrappedValue = field.stringValue
        }
    }
}

extension FirmwareFormat {
    var tint: Color {
        switch self {
        case .elf: .purple
        case .hex: .orange
        case .bin: .teal
        }
    }
}

extension ProgressEvent {
    var tint: Color {
        switch stage {
        case .failed: .red
        case .completed: .green
        default: .accentColor
        }
    }
}

extension ActivityEntry.Level {
    var tint: Color {
        switch self {
        case .info: .secondary
        case .success: .green
        case .error: .red
        }
    }
}

extension MemoryRegionKind {
    var title: String {
        switch self {
        case .nvm: "Flash"
        case .ram: "RAM"
        case .generic: "存储区域"
        }
    }

    var tint: Color {
        switch self {
        case .nvm: .orange
        case .ram: .teal
        case .generic: .gray
        }
    }
}
