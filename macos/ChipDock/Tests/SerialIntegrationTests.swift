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
        let batch = try await SerialBackend().poll(after: 0)
        XCTAssertFalse(batch.status.connected)
        release.signal()
        let wasReleased = try await blocked.value
        XCTAssertTrue(wasReleased, "Serial polling must complete before the probe worker times out")
    }

    @MainActor
    func testToolNavigationAndCommandModeKeepSeparateDrafts() {
        let serial = SerialWorkbench()
        let previousPreferences = serial.active.preferences
        defer {
            serial.active.commandMode = false
            serial.active.input = previousPreferences.draft
            serial.active.preferences = previousPreferences
        }
        let toolkit = ToolkitState()
        serial.active.input = "未发送的中文指令"
        serial.active.commandMode = true
        XCTAssertTrue(serial.active.input.isEmpty)
        serial.active.input = "set-temperature value=42"
        for tool in [WorkbenchTool.serial, .flash, .serial] {
            toolkit.selectedTool = tool
            XCTAssertEqual(serial.active.input, "set-temperature value=42")
            serial.active.commandMode = false
            XCTAssertEqual(serial.active.input, "未发送的中文指令")
            serial.active.commandMode = true
            XCTAssertEqual(serial.active.input, "set-temperature value=42")
        }
        XCTAssertFalse(serial.updateBlocked)
    }
}
