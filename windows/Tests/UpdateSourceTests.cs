using Velopack;
using Velopack.Sources;

namespace ChipDock.Tests;

internal static class UpdateSourceTests
{
    internal static async Task Run()
    {
        var package = new byte[] { 1, 2, 3, 4 };
        var signed = UpdateFeed.Seal(UpdateFeedTests.Feed(package: package), UpdateFeedTests.Keys.Private);
        var downloader = new FakeDownloader();
        var primary = UpdateFeed.SignedFeedUrl("win-x64");
        var fallback = UpdateFeed.SignedFeedUrl("win-x64", fallback: true);
        downloader.Files[primary] = signed;
        downloader.Files[fallback] = signed;
        using var key = UpdateFeed.ImportPublicKey(UpdateFeedTests.Keys.Public)!;
        var source = new UpdateSource(key, downloader);

        var asset = (await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64")).Assets.Single();
        Check.That(downloader.Requests.SequenceEqual([primary]), "valid COS feed does not query GitHub");
        Check.That(asset.Version.ToString() == "0.2.0", "signed feed preserves the selected version");
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64/../../evil"), "channel cannot escape the feed URL");
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, "Other", "win-x64"), "feed belongs to ChipDock");

        downloader.Files[primary] = UpdateFeedTests.Feed();
        downloader.Requests.Clear();
        await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64");
        Check.That(downloader.Requests.SequenceEqual([primary, fallback]), "invalid COS feed falls back to a verified signed GitHub feed");

        downloader.Files[primary] = UpdateFeed.Seal(UpdateFeedTests.Feed(fileName: "ChipDock-0.2.0-win-arm64-full.nupkg"), UpdateFeedTests.Keys.Private);
        downloader.Requests.Clear();
        await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64");
        Check.That(downloader.Requests.SequenceEqual([primary, fallback]), "wrong-architecture primary is replaced only by a valid signed fallback");

        downloader.Files.Remove(primary);
        downloader.Requests.Clear();
        asset = (await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64")).Assets.Single();
        Check.That(downloader.Requests.SequenceEqual([primary, fallback]), "unavailable COS feed retries GitHub once");
        downloader.Files[fallback] = UpdateFeedTests.Feed();
        await Check.ThrowsAsync<InvalidDataException>(() => source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64"), "GitHub feed must also be signed");

        var unavailable = new FakeDownloader();
        await Check.ThrowsAsync<HttpRequestException>(() => new UpdateSource(key, unavailable).GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64"),
            "both feed errors remain visible");
        Check.That(unavailable.Requests.SequenceEqual([primary, fallback]), "each feed is requested at most once");

        var primaryPackage = UpdateFeed.PackageUrl(asset);
        var fallbackPackage = UpdateFeed.PackageUrl(asset, fallback: true);
        var path = Path.Combine(Path.GetTempPath(), $"ChipDock-test-{Guid.NewGuid():N}.nupkg");
        try
        {
            downloader.Files[fallback] = signed;
            await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64");
            downloader.Files[fallbackPackage] = package;
            downloader.Requests.Clear();
            await source.DownloadReleaseEntry(null!, asset, path, _ => { });
            Check.That(downloader.Requests.SequenceEqual([fallbackPackage]), "GitHub feed uses its versioned GitHub package directly");

            downloader.Files[primary] = signed;
            downloader.Requests.Clear();
            await source.GetReleaseFeed(null!, UpdateFeed.AppId, "win-x64");
            Check.That(downloader.Requests.SequenceEqual([primary]), "every new check returns to COS");
            downloader.Files[primaryPackage] = package;
            downloader.Requests.Clear();
            await source.DownloadReleaseEntry(null!, asset, path, _ => { });
            Check.That(File.ReadAllBytes(path).SequenceEqual(package) && downloader.Requests.SequenceEqual([primaryPackage]),
                "healthy COS package does not query GitHub");

            downloader.Files.Remove(primaryPackage);
            downloader.Requests.Clear();
            var progress = new List<int>();
            await source.DownloadReleaseEntry(null!, asset, path, progress.Add);
            Check.That(downloader.Requests.SequenceEqual([primaryPackage, fallbackPackage]), "COS package transport failure retries the same GitHub package");
            Check.That(File.ReadAllBytes(path).SequenceEqual(package) && progress.Contains(0), "partial primary download is replaced and progress resets");

            downloader.Files[primaryPackage] = [1, 2, 3, 5];
            downloader.Requests.Clear();
            await source.DownloadReleaseEntry(null!, asset, path, _ => { });
            Check.That(File.ReadAllBytes(path).SequenceEqual(package) && downloader.Requests.SequenceEqual([primaryPackage, fallbackPackage]),
                "corrupt COS bytes are replaced by the identical hash-pinned GitHub package");

            downloader.Files.Remove(primaryPackage);
            downloader.Files[fallbackPackage] = [1, 2, 3, 5];
            await Check.ThrowsAsync<InvalidDataException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }), "corrupt GitHub package is refused");
            Check.That(!File.Exists(path), "corrupt fallback leaves no pending package");

            downloader.Files.Remove(fallbackPackage);
            await Check.ThrowsAsync<HttpRequestException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }), "both package transport failures remain visible");
            Check.That(!File.Exists(path), "failed download leaves no pending package");

            var redirected = asset with { FileName = "../../evil.nupkg" };
            downloader.Requests.Clear();
            await Check.ThrowsAsync<InvalidDataException>(() => source.DownloadReleaseEntry(null!, redirected, path, _ => { }), "package name cannot redirect");
            Check.That(downloader.Requests.Count == 0, "invalid package name fails before network access");

            using var cancellation = new CancellationTokenSource();
            cancellation.Cancel();
            await Check.ThrowsAsync<OperationCanceledException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }, cancellation.Token),
                "cancellation stops before either download");
            Check.That(downloader.Requests.Count == 0 && !File.Exists(path), "cancelled operation does not query fallback");

            using var midflight = new CancellationTokenSource();
            downloader.OnDownload = midflight.Cancel;
            await Check.ThrowsAsync<OperationCanceledException>(() => source.DownloadReleaseEntry(null!, asset, path, _ => { }, midflight.Token),
                "mid-download cancellation stops fallback");
            Check.That(downloader.Requests.SequenceEqual([primaryPackage]) && !File.Exists(path), "cancelled partial download is deleted");
        }
        finally { File.Delete(path); }
    }

    private sealed class FakeDownloader : IFileDownloader
    {
        internal Dictionary<string, byte[]> Files { get; } = [];
        internal List<string> Requests { get; } = [];
        internal Action? OnDownload { get; set; }

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
            // Leave a partial file even when the simulated transport fails or is cancelled.
            await File.WriteAllBytesAsync(targetFile, [0], cancelToken);
            Requests.Add(url);
            OnDownload?.Invoke();
            cancelToken.ThrowIfCancellationRequested();
            Requests.RemoveAt(Requests.Count - 1);
            await File.WriteAllBytesAsync(targetFile, Get(url), cancelToken);
            progress(100);
        }

        public Task<byte[]> DownloadBytes(string url, IDictionary<string, string>? headers = null, double timeout = 30) => Task.FromResult(Get(url));

        public Task<string> DownloadString(string url, IDictionary<string, string>? headers = null, double timeout = 30) =>
            Task.FromResult(System.Text.Encoding.UTF8.GetString(Get(url)));
    }
}
