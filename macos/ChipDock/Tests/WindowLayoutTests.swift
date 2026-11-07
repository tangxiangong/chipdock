import XCTest
@testable import ChipDock

final class WindowLayoutTests: XCTestCase {
    func testFlashColumnFollowsTheWindowWidth() {
        XCTAssertEqual(FlashView.columnWidth(in: 900), 810)
        XCTAssertEqual(FlashView.columnWidth(in: 1160), 1044)
        XCTAssertEqual(FlashView.columnWidth(in: 1800), 1620)
        XCTAssertEqual(FlashView.columnWidth(in: 40), 0)
    }
}
