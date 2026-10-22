import CryptoKit
import Foundation

struct UpdateVerificationError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

func require(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    guard condition() else { throw UpdateVerificationError(message: message) }
}

func versionParts(_ version: String) throws -> [UInt64] {
    let parts = version.split(separator: ".", omittingEmptySubsequences: false)
    let values = parts.compactMap { UInt64($0) }
    try require(parts.count == 3 && values.count == 3, "Invalid release version: \(version)")
    return values
}

func field(_ item: XMLElement, _ name: String) -> String {
    item.elements(forName: "sparkle:\(name)").first?.stringValue ?? ""
}

func feedItems(_ path: String) throws -> [XMLElement] {
    let document = try XMLDocument(contentsOf: URL(fileURLWithPath: path), options: [.nodeLoadExternalEntitiesNever])
    return try document.nodes(forXPath: "/rss/channel/item").compactMap { $0 as? XMLElement }
}

func verify() throws {
    let arguments = CommandLine.arguments
    try require((5...6).contains(arguments.count), "usage: verify_update appcast archive Info.plist download-prefix [previous-appcast]")
    let plist = try PropertyListSerialization.propertyList(from: Data(contentsOf: URL(fileURLWithPath: arguments[3])), format: nil)
    guard let info = plist as? [String: Any],
          let version = info["CFBundleShortVersionString"] as? String,
          let build = info["CFBundleVersion"] as? String,
          let publicKey = info["SUPublicEDKey"] as? String,
          let keyBytes = Data(base64Encoded: publicKey) else {
        throw UpdateVerificationError(message: "Missing app version or public key")
    }
    let bundleURL = URL(fileURLWithPath: arguments[3]).deletingLastPathComponent().deletingLastPathComponent()
    try require(bundleURL.lastPathComponent == "ChipDock.app", "Release application bundle must be named ChipDock.app")
    for (name, expected) in ["CFBundleIdentifier": "com.xiaoyu.chipdock", "CFBundleName": "ChipDock",
                             "CFBundleDisplayName": "ChipDock", "CFBundleExecutable": "ChipDock"] {
        try require(info[name] as? String == expected, "Unexpected application identity: \(name) must be \(expected)")
    }
    let candidateVersion = try versionParts(version)
    let items = try feedItems(arguments[1])
    try require(items.count == 1, "Expected exactly one release in the generated feed")
    let item = items[0]
    try require(field(item, "shortVersionString") == version, "Feed and application versions differ")
    try require(field(item, "version") == build, "Feed and application build numbers differ")
    try require(field(item, "minimumSystemVersion") == info["LSMinimumSystemVersion"] as? String,
                "Feed and application minimum macOS versions differ")
    guard let enclosure = item.elements(forName: "enclosure").first,
          let encodedSignature = enclosure.attribute(forName: "sparkle:edSignature")?.stringValue,
          let signature = Data(base64Encoded: encodedSignature),
          let lengthText = enclosure.attribute(forName: "length")?.stringValue,
          let length = Int(lengthText) else {
        throw UpdateVerificationError(message: "Missing archive signature or length")
    }
    let archiveURL = URL(fileURLWithPath: arguments[2])
    let expectedURL = arguments[4] + archiveURL.lastPathComponent
    try require(expectedURL.hasPrefix("https://github.com/"), "Release download must use HTTPS on GitHub")
    try require(enclosure.attribute(forName: "url")?.stringValue == expectedURL, "Unexpected archive download URL")
    let archive = try Data(contentsOf: archiveURL, options: .mappedIfSafe)
    try require(archive.count == length, "Archive length differs from the feed")
    let verifier = try Curve25519.Signing.PublicKey(rawRepresentation: keyBytes)
    try require(verifier.isValidSignature(signature, for: archive), "Archive Ed25519 signature is invalid")

    if arguments.count == 6 {
        for previous in try feedItems(arguments[5]) {
            let previousVersion = try versionParts(field(previous, "shortVersionString"))
            try require(!candidateVersion.lexicographicallyPrecedes(previousVersion), "Refusing to publish a semantic downgrade")
            if previousVersion == candidateVersion {
                let oldBuild = field(previous, "version")
                try require(build.compare(oldBuild, options: .numeric) != .orderedAscending, "Refusing to publish an older build")
                if build == oldBuild {
                    let oldSignature = previous.elements(forName: "enclosure").first?.attribute(forName: "sparkle:edSignature")?.stringValue
                    try require(oldSignature == encodedSignature, "Refusing to replace an already published build")
                }
            }
        }
    }
    print("Verified Sparkle update \(version) (build \(build)): Ed25519 signature, size and release metadata")
}

do { try verify() }
catch {
    FileHandle.standardError.write(Data("error: \(error.localizedDescription)\n".utf8))
    exit(1)
}
