import CryptoKit
import Foundation

@main
struct UpdateFeedTests {
    static func main() throws {
        let executable = CommandLine.arguments[1]
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("FlashDeskUpdateTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let archive = directory.appendingPathComponent("FlashDesk-0.1.0-macOS-universal.zip")
        let appcast = directory.appendingPathComponent("appcast.xml")
        let previous = directory.appendingPathComponent("previous.xml")
        let info = directory.appendingPathComponent("Info.plist")
        let prefix = "https://github.com/tangxiangong/flashdesk/releases/download/v0.1.0/"
        let bytes = Data("signed update fixture".utf8)
        // Ephemeral fixture key: it never enters the Keychain or a key file.
        let signingKey = Curve25519.Signing.PrivateKey()
        let signature = try signingKey.signature(for: bytes).base64EncodedString()
        let publicKey = signingKey.publicKey.rawRepresentation.base64EncodedString()
        var passed = 0

        func feed(version: String = "0.1.0", build: String = "1", signature: String, length: Int = bytes.count,
                  url: String = prefix + archive.lastPathComponent, minimum: String = "14.0") -> String {
            """
            <?xml version="1.0" encoding="UTF-8"?>
            <rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0"><channel><item>
            <sparkle:version>\(build)</sparkle:version><sparkle:shortVersionString>\(version)</sparkle:shortVersionString>
            <sparkle:minimumSystemVersion>\(minimum)</sparkle:minimumSystemVersion>
            <enclosure url="\(url)" sparkle:edSignature="\(signature)" length="\(length)" type="application/octet-stream"/>
            </item></channel></rss>
            """
        }

        func check(_ name: String, feedText: String? = nil, payload: Data = bytes,
                   key: String = publicKey, oldFeed: String? = nil, succeeds: Bool) throws {
            try payload.write(to: archive)
            try (feedText ?? feed(signature: signature)).write(to: appcast, atomically: true, encoding: .utf8)
            let properties = ["CFBundleShortVersionString": "0.1.0", "CFBundleVersion": "1",
                              "SUPublicEDKey": key, "LSMinimumSystemVersion": "14.0"]
            try PropertyListSerialization.data(fromPropertyList: properties, format: .xml, options: 0).write(to: info)
            let process = Process()
            process.executableURL = URL(fileURLWithPath: executable)
            process.arguments = [appcast.path, archive.path, info.path, prefix]
            if let oldFeed {
                try oldFeed.write(to: previous, atomically: true, encoding: .utf8)
                process.arguments?.append(previous.path)
            }
            let output = Pipe()
            process.standardOutput = output
            process.standardError = output
            try process.run()
            process.waitUntilExit()
            let message = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            guard (process.terminationStatus == 0) == succeeds else {
                throw NSError(domain: "UpdateFeedTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(name): \(message)"])
            }
            passed += 1
        }

        try check("valid signed update", succeeds: true)
        try check("modified archive", payload: Data("modified update bytes".utf8), succeeds: false)
        try check("wrong signing key", key: Curve25519.Signing.PrivateKey().publicKey.rawRepresentation.base64EncodedString(), succeeds: false)
        try check("invalid signature", feedText: feed(signature: "invalid"), succeeds: false)
        try check("wrong archive length", feedText: feed(signature: signature, length: bytes.count + 1), succeeds: false)
        try check("wrong version", feedText: feed(version: "0.2.0", signature: signature), succeeds: false)
        try check("wrong build", feedText: feed(build: "2", signature: signature), succeeds: false)
        try check("wrong minimum system", feedText: feed(signature: signature, minimum: "15.0"), succeeds: false)
        try check("wrong download URL", feedText: feed(signature: signature, url: "https://example.com/update.zip"), succeeds: false)
        try check("newer than previous release", oldFeed: feed(version: "0.0.9", signature: signature), succeeds: true)
        try check("semantic downgrade", oldFeed: feed(version: "0.2.0", signature: signature), succeeds: false)
        try check("build downgrade", oldFeed: feed(build: "2", signature: signature), succeeds: false)
        try check("published build replacement", oldFeed: feed(signature: "different"), succeeds: false)
        print("Signed update metadata: \(passed) checks passed")
    }
}
