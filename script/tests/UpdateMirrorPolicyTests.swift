import Foundation
import Sparkle

@main
struct UpdateMirrorPolicyTests {
    static func main() throws {
        var passed = 0
        func check(_ value: Bool, _ message: String) throws {
            guard value else { throw NSError(domain: "UpdateMirrorPolicyTests", code: 1,
                                             userInfo: [NSLocalizedDescriptionKey: message]) }
            passed += 1
        }
        for code in [NSURLErrorTimedOut, NSURLErrorCannotFindHost, NSURLErrorNotConnectedToInternet] {
            try check(UpdateMirrorPolicy.canRetry(NSError(domain: NSURLErrorDomain, code: code)), "Network failure should retry")
        }
        for code in [1000, 1002, 2001] {
            try check(UpdateMirrorPolicy.canRetry(NSError(domain: SUSparkleErrorDomain, code: code)), "Feed or download transport should retry")
        }
        for code in [1001, 2000, 3000, 3001, 3002, 4005, 4007, 5000] {
            try check(!UpdateMirrorPolicy.canRetry(NSError(domain: SUSparkleErrorDomain, code: code)), "No-update, validation and installation errors must not retry")
        }
        let cancelled = NSError(domain: NSURLErrorDomain, code: NSURLErrorCancelled)
        let wrapped = NSError(domain: "Transport", code: 1, userInfo: [NSUnderlyingErrorKey: cancelled])
        for error in [cancelled, wrapped] {
            let failure = NSError(domain: SUSparkleErrorDomain, code: 2001, userInfo: [NSUnderlyingErrorKey: error])
            try check(!UpdateMirrorPolicy.canRetry(failure), "Nested cancellation must not retry")
        }
        let invalidSignature = NSError(domain: SUSparkleErrorDomain, code: 3001)
        try check(!UpdateMirrorPolicy.canRetry(NSError(domain: NSURLErrorDomain, code: NSURLErrorUnknown,
                                                       userInfo: [NSUnderlyingErrorKey: invalidSignature])), "Wrapped signature error must not retry")
        let key = Data(repeating: 1, count: 32).base64EncodedString()
        let configuration = try UpdateConfiguration(feedURL: UpdateConfiguration.primaryFeedURL, publicKey: key)
        try check(configuration.fallbackFeedURL.absoluteString == UpdateConfiguration.githubFeedURL, "Default fallback must use the renamed GitHub repository")
        do {
            _ = try UpdateConfiguration(feedURL: UpdateConfiguration.primaryFeedURL, publicKey: key, fallbackFeedURL: "http://example.com/appcast.xml")
            try check(false, "Insecure fallback must be rejected")
        } catch UpdateConfiguration.ConfigurationError.invalidFeed { passed += 1 }
        print("macOS update mirror policy: \(passed) checks passed")
    }
}
