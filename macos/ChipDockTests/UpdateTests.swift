import Foundation
import Sparkle
import XCTest
@testable import ChipDock

@MainActor
final class UpdateTests: XCTestCase {
    private let testPublicKey = Data(repeating: 1, count: 32).base64EncodedString()

    func testValidHTTPSFeedAndEd25519PublicKey() throws {
        let configuration = try UpdateConfiguration(feedURL: "https://example.com/appcast.xml", publicKey: testPublicKey)
        XCTAssertEqual(configuration.feedURL.scheme, "https")
        XCTAssertEqual(configuration.publicKey, testPublicKey)
    }

    func testMissingAndInvalidFeedAreRejected() {
        for url in [nil, "", "http://example.com/appcast.xml", "file:///tmp/appcast.xml", "https:"] as [String?] {
            XCTAssertThrowsError(try UpdateConfiguration(feedURL: url, publicKey: testPublicKey))
        }
    }

    func testMissingAndInvalidPublicKeysAreRejected() {
        for key in [nil, "", "not-a-key", Data(repeating: 1, count: 31).base64EncodedString()] as [String?] {
            XCTAssertThrowsError(try UpdateConfiguration(feedURL: "https://example.com/appcast.xml", publicKey: key))
        }
    }

    func testAppContainsStableFeedAndRequiresArchiveVerification() {
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "SUFeedURL") as? String,
                       UpdateConfiguration.primaryFeedURL)
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "SUFallbackFeedURL") as? String,
                       UpdateConfiguration.githubFeedURL)
        XCTAssertNoThrow(try UpdateConfiguration(bundle: .main))
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String, "ChipDock")
        XCTAssertEqual(Bundle.main.bundleIdentifier, "com.xiaoyu.chipdock")
        XCTAssertEqual(Bundle.main.bundleURL.lastPathComponent, "ChipDock.app")
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String, "ChipDock")
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "CFBundleExecutable") as? String, "ChipDock")
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "SUVerifyUpdateBeforeExtraction") as? Bool, true)
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "SUAutomaticallyUpdate") as? Bool, true)
    }

    func testSparkleFrameworkIsEmbeddedInApplication() {
        let path = Bundle.main.bundleURL.appendingPathComponent("Contents/Frameworks/Sparkle.framework")
        XCTAssertTrue(FileManager.default.fileExists(atPath: path.path))
        XCTAssertEqual(Bundle(for: SPUUpdater.self).bundleURL.lastPathComponent, "Sparkle.framework")
    }

    func testSparkleComparesNumericVersionComponents() {
        let comparator = SUStandardVersionComparator.default
        XCTAssertEqual(comparator.compareVersion("0.10.0", toVersion: "0.9.9"), .orderedDescending)
        XCTAssertEqual(comparator.compareVersion("0.1.0", toVersion: "0.1.0"), .orderedSame)
        XCTAssertEqual(comparator.compareVersion("0.0.9", toVersion: "0.1.0"), .orderedAscending)
    }

    func testRelaunchGuardIsExposedToSparkleDelegate() {
        let service = UpdateService()
        XCTAssertTrue(service.responds(to: NSSelectorFromString("updater:shouldPostponeRelaunchForUpdate:untilInvokingBlock:")))
    }

    func testRelaunchIsDeferredUntilHardwareOperationEnds() {
        let state = HardwareOperationState()
        let service = UpdateService(isBusy: { state.busy })
        var installations = 0
        XCTAssertTrue(service.postponeRelaunchIfBusy { installations += 1 })
        service.resumeRelaunchIfIdle()
        XCTAssertEqual(installations, 0)
        state.busy = false
        service.resumeRelaunchIfIdle()
        XCTAssertEqual(installations, 1)
        service.resumeRelaunchIfIdle()
        XCTAssertEqual(installations, 1)
    }

    func testIdleRelaunchContinuesWithoutDeferral() {
        let service = UpdateService()
        XCTAssertFalse(service.postponeRelaunchIfBusy { XCTFail("Sparkle retains the immediate install handler") })
    }

    func testSparkleCannotStartCheckDuringHardwareOperation() {
        let state = HardwareOperationState()
        let service = UpdateService(isBusy: { state.busy })
        let controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: nil, userDriverDelegate: nil)
        XCTAssertThrowsError(try service.updater(controller.updater, mayPerform: .updates))
        state.busy = false
        XCTAssertNoThrow(try service.updater(controller.updater, mayPerform: .updates))
    }
}

@MainActor
private final class HardwareOperationState {
    var busy = true
}
