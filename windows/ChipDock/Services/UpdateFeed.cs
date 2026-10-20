using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Velopack;

namespace ChipDock;

/// The Windows update feed, shared by the app, the release tool and the tests.
///
/// Like the macOS appcast, each architecture's feed lives on the `update-feed`
/// GitHub release while its package stays on the immutable version release.
/// Windows builds are not code-signed, so the Velopack feed is published inside
/// an envelope that carries an ECDSA P-256 signature over its exact bytes; the
/// feed in turn pins the package's size and SHA-256. One file holds both, so a
/// client never pairs a new feed with an old signature while it is being replaced.
internal static class UpdateFeed
{
    // Keep the deployed Velopack identity so existing installations can upgrade.
    internal const string AppId = "FlashDesk";
    internal const string Repository = "tangxiangong/chipdock";
    internal const string LegacyRepository = "tangxiangong/flashdesk";
    internal static readonly string[] Repositories = [Repository, LegacyRepository];
    internal const string FeedUrl = $"https://github.com/{Repository}/releases/download/update-feed";
    internal const string PrivateKeyVariable = "WINDOWS_UPDATE_PRIVATE_KEY";
    internal static readonly string[] Architectures = ["x64", "arm64"];

    internal static string Channel(string architecture) => Architectures.Contains(architecture)
        ? $"win-{architecture}"
        : throw new ArgumentException($"Unsupported update architecture: {architecture}", nameof(architecture));

    internal static bool IsChannel(string? channel) => Architectures.Any(architecture => channel == $"win-{architecture}");

    /// The plain Velopack feed written by `vpk pack`.
    internal static string FeedFile(string channel) => $"releases.{channel}.json";

    /// The signed envelope that clients read from `update-feed`.
    internal static string SignedFeedFile(string channel) => $"releases.{channel}.signed.json";

    internal static string SignedFeedUrl(string channel, string repository = Repository) =>
        $"https://github.com/{repository}/releases/download/update-feed/{SignedFeedFile(channel)}";

    internal static string PackageUrl(VelopackAsset asset, string repository = Repository) =>
        $"https://github.com/{repository}/releases/download/v{asset.Version}/{asset.FileName}";

    /// Creates a key pair: the public key goes into the project file, the private key into the release secret.
    internal static (string PublicKey, string PrivateKey) CreateKeyPair()
    {
        using var key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        return (Convert.ToBase64String(key.ExportSubjectPublicKeyInfo()), Convert.ToBase64String(key.ExportPkcs8PrivateKey()));
    }

    /// The embedded public key, or null when a build was made without a valid one.
    internal static ECDsa? ImportPublicKey(string? publicKey)
    {
        if (string.IsNullOrWhiteSpace(publicKey)) return null;
        var key = ECDsa.Create();
        try
        {
            var bytes = Convert.FromBase64String(publicKey.Trim());
            key.ImportSubjectPublicKeyInfo(bytes, out var read);
            if (read == bytes.Length && key.KeySize == 256) return key;
        }
        catch (Exception error) when (error is FormatException or CryptographicException) { }
        key.Dispose();
        return null;
    }

    /// Signs a feed that already satisfies <see cref="Parse"/>, so nothing unpublishable is ever sealed.
    internal static byte[] Seal(byte[] feed, string privateKey)
    {
        Parse(feed);
        using var key = ECDsa.Create();
        try { key.ImportPkcs8PrivateKey(Convert.FromBase64String(privateKey.Trim()), out _); }
        catch (FormatException) { throw new CryptographicException($"{PrivateKeyVariable} is not a base64 PKCS#8 key."); }
        if (key.KeySize != 256) throw new CryptographicException($"{PrivateKeyVariable} must be an ECDSA P-256 key.");
        var signature = key.SignData(feed, HashAlgorithmName.SHA256, DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
        return JsonSerializer.SerializeToUtf8Bytes(new Dictionary<string, string>
        {
            ["feed"] = Convert.ToBase64String(feed),
            ["signature"] = Convert.ToBase64String(signature),
        });
    }

    /// Verifies the signature before parsing anything in the feed, then applies <see cref="Parse"/>.
    internal static VelopackAssetFeed Open(byte[] envelope, ECDsa publicKey)
    {
        byte[] feed, signature;
        try
        {
            using var document = JsonDocument.Parse(envelope);
            var root = document.RootElement;
            feed = Convert.FromBase64String(root.GetProperty("feed").GetString() ?? "");
            signature = Convert.FromBase64String(root.GetProperty("signature").GetString() ?? "");
        }
        catch (Exception error) when (error is JsonException or FormatException or KeyNotFoundException or InvalidOperationException)
        {
            throw new InvalidDataException("更新信息格式无效", error);
        }
        if (!publicKey.VerifyData(feed, signature, HashAlgorithmName.SHA256, DSASignatureFormat.IeeeP1363FixedFieldConcatenation))
            throw new InvalidDataException("更新信息签名无效");
        return Parse(feed);
    }

    /// Requires exactly one stable full package for this app, named by a safe basename and pinned by size and SHA-256.
    internal static VelopackAssetFeed Parse(byte[] feed)
    {
        VelopackAssetFeed? parsed;
        try { parsed = VelopackAssetFeed.FromJson(Encoding.UTF8.GetString(feed)); }
        catch (Exception error) when (error is JsonException or FormatException or ArgumentException)
        {
            throw new InvalidDataException("更新信息格式无效", error);
        }
        if (parsed?.Assets is not { Length: 1 } assets) throw new InvalidDataException("更新信息必须只包含一个安装包");
        var asset = assets[0];
        if (asset.Type != VelopackAssetType.Full) throw new InvalidDataException("更新信息只能包含完整安装包");
        if (asset.PackageId != AppId) throw new InvalidDataException($"更新包不属于 {AppId}");
        if (asset.Version is null || asset.Version.IsPrerelease || asset.Version.HasMetadata) throw new InvalidDataException("更新包版本无效");
        RequireSafeFileName(asset.FileName);
        if (asset.Size <= 0 || asset.SHA256 is not { Length: 64 } hash || !hash.All(Uri.IsHexDigit))
            throw new InvalidDataException("更新包缺少大小或 SHA-256");
        return parsed;
    }

    /// Feeds name packages by basename; anything else could redirect a download.
    internal static void RequireSafeFileName(string? fileName)
    {
        if (string.IsNullOrEmpty(fileName) || fileName is "." or ".." || fileName.IndexOfAny(['/', '\\', ':', '?', '#', '%']) >= 0
            || fileName.Any(char.IsControl) || !fileName.EndsWith(".nupkg", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("更新包文件名无效");
    }

    internal static async Task VerifyFile(string path, VelopackAsset asset, CancellationToken cancellation = default)
    {
        await using var file = File.OpenRead(path);
        if (file.Length != asset.Size) throw new InvalidDataException("更新包大小与更新信息不一致");
        var hash = Convert.ToHexString(await SHA256.HashDataAsync(file, cancellation));
        if (!hash.Equals(asset.SHA256, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("更新包 SHA-256 与更新信息不一致");
    }

    /// Publication only moves forward: a newer version, or a rerun with the identical package.
    internal static void RequirePublishable(VelopackAsset candidate, VelopackAsset? previous)
    {
        if (previous is null) return;
        var order = candidate.Version.CompareTo(previous.Version);
        if (order < 0) throw new InvalidOperationException($"Refusing to publish a downgrade: {candidate.Version} < {previous.Version}");
        if (order == 0 && (candidate.FileName != previous.FileName || candidate.Size != previous.Size
            || !candidate.SHA256.Equals(previous.SHA256, StringComparison.OrdinalIgnoreCase)))
            throw new InvalidOperationException($"Refusing to replace the published {candidate.Version} package");
    }
}
