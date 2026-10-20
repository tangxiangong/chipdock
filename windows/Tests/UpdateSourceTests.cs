using Velopack;
using Velopack.Sources;

namespace FlashDesk.Tests;

internal static class UpdateSourceTests
{
    internal static async Task Run()
    {
        var package = new byte[] { 1, 2, 3, 4 };
        var downloader = new FakeDownloader();
        downloader.Files[UpdateFeed.SignedFeedUrl("win-x64")] = UpdateFeed.Seal(UpdateFeedTests.Feed(package: package), UpdateFeedTests.Keys.Private);
        using var key = UpdateFeed.ImportPublicKey(UpdateFeedTests.Keys.Public)!;
        var source = new UpdateSource(key, downloader);

        var asset = (await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64")).Assets.Single();
        Check.That(asset.Version.ToString() == "0.2.0", "signed feed is read from update-feed");
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64/../../evil"), "channel cannot escape the feed URL");
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, "Other", "win-x64"), "feed belongs to FlashDesk");
        downloader.Files[UpdateFeed.SignedFeedUrl("win-arm64")] = UpdateFeedTests.Feed();
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-arm64"), "unsigned feed is refused");
        await Check.ThrowsAsync<HttpRequestException>(() => new UpdateSource(key, new FakeDownloader()).GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64"),
            "missing feed surfaces as an HTTP error");

        var migrating = new FakeDownloader();
        var legacyFeed = UpdateFeed.SignedFeedUrl("win-x64", UpdateFeed.LegacyRepository);
        migrating.Files[legacyFeed] = UpdateFeed.Seal(UpdateFeedTests.Feed(package: package), UpdateFeedTests.Keys.Private);
        var migrationSource = new UpdateSource(key, migrating);
        var migrated = (await migrationSource.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64")).Assets.Single();
        Check.That(migrating.Requests.SequenceEqual(new[] { UpdateFeed.SignedFeedUrl("win-x64"), legacyFeed }),
            "new repository is preferred and the old feed works before rename");
        migrating.Files[UpdateFeed.SignedFeedUrl("win-x64")] = migrating.Files[legacyFeed];
        migrating.Requests.Clear();
        await migrationSource.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64");
        Check.That(migrating.Requests.Count == 1 && migrating.Requests[0] == UpdateFeed.SignedFeedUrl("win-x64"),
            "each check returns to the new repository after rename");
        migrating.Files[UpdateFeed.SignedFeedUrl("win-x64")] = UpdateFeedTests.Feed();
        migrating.Requests.Clear();
        await Check.ThrowsAsync<InvalidDataException>(() => migrationSource.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64"),
            "an invalid new feed cannot fall back to an older feed");
        Check.That(migrating.Requests.Count == 1, "signature validation failure never retries the legacy feed");

        var path = Path.Combine(Path.GetTempPath(), $"FlashDesk-test-{Guid.NewGuid():N}.nupkg");
        try
        {
            var legacyPackage = UpdateFeed.PackageUrl(migrated, UpdateFeed.LegacyRepository);
            migrating.Files[legacyPackage] = package;
            await migrationSource.DownloadReleaseEntry(null!, migrated, path, _ => { });
            Check.That(migrating.Requests[^1] == legacyPackage && File.ReadAllBytes(path).SequenceEqual(package),
                "package download also survives repository migration");
            downloader.Files[UpdateFeed.PackageUrl(asset)] = package;
            await source.DownloadReleaseEntry(null!, asset, path, _ => { });
            Check.That(File.ReadAllBytes(path).SequenceEqual(package) && downloader.Requests[^1] == UpdateFeed.PackageUrl(asset),
                "package downloads from the version release");

            downloader.Files[UpdateFeed.PackageUrl(asset)] = [1, 2, 3, 5];
            await Check.ThrowsAsync<InvalidDataException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }), "corrupt package is refused");
            Check.That(!File.Exists(path), "corrupt package is deleted");

            var redirected = asset with { FileName = "../../evil.nupkg" };
            await Check.ThrowsAsync<InvalidDataException>(() => source.DownloadReleaseEntry(null!, redirected, path, _ => { }), "package name cannot redirect");

            using var cancellation = new CancellationTokenSource();
            cancellation.Cancel();
            downloader.Files[UpdateFeed.PackageUrl(asset)] = package;
            await Check.ThrowsAsync<OperationCanceledException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }, cancellation.Token),
                "cancellation stops the download");
            Check.That(!File.Exists(path), "cancelled download leaves no package");
        }
        finally { File.Delete(path); }
    }

    private sealed class FakeDownloader : IFileDownloader
    {
        internal Dictionary<string, byte[]> Files { get; } = [];
        internal List<string> Requests { get; } = [];

        private byte[] Get(string url)
        {
            Requests.Add(url);
            return Files.TryGetValue(url, out var bytes)
                ? bytes
                : throw new HttpRequestException("Not Found", null, System.Net.HttpStatusCode.NotFound);
        }

        public async Task DownloadFile(string url, string targetFile, Action<int> progress, IDictionary<string, string>? headers = null,
            double timeout = 30, CancellationToken cancelToken = default)
        {
            cancelToken.ThrowIfCancellationRequested();
            await File.WriteAllBytesAsync(targetFile, Get(url), cancelToken);
            progress(100);
        }

        public Task<byte[]> DownloadBytes(string url, IDictionary<string, string>? headers = null, double timeout = 30) => Task.FromResult(Get(url));

        public Task<string> DownloadString(string url, IDictionary<string, string>? headers = null, double timeout = 30) =>
            Task.FromResult(System.Text.Encoding.UTF8.GetString(Get(url)));
    }
}
