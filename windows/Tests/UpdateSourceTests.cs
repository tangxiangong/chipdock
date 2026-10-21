using Velopack;
using Velopack.Sources;

namespace ChipDock.Tests;

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
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, "Other", "win-x64"), "feed belongs to ChipDock");
        downloader.Files[UpdateFeed.SignedFeedUrl("win-arm64")] = UpdateFeedTests.Feed();
        downloader.Requests.Clear();
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-arm64"), "unsigned feed is refused");
        Check.That(downloader.Requests.Count == 1, "signature validation failure does not retry another feed");
        var unavailable = new FakeDownloader();
        await Check.ThrowsAsync<HttpRequestException>(() => new UpdateSource(key, unavailable).GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64"),
            "missing feed surfaces as an HTTP error");
        Check.That(unavailable.Requests.SequenceEqual(new[]
            { "https://github.com/tangxiangong/chipdock/releases/download/update-feed/releases.win-x64.signed.json" }),
            "unavailable feed only requests the renamed repository");

        var path = Path.Combine(Path.GetTempPath(), $"ChipDock-test-{Guid.NewGuid():N}.nupkg");
        try
        {
            downloader.Requests.Clear();
            await Check.ThrowsAsync<HttpRequestException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }),
                "missing package surfaces as an HTTP error");
            Check.That(downloader.Requests.SequenceEqual(new[]
                { "https://github.com/tangxiangong/chipdock/releases/download/v0.2.0/FlashDesk-0.2.0-win-x64-full.nupkg" }),
                "unavailable package only requests the renamed repository");
            Check.That(!File.Exists(path), "failed download leaves no package");
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
