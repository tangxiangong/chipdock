import Foundation

/// Blocking USB and filesystem work stays on a serial queue. UniFFI owns the
/// Rust handle; the separate progress drain only acquires the event queue lock.
final class RustService: @unchecked Sendable {
    private let core = FlashDeskCore()
    private let queue = DispatchQueue(label: "com.xiaoyu.flashdesk.core", qos: .userInitiated)

    var version: String { core.version() }

    func run<T: Sendable>(_ operation: @escaping @Sendable (FlashDeskCore) throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { [core] in
                do { continuation.resume(returning: try operation(core)) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }

    func takeEvents() -> [ProgressEvent] { core.takeEvents() }
}
