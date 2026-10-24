import AppKit
import SwiftUI
import XCTest
@testable import ChipDock

final class SerialIntegrationTests: XCTestCase {
    @MainActor
    func testSerialPollDoesNotWaitForTheProbeWorker() async throws {
        let entered = expectation(description: "probe worker entered")
        let release = DispatchSemaphore(value: 0)
        let probe = RustService()
        let blocked = Task {
            try await probe.run { _ in
                entered.fulfill()
                return release.wait(timeout: .now() + 5) == .success
            }
        }
        defer { release.signal() }
        await fulfillment(of: [entered], timeout: 3)
        let batch = try await SerialBackend().poll()
        XCTAssertFalse(batch.status.connected)
        release.signal()
        let wasReleased = try await blocked.value
        XCTAssertTrue(wasReleased, "Serial polling must complete before the probe worker times out")
    }

    @MainActor
    func testPeerToolsKeepTheirStateAcrossNavigationAtMinimumSize() async throws {
        let store = WorkbenchStore()
        let serial = SerialStore()
        let toolkit = ToolkitState()
        serial.input = "未发送的中文指令"
        let host = NSHostingView(rootView: ToolkitView(toolkit: toolkit, store: store, serial: serial))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 640),
                              styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = host
        defer { window.close() }
        for width in [900.0, 1160.0] {
            window.setContentSize(NSSize(width: width, height: 640))
            for tool in [WorkbenchTool.serial, .flash, .serial] {
                toolkit.selectedTool = tool
                window.layoutIfNeeded()
                host.layoutSubtreeIfNeeded()
                try await Task.sleep(for: .milliseconds(40))
                XCTAssertGreaterThan(host.bounds.width, 0)
                XCTAssertEqual(serial.input, "未发送的中文指令")
            }
        }
        XCTAssertFalse(serial.updateBlocked)
    }
}
