using System.Security.Cryptography;
using Velopack;
using Velopack.Logging;
using Velopack.Sources;

namespace ChipDock;

/// Reads the signed feed from `update-feed`, then downloads the package it pins
/// from the version release and checks it against the feed before Velopack does.
internal sealed class UpdateSource(ECDsa publicKey, IFileDownloader? downloader = null) : IUpdateSource
{
    // Velopack downloader timeouts are in minutes.
    private const double FeedTimeout = 1;
    private const double PackageTimeout = 30;
    private readonly IFileDownloader downloader = downloader ?? new HttpClientFileDownloader();
    private string repository = UpdateFeed.Repository;

    public async Task<VelopackAssetFeed> GetReleaseFeed(IVelopackLogger logger, string? appId, string channel,
        Guid? stagingId = null, VelopackAsset? latestLocalRelease = null)
    {
        if (appId is not null && appId != UpdateFeed.AppId) throw new InvalidDataException($"更新源不属于 {appId}");
        if (!UpdateFeed.IsChannel(channel)) throw new InvalidDataException($"不支持的更新通道：{channel}");
        foreach (var candidate in UpdateFeed.Repositories)
        {
            byte[] envelope;
            try { envelope = await downloader.DownloadBytes(UpdateFeed.SignedFeedUrl(channel, candidate), null, FeedTimeout); }
            catch (Exception error) when (candidate != UpdateFeed.LegacyRepository && error is HttpRequestException or TimeoutException or TaskCanceledException)
            { continue; }
            // Invalid signatures never trigger fallback to another feed.
            var feed = UpdateFeed.Open(envelope, publicKey);
            repository = candidate;
            return feed;
        }
        throw new InvalidOperationException("没有可用的更新源");
    }

    public async Task DownloadReleaseEntry(IVelopackLogger logger, VelopackAsset releaseEntry,
        string localFile, Action<int> progress, CancellationToken cancelToken = default)
    {
        UpdateFeed.RequireSafeFileName(releaseEntry.FileName);
        try
        {
            var repositories = new[] { repository }.Concat(UpdateFeed.Repositories.Where(candidate => candidate != repository)).ToArray();
            for (var index = 0; index < repositories.Length; index++)
            {
                try
                {
                    await downloader.DownloadFile(UpdateFeed.PackageUrl(releaseEntry, repositories[index]), localFile, progress, null, PackageTimeout, cancelToken);
                    break;
                }
                catch (HttpRequestException) when (index + 1 < repositories.Length)
                {
                    if (File.Exists(localFile)) File.Delete(localFile);
                }
            }
            await UpdateFeed.VerifyFile(localFile, releaseEntry, cancelToken);
        }
        catch
        {
            // A partial or mismatched package must never be picked up as a pending update.
            try { File.Delete(localFile); }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException) { }
            throw;
        }
    }
}
