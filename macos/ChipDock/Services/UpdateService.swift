import Combine
import Foundation
import Observation
import Sparkle

struct UpdateConfiguration: Sendable {
    let feedURL: URL
    let publicKey: String

    init(feedURL: String?, publicKey: String?) throws {
        guard let feedURL, let url = URL(string: feedURL),
              url.scheme == "https", url.host?.isEmpty == false else {
            throw ConfigurationError.invalidFeed
        }
        guard let publicKey, let decoded = Data(base64Encoded: publicKey), decoded.count == 32 else {
            throw ConfigurationError.missingPublicKey
        }
        self.feedURL = url
        self.publicKey = publicKey
    }

    init(bundle: Bundle) throws {
        try self.init(feedURL: bundle.object(forInfoDictionaryKey: "SUFeedURL") as? String,
                      publicKey: bundle.object(forInfoDictionaryKey: "SUPublicEDKey") as? String)
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

/// Sparkle owns update discovery, signature verification, installation and relaunch.
@MainActor
@Observable
final class UpdateService: NSObject, SPUUpdaterDelegate {
    private(set) var ready = false
    private(set) var unavailableReason: String?
    var automaticallyChecksForUpdates = false {
        didSet {
            guard let updater = controller?.updater,
                  updater.automaticallyChecksForUpdates != automaticallyChecksForUpdates else { return }
            updater.automaticallyChecksForUpdates = automaticallyChecksForUpdates
        }
    }

    var automaticallyDownloadsUpdates = false {
        didSet {
            guard let updater = controller?.updater,
                  updater.automaticallyDownloadsUpdates != automaticallyDownloadsUpdates else { return }
            updater.automaticallyDownloadsUpdates = automaticallyDownloadsUpdates
        }
    }

    var canCheckForUpdates: Bool { ready && !isBusy() }

    @ObservationIgnored private var controller: SPUStandardUpdaterController?
    @ObservationIgnored private var observations = Set<AnyCancellable>()
    @ObservationIgnored private var pendingRelaunch: (() -> Void)?
    @ObservationIgnored private var resumeTask: Task<Void, Never>?
    @ObservationIgnored private var feedURL: URL?
    private var starting = false
    private let isBusy: @MainActor () -> Bool

    init(isBusy: @escaping @MainActor () -> Bool = { false }) {
        self.isBusy = isBusy
        super.init()
    }

    func start() async {
        guard !starting, controller == nil, unavailableReason == nil else { return }
        starting = true
        defer { starting = false }
        let configuration: UpdateConfiguration
        do { configuration = try UpdateConfiguration(bundle: .main) }
        catch { unavailableReason = error.localizedDescription; return }

        guard !Task.isCancelled else { return }
        feedURL = configuration.feedURL

        let controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: self, userDriverDelegate: nil)
        self.controller = controller
        do { try controller.updater.start() }
        catch { unavailableReason = error.localizedDescription; return }

        controller.updater.publisher(for: \.canCheckForUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.ready = $0 }
            .store(in: &observations)
        controller.updater.publisher(for: \.automaticallyChecksForUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.automaticallyChecksForUpdates = $0 }
            .store(in: &observations)
        controller.updater.publisher(for: \.automaticallyDownloadsUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.automaticallyDownloadsUpdates = $0 }
            .store(in: &observations)
    }

    func checkForUpdates() {
        guard canCheckForUpdates else { return }
        controller?.checkForUpdates(nil)
    }

    func feedURLString(for updater: SPUUpdater) -> String? {
        feedURL?.absoluteString
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
