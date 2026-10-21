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
    @MainActor
    func testPeerToolMenusRenderBothPages() async throws {
        func find(_ menu: NSMenu, title: String) -> (NSMenu, Int)? {
            for (index, item) in menu.items.enumerated() {
                if item.title == title { return (menu, index) }
                if let submenu = item.submenu, let found = find(submenu, title: title) { return found }
            }
            return nil
        }
        let mainMenu = try XCTUnwrap(NSApp.mainMenu)
        let window = try XCTUnwrap(NSApp.windows.first { $0.title == "ChipDock" })
        let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent(".build/SerialScreenshots")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        window.orderFront(nil)
        var snapshots: [Data] = []
        for (title, filename) in [("串口连接", "serial.png"), ("固件烧录", "flash.png")] {
            let (menu, index) = try XCTUnwrap(find(mainMenu, title: title))
            menu.performActionForItem(at: index)
            try await Task.sleep(for: .milliseconds(200))
            window.layoutIfNeeded()
            let view = try XCTUnwrap(window.contentView?.superview)
            view.layoutSubtreeIfNeeded()
            let bitmap = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
            view.cacheDisplay(in: view.bounds, to: bitmap)
            let data = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
            XCTAssertGreaterThan(data.count, 2000)
            try data.write(to: directory.appendingPathComponent(filename))
            snapshots.append(data)
        }
        XCTAssertNotEqual(snapshots[0], snapshots[1], "Peer tool commands must switch the rendered page")
    }

    @MainActor
    func testTerminalSearchSurvivesToolNavigation() async throws {
        func searchField(in view: NSView) -> NSSearchField? {
            if let field = view as? NSSearchField { return field }
            return view.subviews.lazy.compactMap { searchField(in: $0) }.first
        }
        let toolkit = ToolkitState()
        let host = NSHostingView(rootView: ToolkitView(toolkit: toolkit, store: WorkbenchStore(), serial: SerialStore()))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1160, height: 740),
                              styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = host
        defer { window.close() }
        toolkit.selectedTool = .serial
        host.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(50))
        let original = try XCTUnwrap(searchField(in: host))
        original.stringValue = "设备响应"
        original.delegate?.controlTextDidChange?(Notification(name: NSControl.textDidChangeNotification, object: original))
        for tool in [WorkbenchTool.flash, .serial] {
            toolkit.selectedTool = tool
            host.layoutSubtreeIfNeeded()
            try await Task.sleep(for: .milliseconds(50))
        }
        XCTAssertEqual(try XCTUnwrap(searchField(in: host)).stringValue, "设备响应")
    }

}
