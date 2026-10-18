import AppKit
import SwiftUI

enum Theme {
    static let well = Color.primary.opacity(0.05)
    static let mono = Font.system(size: 12, design: .monospaced)
}

/// The system search field, which SwiftUI only offers through `.searchable`
/// in a toolbar.
struct SearchField: NSViewRepresentable {
    var prompt: String
    @Binding var text: String

    func makeNSView(context: Context) -> NSSearchField {
        let field = NSSearchField()
        field.placeholderString = prompt
        field.sendsSearchStringImmediately = true
        field.delegate = context.coordinator
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        return field
    }

    func updateNSView(_ field: NSSearchField, context: Context) {
        context.coordinator.text = $text
        if field.stringValue != text { field.stringValue = text }
    }

    func makeCoordinator() -> Coordinator { Coordinator(text: $text) }

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
