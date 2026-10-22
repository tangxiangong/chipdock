import Combine
import Foundation
import Observation
import Sparkle

struct UpdateConfiguration: Sendable {
    static let primaryFeedURL = "https://app-release-1302963684.cos.ap-guangzhou.myqcloud.com/chipdock/update-feed/appcast.xml"
    static let githubFeedURL = "https://github.com/tangxiangong/chipdock/releases/download/update-feed/appcast.xml"
    let feedURL: URL
    let fallbackFeedURL: URL
    let publicKey: String

    init(feedURL: String?, publicKey: String?, fallbackFeedURL: String? = githubFeedURL) throws {
        guard let feedURL, let url = URL(string: feedURL),
              url.scheme == "https", url.host?.isEmpty == false,
              let fallbackFeedURL, let fallback = URL(string: fallbackFeedURL),
              fallback.scheme == "https", fallback.host?.isEmpty == false else {
            throw ConfigurationError.invalidFeed
        }
        guard let publicKey, let decoded = Data(base64Encoded: publicKey), decoded.count == 32 else {
            throw ConfigurationError.missingPublicKey
        }
        self.feedURL = url
        self.fallbackFeedURL = fallback
        self.publicKey = publicKey
    }

    init(bundle: Bundle) throws {
        try self.init(feedURL: bundle.object(forInfoDictionaryKey: "SUFeedURL") as? String,
                      publicKey: bundle.object(forInfoDictionaryKey: "SUPublicEDKey") as? String,
                      fallbackFeedURL: bundle.object(forInfoDictionaryKey: "SUFallbackFeedURL") as? String)
    }

    enum ConfigurationError: LocalizedError {
        case invalidFeed
        case missingPublicKey

        var errorDescription: String? {
            switch self {
            case .invalidFeed: "更新地址无效"
            case .missingPublicKey: "此构建未配置更新验证公钥"
            }
        }
    }
}

enum UpdateMirrorPolicy {
    static func canRetry(_ error: NSError) -> Bool {
        var failure: NSError? = error
        while let current = failure {
            if current.domain == NSURLErrorDomain && current.code == NSURLErrorCancelled { return false }
            // Sparkle signature, extraction and installation errors must stay visible.
            if current.domain == SUSparkleErrorDomain && ![1000, 1002, 2001].contains(current.code) { return false }
            failure = current.userInfo[NSUnderlyingErrorKey] as? NSError
        }
        return error.domain == NSURLErrorDomain
            || (error.domain == SUSparkleErrorDomain && [1000, 1002, 2001].contains(error.code))
    }
}

@MainActor
private final class MirrorUpdateDriver: SPUStandardUserDriver {
    var shouldRetry: ((NSError) -> Bool)?

    override func showUpdaterError(_ error: Error, acknowledgement: @escaping () -> Void) {
        if shouldRetry?(error as NSError) == true {
            acknowledgement()
        } else {
            super.showUpdaterError(error, acknowledgement: acknowledgement)
        }
    }
}

/// Sparkle owns update discovery, signature verification, installation and relaunch.
@MainActor
@Observable
final class UpdateService: NSObject, SPUUpdaterDelegate {
    private(set) var ready = false
    private(set) var unavailableReason: String?
    var automaticallyChecksForUpdates = false {
        didSet {
            guard let updater = sparkleUpdater,
                  updater.automaticallyChecksForUpdates != automaticallyChecksForUpdates else { return }
            updater.automaticallyChecksForUpdates = automaticallyChecksForUpdates
        }
    }

    var automaticallyDownloadsUpdates = false {
        didSet {
            guard let updater = sparkleUpdater,
                  updater.automaticallyDownloadsUpdates != automaticallyDownloadsUpdates else { return }
            updater.automaticallyDownloadsUpdates = automaticallyDownloadsUpdates
        }
    }

    var canCheckForUpdates: Bool { ready && pendingFallback == nil && !isBusy() }

    @ObservationIgnored private var sparkleUpdater: SPUUpdater?
    @ObservationIgnored private var observations = Set<AnyCancellable>()
    @ObservationIgnored private var pendingRelaunch: (() -> Void)?
    @ObservationIgnored private var resumeTask: Task<Void, Never>?
    @ObservationIgnored private var feedURL: URL?
    @ObservationIgnored private var fallbackFeedURL: URL?
    @ObservationIgnored private var fallbackTask: Task<Void, Never>?
    private var usingFallback = false
    private var pendingFallback: SPUUpdateCheck?
    private var starting = false
    private let isBusy: @MainActor () -> Bool

    init(isBusy: @escaping @MainActor () -> Bool = { false }) {
        self.isBusy = isBusy
        super.init()
    }

    func start() async {
        guard !starting, sparkleUpdater == nil, unavailableReason == nil else { return }
        starting = true
        defer { starting = false }
        let configuration: UpdateConfiguration
        do { configuration = try UpdateConfiguration(bundle: .main) }
        catch { unavailableReason = error.localizedDescription; return }

        guard !Task.isCancelled else { return }
        feedURL = configuration.feedURL
        fallbackFeedURL = configuration.fallbackFeedURL

        let driver = MirrorUpdateDriver(hostBundle: .main, delegate: nil)
        driver.shouldRetry = { [weak self] error in
            self?.usingFallback == false && UpdateMirrorPolicy.canRetry(error)
        }
        let updater = SPUUpdater(hostBundle: .main, applicationBundle: .main, userDriver: driver, delegate: self)
        sparkleUpdater = updater
        do { try updater.start() }
        catch { unavailableReason = error.localizedDescription; return }

        updater.publisher(for: \.canCheckForUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in
                self?.ready = $0
                self?.retryFallbackIfReady()
            }
            .store(in: &observations)
        updater.publisher(for: \.automaticallyChecksForUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.automaticallyChecksForUpdates = $0 }
            .store(in: &observations)
        updater.publisher(for: \.automaticallyDownloadsUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.automaticallyDownloadsUpdates = $0 }
            .store(in: &observations)
    }

    func checkForUpdates() {
        guard canCheckForUpdates else { return }
        sparkleUpdater?.checkForUpdates()
    }

    func feedURLString(for updater: SPUUpdater) -> String? {
        (usingFallback ? fallbackFeedURL : feedURL)?.absoluteString
    }

    func updater(_ updater: SPUUpdater, didFinishUpdateCycleFor updateCheck: SPUUpdateCheck, error: Error?) {
        defer { if pendingFallback == nil { usingFallback = false } }
        guard !usingFallback, let failure = error as NSError?, UpdateMirrorPolicy.canRetry(failure) else { return }
        usingFallback = true
        pendingFallback = updateCheck
        fallbackTask?.cancel()
        // Let Sparkle tear down its driver, and wait if a hardware operation began meanwhile.
        fallbackTask = Task { [weak self] in
            await Task.yield()
            while self?.pendingFallback != nil {
                self?.retryFallbackIfReady()
                guard self?.pendingFallback != nil else { return }
                do { try await Task.sleep(for: .seconds(1)) }
                catch { return }
            }
        }
    }

    private func retryFallbackIfReady() {
        guard let check = pendingFallback, let updater = sparkleUpdater,
              updater.canCheckForUpdates, !isBusy() else { return }
        pendingFallback = nil
        guard check != .updatesInBackground || updater.automaticallyChecksForUpdates else {
            usingFallback = false
            return
        }
        ready = false
        switch check {
        case .updates: updater.checkForUpdates()
        case .updatesInBackground: updater.checkForUpdatesInBackground()
        case .updateInformation: updater.checkForUpdateInformation()
        @unknown default: usingFallback = false
        }
    }

    func updater(_ updater: SPUUpdater, mayPerform updateCheck: SPUUpdateCheck) throws {
        guard !isBusy() else {
            throw NSError(domain: "com.xiaoyu.chipdock.updates", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "请等待硬件操作完成并断开串口后检查更新"])
        }
    }

    func updater(_ updater: SPUUpdater, shouldPostponeRelaunchForUpdate item: SUAppcastItem,
                 untilInvokingBlock installHandler: @escaping () -> Void) -> Bool {
        postponeRelaunchIfBusy(installHandler)
    }

    func postponeRelaunchIfBusy(_ installHandler: @escaping () -> Void) -> Bool {
        guard isBusy() else { return false }
        pendingRelaunch = installHandler
        resumeTask?.cancel()
        resumeTask = Task { [weak self] in
            while self?.pendingRelaunch != nil {
                do { try await Task.sleep(for: .seconds(1)) }
                catch { return }
                self?.resumeRelaunchIfIdle()
            }
        }
        return true
    }

    func resumeRelaunchIfIdle() {
        guard !isBusy(), let install = pendingRelaunch else { return }
        pendingRelaunch = nil
        resumeTask?.cancel()
        resumeTask = nil
        install()
    }
}
