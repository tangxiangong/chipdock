import Foundation

/// Blocking USB and filesystem work runs serially on one dedicated thread.
/// UniFFI owns the Rust handle; the separate progress drain only acquires the
/// event queue lock.
final class RustService: @unchecked Sendable {
    private let core = FlashDeskCore()

    var version: String { core.version() }

    func run<T: Sendable>(_ operation: @escaping @Sendable (FlashDeskCore) throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            CoreThread.shared.submit { [core] in
                do { continuation.resume(returning: try operation(core)) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }

    func takeEvents() -> [ProgressEvent] { core.takeEvents() }
}

/// hidapi schedules its process-wide IOHIDManager on the run loop of whichever
/// thread first enumerates HID probes and never moves it. GCD retires idle
/// workers, so a dispatch queue eventually leaves that manager on a dead run
/// loop and the next enumeration crashes. One long-lived thread keeps it valid.
///
/// The thread runs at default QoS because nusb parks it until its own
/// default-QoS run loop thread completes each transfer; a higher QoS would only
/// create a priority inversion without making USB work faster.
private final class CoreThread: @unchecked Sendable {
    static let shared = CoreThread()

    private let condition = NSCondition()
    private var jobs: [@Sendable () -> Void] = []

    private init() {
        let thread = Thread { [unowned self] in drain() }
        thread.name = "com.xiaoyu.flashdesk.core"
        thread.qualityOfService = .default
        thread.start()
    }

    func submit(_ job: @escaping @Sendable () -> Void) {
        condition.lock()
        jobs.append(job)
        condition.signal()
        condition.unlock()
    }

    private func drain() -> Never {
        while true {
            condition.lock()
            while jobs.isEmpty { condition.wait() }
            let job = jobs.removeFirst()
            condition.unlock()
            job()
        }
    }
}
