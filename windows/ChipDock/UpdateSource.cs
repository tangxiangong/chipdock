using System.Security.Cryptography;
using Velopack;
using Velopack.Logging;
using Velopack.Sources;

namespace ChipDock;

/// Reads the signed feed from `chipdock-updates`, then downloads the package it pins
/// from the version release and checks it against the feed before Velopack does.
internal sealed class UpdateSource(ECDsa publicKey, IFileDownloader? downloader = null) : IUpdateSource
{
    // Velopack downloader timeouts are in minutes.
    private const double FeedTimeout = 1;
    private const double PackageTimeout = 30;
    private readonly IFileDownloader downloader = downloader ?? new HttpClientFileDownloader();
    private bool usingFallback;

    public async Task<VelopackAssetFeed> GetReleaseFeed(IVelopackLogger logger, string? appId, string channel,
        Guid? stagingId = null, VelopackAsset? latestLocalRelease = null)
    {
        if (appId is not null && appId != UpdateFeed.AppId) throw new InvalidDataException($"更新源不属于 {appId}");
        if (!UpdateFeed.IsChannel(channel)) throw new InvalidDataException($"不支持的更新通道：{channel}");
        usingFallback = false;
        try { return await ReadFeed(fallback: false); }
        catch (Exception error)
        {
            System.Diagnostics.Trace.TraceWarning("COS update feed failed; trying GitHub: {0}", error.Message);
            usingFallback = true;
            return await ReadFeed(fallback: true);
        }

        async Task<VelopackAssetFeed> ReadFeed(bool fallback)
        {
            var envelope = await downloader.DownloadBytes(UpdateFeed.SignedFeedUrl(channel, fallback), null, FeedTimeout);
            var feed = UpdateFeed.Open(envelope, publicKey);
            var asset = feed.Assets[0];
            if (asset.FileName != $"{UpdateFeed.AppId}-{asset.Version}-{channel}-full.nupkg")
                throw new InvalidDataException("更新包架构与更新通道不一致");
            return feed;
        }
    }

    public async Task DownloadReleaseEntry(IVelopackLogger logger, VelopackAsset releaseEntry,
        string localFile, Action<int> progress, CancellationToken cancelToken = default)
    {
        UpdateFeed.RequireSafeFileName(releaseEntry.FileName);
        try
        {
            cancelToken.ThrowIfCancellationRequested();
            try { await Download(usingFallback); }
            catch (Exception error) when (!usingFallback && !cancelToken.IsCancellationRequested)
            {
                System.Diagnostics.Trace.TraceWarning("COS update download failed; trying GitHub: {0}", error.Message);
                File.Delete(localFile);
                progress(0);
                // Keep the exact version, filename, size and hash chosen from the verified feed.
                await Download(fallback: true);
            }
        }
        catch
        {
            // A partial or mismatched package must never be picked up as a pending update.
            try { File.Delete(localFile); }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException) { }
            throw;
        }

        async Task Download(bool fallback)
        {
            await downloader.DownloadFile(UpdateFeed.PackageUrl(releaseEntry, fallback), localFile, progress, null, PackageTimeout, cancelToken);
            await UpdateFeed.VerifyFile(localFile, releaseEntry, cancelToken);
        }
    }
}
