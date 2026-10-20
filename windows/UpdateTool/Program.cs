using System.Security.Cryptography;
using System.Xml.Linq;
using ChipDock;
using Velopack;

// Usage:
//   keygen <private-key-file>
//       Writes a new private key to a file that must not exist yet and prints the public key.
//   seal <directory> <architecture> <public-key | ChipDock.csproj>
//       Signs releases.win-<arch>.json with WINDOWS_UPDATE_PRIVATE_KEY into
//       releases.win-<arch>.signed.json, after proving the secret matches the shipped public key.
//   verify <directory> <architecture> <version> <public-key | ChipDock.csproj> [previous-signed-feed]
//       Checks the signed feed and its package, and that it may replace the previously published feed.
try
{
    switch (args)
    {
        case ["keygen", var path]:
            KeyGen(path);
            break;
        case ["seal", var directory, var architecture, var key]:
            Seal(directory, architecture, key);
            break;
        case ["verify", var directory, var architecture, var version, var key]:
            await Verify(directory, architecture, version, key, null);
            break;
        case ["verify", var directory, var architecture, var version, var key, var previous]:
            await Verify(directory, architecture, version, key, previous);
            break;
        default:
            Console.Error.WriteLine("usage: keygen <file> | seal <dir> <arch> <key> | verify <dir> <arch> <version> <key> [previous]");
            return 2;
    }
    return 0;
}
catch (Exception error) when (error is InvalidDataException or InvalidOperationException or CryptographicException
    or IOException or ArgumentException)
{
    Console.Error.WriteLine($"error: {error.Message}");
    return 1;
}

static void KeyGen(string path)
{
    var (publicKey, privateKey) = UpdateFeed.CreateKeyPair();
    // CreateNew refuses to replace a key that may already protect published updates.
    using (var file = new FileStream(path, FileMode.CreateNew, FileAccess.Write))
    using (var writer = new StreamWriter(file)) writer.Write(privateKey);
    Console.WriteLine(publicKey);
    Console.Error.WriteLine($"Private key written to {path}. Store it as the {UpdateFeed.PrivateKeyVariable} secret, then delete the file.");
}

static void Seal(string directory, string architecture, string key)
{
    var privateKey = Environment.GetEnvironmentVariable(UpdateFeed.PrivateKeyVariable);
    if (string.IsNullOrWhiteSpace(privateKey)) throw new InvalidOperationException($"{UpdateFeed.PrivateKeyVariable} is not set.");
    using var publicKey = PublicKey(key);
    var channel = UpdateFeed.Channel(architecture);
    var envelope = UpdateFeed.Seal(File.ReadAllBytes(Path.Combine(directory, UpdateFeed.FeedFile(channel))), privateKey);
    // A secret from another key pair would publish feeds every installed copy rejects.
    try { UpdateFeed.Open(envelope, publicKey); }
    catch (InvalidDataException)
    {
        throw new InvalidOperationException($"{UpdateFeed.PrivateKeyVariable} does not match the app's UpdatePublicKey.");
    }
    File.WriteAllBytes(Path.Combine(directory, UpdateFeed.SignedFeedFile(channel)), envelope);
    Console.WriteLine($"Signed {UpdateFeed.SignedFeedFile(channel)}");
}

static async Task Verify(string directory, string architecture, string version, string key, string? previousPath)
{
    using var publicKey = PublicKey(key);
    var channel = UpdateFeed.Channel(architecture);
    var asset = UpdateFeed.Open(File.ReadAllBytes(Path.Combine(directory, UpdateFeed.SignedFeedFile(channel))), publicKey).Assets[0];
    if (asset.Version.ToString() != version) throw new InvalidDataException($"Feed version {asset.Version} does not match {version}");
    var expected = $"{UpdateFeed.AppId}-{version}-{channel}-full.nupkg";
    if (asset.FileName != expected) throw new InvalidDataException($"Feed package {asset.FileName} is not {expected}");
    await UpdateFeed.VerifyFile(Path.Combine(directory, asset.FileName), asset);
    VelopackAsset? previous = null;
    if (previousPath is not null)
    {
        try { previous = UpdateFeed.Open(File.ReadAllBytes(previousPath), publicKey).Assets[0]; }
        catch (InvalidDataException error)
        {
            throw new InvalidOperationException($"The published {channel} feed cannot be verified ({error.Message}); refusing to replace it.");
        }
    }
    UpdateFeed.RequirePublishable(asset, previous);
    Console.WriteLine($"Verified {channel} {version}: {asset.FileName}, {asset.Size} bytes, SHA-256 {asset.SHA256}");
}

static ECDsa PublicKey(string value)
{
    var key = File.Exists(value)
        ? XDocument.Load(value).Descendants("UpdatePublicKey").Select(element => element.Value.Trim()).FirstOrDefault(text => text.Length > 0)
        : value;
    return UpdateFeed.ImportPublicKey(key)
        ?? throw new InvalidOperationException("No valid ECDSA P-256 UpdatePublicKey is configured.");
}
