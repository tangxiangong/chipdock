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

    public async Task<VelopackAssetFeed> GetReleaseFeed(IVelopackLogger logger, string? appId, string channel,
        Guid? stagingId = null, VelopackAsset? latestLocalRelease = null)
    {
        if (appId is not null && appId != UpdateFeed.AppId) throw new InvalidDataException($"更新源不属于 {appId}");
        if (!UpdateFeed.IsChannel(channel)) throw new InvalidDataException($"不支持的更新通道：{channel}");
        var envelope = await downloader.DownloadBytes(UpdateFeed.SignedFeedUrl(channel), null, FeedTimeout);
        return UpdateFeed.Open(envelope, publicKey);
    }

    public async Task DownloadReleaseEntry(IVelopackLogger logger, VelopackAsset releaseEntry,
        string localFile, Action<int> progress, CancellationToken cancelToken = default)
    {
        UpdateFeed.RequireSafeFileName(releaseEntry.FileName);
        try
        {
            await downloader.DownloadFile(UpdateFeed.PackageUrl(releaseEntry), localFile, progress, null, PackageTimeout, cancelToken);
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
