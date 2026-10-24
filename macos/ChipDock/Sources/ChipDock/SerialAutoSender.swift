import Foundation
import Observation

@MainActor
@Observable
final class SerialAutoSender {
    static let intervalRange = 10...3_600_000
    private(set) var isRunning = false
    @ObservationIgnored private var task: Task<Void, Never>?
    private var revision = 0

    deinit { task?.cancel() }

    @discardableResult
    func start(intervalMilliseconds: Int, send: @escaping @MainActor () async -> Bool) -> Bool {
        guard Self.intervalRange.contains(intervalMilliseconds) else { return false }
        stop()
        let revision = revision
        isRunning = true
        task = Task { [weak self] in
            defer {
                if let self, self.revision == revision {
                    self.isRunning = false
                    self.task = nil
                }
            }
            while !Task.isCancelled {
                guard await send() else { break }
                do { try await Task.sleep(for: .milliseconds(intervalMilliseconds)) }
                catch { break }
            }
        }
        return true
    }

    func stop() {
        revision &+= 1
        isRunning = false
        task?.cancel()
        task = nil
    }
}
