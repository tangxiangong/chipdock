using System.Security.Cryptography;
using System.Text.Json;
using Velopack;

namespace FlashDesk.Tests;

internal static class UpdateFeedTests
{
    internal static readonly (string Public, string Private) Keys = UpdateFeed.CreateKeyPair();

    /// A feed in the shape `vpk pack` writes.
    internal static byte[] Feed(string version = "0.2.0", string? fileName = null, byte[]? package = null,
        string type = "Full", string packageId = "FlashDesk", int count = 1, string? sha256 = null, long? size = null)
    {
        package ??= [1, 2, 3, 4];
        var asset = new Dictionary<string, object>
        {
            ["PackageId"] = packageId,
            ["Version"] = version,
            ["Type"] = type,
            ["FileName"] = fileName ?? $"FlashDesk-{version}-win-x64-full.nupkg",
            ["SHA1"] = Convert.ToHexString(SHA1.HashData(package)),
            ["SHA256"] = sha256 ?? Convert.ToHexString(SHA256.HashData(package)),
            ["Size"] = size ?? package.Length,
            ["NotesMarkdown"] = "## 新功能\n- 自动更新",
            ["NotesHTML"] = "",
        };
        return JsonSerializer.SerializeToUtf8Bytes(new { Assets = Enumerable.Repeat(asset, count) });
    }

    internal static void Run()
    {
        Check.That(UpdateFeed.Channel("x64") == "win-x64" && UpdateFeed.Channel("arm64") == "win-arm64", "architecture channels");
        Check.Throws<ArgumentException>(() => UpdateFeed.Channel("x86"), "unsupported architecture");
        Check.That(UpdateFeed.IsChannel("win-arm64") && !UpdateFeed.IsChannel("win-x64/../x") && !UpdateFeed.IsChannel(null), "channel allow-list");
        Check.That(UpdateFeed.SignedFeedUrl("win-x64") == "https://github.com/tangxiangong/flashdesk/releases/download/update-feed/releases.win-x64.signed.json",
            "feed lives on the update-feed release");

        using var publicKey = UpdateFeed.ImportPublicKey(Keys.Public) ?? throw new InvalidOperationException("FAIL: generated public key imports");
        Check.That(UpdateFeed.ImportPublicKey(null) is null && UpdateFeed.ImportPublicKey("  ") is null, "missing key disables updates");
        Check.That(UpdateFeed.ImportPublicKey("not base64!") is null, "malformed key disables updates");
        using (var other = ECDsa.Create(ECCurve.NamedCurves.nistP384))
            Check.That(UpdateFeed.ImportPublicKey(Convert.ToBase64String(other.ExportSubjectPublicKeyInfo())) is null, "only P-256 keys are accepted");

        var feed = Feed();
        var envelope = UpdateFeed.Seal(feed, Keys.Private);
        var asset = UpdateFeed.Open(envelope, publicKey).Assets.Single();
        Check.That(asset.Version.ToString() == "0.2.0" && asset.Size == 4 && asset.Type == VelopackAssetType.Full, "sealed feed round-trips");
        Check.That(asset.NotesMarkdown?.Contains("自动更新") == true, "release notes survive sealing");
        Check.That(UpdateFeed.PackageUrl(asset) == "https://github.com/tangxiangong/flashdesk/releases/download/v0.2.0/FlashDesk-0.2.0-win-x64-full.nupkg",
            "package downloads from the immutable version release");

        Check.Throws<InvalidDataException>(() => UpdateFeed.Open(Reseal(envelope, feed => feed[^3] ^= 1), publicKey), "tampered feed", "签名无效");
        var (otherPublic, otherPrivate) = UpdateFeed.CreateKeyPair();
        using var otherKey = UpdateFeed.ImportPublicKey(otherPublic)!;
        Check.Throws<InvalidDataException>(() => UpdateFeed.Open(envelope, otherKey), "feed signed by another key", "签名无效");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Open(UpdateFeed.Seal(feed, otherPrivate), publicKey), "secret from another key pair", "签名无效");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Open(feed, publicKey), "unsigned feed", "格式无效");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Open("{\"feed\":\"%%\",\"signature\":\"\"}"u8.ToArray(), publicKey), "malformed envelope", "格式无效");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Open("[]"u8.ToArray(), publicKey), "envelope must be an object", "格式无效");
        Check.Throws<CryptographicException>(() => UpdateFeed.Seal(feed, "not a key"), "malformed private key");

        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(count: 2)), "exactly one package", "只包含一个");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(count: 0)), "empty feed", "只包含一个");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(type: "Delta")), "no delta packages", "完整安装包");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(packageId: "Other")), "package identity", "不属于");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(version: "0.3.0-beta.1")), "no prerelease versions", "版本无效");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(sha256: "")), "SHA-256 required", "SHA-256");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(sha256: new string('z', 64))), "SHA-256 is hex", "SHA-256");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(size: 0)), "size required", "大小");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Parse("not json"u8.ToArray()), "malformed feed", "格式无效");
        foreach (var name in new[] { "../FlashDesk.nupkg", "a/b.nupkg", @"a\b.nupkg", "https://example.com/x.nupkg", "x?.nupkg", "FlashDesk.exe", "..", "" })
            Check.Throws<InvalidDataException>(() => UpdateFeed.Parse(Feed(fileName: name)), $"unsafe package name {name}", "文件名无效");
        Check.Throws<InvalidDataException>(() => UpdateFeed.Seal(Feed(count: 2), Keys.Private), "unpublishable feeds are never sealed");

        Check.That(Publishable("0.2.0", "0.1.0"), "newer version publishes");
        Check.That(Publishable("0.2.0", null), "first release publishes");
        Check.That(Publishable("0.2.0", "0.2.0"), "identical rerun publishes");
        Check.That(!Publishable("0.1.0", "0.2.0"), "downgrade is refused");
        Check.That(!Publishable("0.2.0", "0.2.0", previousPackage: [9, 9]), "replacing published bytes is refused");
        Check.That(Publishable("0.10.0", "0.9.0"), "versions compare numerically");

        VerifyFileChecks().GetAwaiter().GetResult();
    }

    private static byte[] Reseal(byte[] envelope, Action<byte[]> change)
    {
        using var document = JsonDocument.Parse(envelope);
        var feed = Convert.FromBase64String(document.RootElement.GetProperty("feed").GetString()!);
        change(feed);
        return JsonSerializer.SerializeToUtf8Bytes(new Dictionary<string, string>
        {
            ["feed"] = Convert.ToBase64String(feed),
            ["signature"] = document.RootElement.GetProperty("signature").GetString()!,
        });
    }

    private static bool Publishable(string candidate, string? previous, byte[]? previousPackage = null)
    {
        var next = UpdateFeed.Parse(Feed(candidate)).Assets[0];
        var published = previous is null ? null : UpdateFeed.Parse(Feed(previous, package: previousPackage)).Assets[0];
        try { UpdateFeed.RequirePublishable(next, published); return true; }
        catch (InvalidOperationException) { return false; }
    }

    private static async Task VerifyFileChecks()
    {
        var asset = UpdateFeed.Parse(Feed()).Assets[0];
        var path = Path.Combine(Path.GetTempPath(), $"FlashDesk-test-{Guid.NewGuid():N}.nupkg");
        try
        {
            await File.WriteAllBytesAsync(path, [1, 2, 3, 4]);
            await UpdateFeed.VerifyFile(path, asset);
            Check.That(true, "matching package verifies");
            await File.WriteAllBytesAsync(path, [1, 2, 3]);
            await Check.ThrowsAsync<InvalidDataException>(() => UpdateFeed.VerifyFile(path, asset), "size mismatch", "大小");
            await File.WriteAllBytesAsync(path, [1, 2, 3, 5]);
            await Check.ThrowsAsync<InvalidDataException>(() => UpdateFeed.VerifyFile(path, asset), "hash mismatch", "SHA-256");
        }
        finally { File.Delete(path); }
    }
}
