import AppKit
import SwiftUI
import XCTest
@testable import FlashDesk

final class WindowLayoutTests: XCTestCase {
    @MainActor
    func testNarrowWindowsAndAuxiliaryPanesDoNotEnterConstraintLoop() async throws {
        let store = WorkbenchStore()
        let host = NSHostingView(rootView: ContentView(store: store))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1160, height: 740),
                              styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = host
        defer { window.close() }

        // This reproduces the minimum-width/inspector combination that previously
        // crashed the application before XCTest could finish launching.
        for width in [960.0, 1160.0] {
            window.setContentSize(NSSize(width: width, height: 740))
            for configuration in 0..<8 {
                store.showInspector = configuration & 1 != 0
                store.showConsole = configuration & 2 != 0
                store.showMemory = configuration & 4 != 0
                window.layoutIfNeeded()
                host.layoutSubtreeIfNeeded()
                try await Task.sleep(for: .milliseconds(40))
                XCTAssertGreaterThan(host.bounds.width, 0)
            }
        }
    }
}
