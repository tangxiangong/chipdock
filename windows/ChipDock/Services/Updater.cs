using System.Diagnostics;
using System.Net;
using System.Net.Http;
using System.Reflection;
using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Velopack;

namespace ChipDock;

/// Velopack installs updates; this class decides when. As Sparkle does on macOS,
/// it checks every six hours, installs background downloads when the app quits,
/// and never checks or restarts while a hardware operation is running.
internal sealed class Updater
{
    private static readonly TimeSpan FirstCheckDelay = TimeSpan.FromSeconds(10);
    private static readonly TimeSpan CheckInterval = TimeSpan.FromHours(6);
    private static readonly TimeSpan RetryDelay = TimeSpan.FromMinutes(1);

    private readonly WorkbenchStore store;
    private readonly FrameworkElement owner;
    private readonly UpdateManager? manager;
    private CancellationTokenSource? backgroundDownload;
    private string? announced;
    private bool checking;
    private bool started;

    internal Updater(WorkbenchStore store, FrameworkElement owner)
    {
        this.store = store;
        this.owner = owner;
        manager = CreateManager(out var reason);
        UnavailableReason = reason;
    }

    /// Raised when <see cref="CanCheck"/> may have changed.
    internal event Action? Changed;

    internal string? UnavailableReason { get; }

    internal bool CanCheck => UnavailableReason is null && !checking && !store.IsBusy;

    internal string CurrentVersion => manager?.CurrentVersion?.ToString() ?? store.CoreVersion;

    internal bool AutomaticChecks
    {
        get => AppSettings.AutomaticUpdateChecks;
        set => AppSettings.AutomaticUpdateChecks = value;
    }

    internal bool AutomaticDownloads
    {
        get => AppSettings.AutomaticUpdateDownloads;
        set
        {
            AppSettings.AutomaticUpdateDownloads = value;
            if (value) return;
            // Turning downloads off withdraws consent for any package prepared earlier.
            backgroundDownload?.Cancel();
            AppSettings.PendingUpdate = "";
        }
    }

    private static string Architecture =>
        RuntimeInformation.ProcessArchitecture == System.Runtime.InteropServices.Architecture.Arm64 ? "arm64" : "x64";

    /// Set by the release build from the UpdatePublicKey MSBuild property.
    private static string? PublicKey => typeof(Updater).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
        .FirstOrDefault(attribute => attribute.Key == "UpdatePublicKey")?.Value;

    private static UpdateManager? CreateManager(out string? unavailable)
    {
        if (UpdateFeed.ImportPublicKey(PublicKey) is not { } key)
        {
            unavailable = "此构建未配置更新验证公钥";
            return null;
        }
        try
        {
            var manager = new UpdateManager(new UpdateSource(key), new UpdateOptions
            {
                ExplicitChannel = UpdateFeed.Channel(Architecture),
                AllowVersionDowngrade = false,
                MaximumDeltasBeforeFallback = -1,
            });
            unavailable = manager.IsInstalled ? null : "开发构建不支持自动更新，请使用安装包安装 ChipDock";
            return manager.IsInstalled ? manager : null;
        }
        catch (Exception error)
        {
            Trace.TraceError("Update manager unavailable: {0}", error);
            unavailable = "自动更新不可用";
            return null;
        }
    }

    /// Runs before WinUI starts, in the primary instance only: installs a background
    /// download that did not get installed when the app last quit.
    internal static void ApplyPending()
    {
        try
        {
            var requested = AppSettings.PendingUpdate;
            if (requested.Length == 0) return;
            // A stale marker must never authorize a different cached package later.
            AppSettings.PendingUpdate = "";
            if (!AppSettings.AutomaticUpdateDownloads || CreateManager(out _) is not { } manager) return;
            // Relaunch with this launch's arguments, so a firmware opened from Explorer still opens.
            if (manager.UpdatePendingRestart is { } pending && pending.Version.ToString() == requested)
                manager.ApplyUpdatesAndRestart(pending, Environment.GetCommandLineArgs()[1..]);
        }
        catch (Exception error) { Trace.TraceError("Pending update failed: {0}", error); }
    }

    /// Hands a background download to Velopack, which installs it once this process exits.
    /// The window only closes while no hardware operation is running.
    internal void ApplyOnExit()
    {
        if (manager is null) return;
        try
        {
            var requested = AppSettings.PendingUpdate;
            if (requested.Length == 0) return;
            AppSettings.PendingUpdate = "";
            if (AutomaticDownloads && manager.UpdatePendingRestart is { } pending && pending.Version.ToString() == requested)
                manager.WaitExitThenApplyUpdates(pending, silent: true, restart: false);
        }
        catch (Exception error) { Trace.TraceError("Update on exit failed: {0}", error); }
    }

    internal async Task Start()
    {
        if (manager is null || started) return;
        started = true;
        await Task.Delay(FirstCheckDelay);
        while (true)
        {
            // The schedule survives relaunches, so frequent restarts do not mean frequent checks.
            // A check recorded in the future, e.g. before the clock was corrected, counts as overdue.
            var now = DateTimeOffset.Now;
            var last = AppSettings.LastUpdateCheck is { } recorded && recorded <= now ? recorded : DateTimeOffset.MinValue;
            var due = last + CheckInterval - now;
            if (due > TimeSpan.Zero) await Task.Delay(due);
            else if (!AutomaticChecks || !CanCheck) await Task.Delay(RetryDelay);
            else
            {
                try
                {
                    AppSettings.LastUpdateCheck = now;
                    await Check(manual: false);
                }
                catch (Exception error) { Trace.TraceWarning("Update check failed: {0}", error.Message); }
            }
        }
    }

    internal async Task Check(bool manual = true)
    {
        if (manager is null || checking) return;
        if (store.IsBusy)
        {
            if (manual) await ShowMessage("无法检查更新", "请等待当前硬件操作完成后检查更新。");
            return;
        }
        SetChecking(true);
        try
        {
            UpdateInfo? update;
            try { update = await manager.CheckForUpdatesAsync(); }
            catch (Exception error) when (manual)
            {
                await ShowMessage("无法检查更新", Describe(error));
                return;
            }
            if (manual) AppSettings.LastUpdateCheck = DateTimeOffset.Now;
            if (update is null)
            {
                if (manual) await ShowMessage("当前已是最新版本", $"ChipDock {CurrentVersion} 是目前的最新版本。");
                return;
            }
            var version = update.TargetFullRelease.Version.ToString();
            if (!manual && AutomaticDownloads)
            {
                await DownloadInBackground(update, version);
                return;
            }
            if (!manual && announced == version) return;
            announced = version;
            await Offer(update);
        }
        finally { SetChecking(false); }
    }

    private void SetChecking(bool value)
    {
        checking = value;
        Changed?.Invoke();
    }

    private async Task DownloadInBackground(UpdateInfo update, string version)
    {
        if (AppSettings.PendingUpdate == version) return;
        using var cancellation = new CancellationTokenSource();
        backgroundDownload = cancellation;
        try
        {
            await manager!.DownloadUpdatesAsync(update, cancelToken: cancellation.Token);
            if (cancellation.IsCancellationRequested || !AutomaticDownloads) return;
            AppSettings.PendingUpdate = version;
            store.AppendLog("更新", $"ChipDock {version} 已下载，将在退出时安装");
        }
        catch (OperationCanceledException) { }
        finally { backgroundDownload = null; }
    }

    private async Task Offer(UpdateInfo update)
    {
        var target = update.TargetFullRelease;
        var version = target.Version.ToString();
        var progress = new ProgressBar { Maximum = 100, Visibility = Visibility.Collapsed };
        var status = Ui.Secondary(size: 13, wrap: true);
        var failure = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = false, IsOpen = false };
        var releaseLink = new HyperlinkButton
        {
            Content = "在 GitHub 查看发布说明",
            NavigateUri = new Uri($"https://github.com/{UpdateFeed.Repository}/releases/tag/v{version}"),
            Padding = new Thickness(0),
        };
        var content = Ui.Column(12,
            Ui.Text($"ChipDock {version} 现已推出，当前版本为 {CurrentVersion}。", wrap: true),
            Notes(target.NotesMarkdown),
            releaseLink,
            progress,
            status,
            failure);
        content.MinWidth = 420;
        var dialog = new ContentDialog
        {
            Title = "发现新版本",
            Content = content,
            PrimaryButtonText = "下载并安装",
            CloseButtonText = "稍后",
            DefaultButton = ContentDialogButton.Primary,
        };
        using var cancellation = new CancellationTokenSource();
        Task? installation = null;
        var installing = false;

        async Task Install()
        {
            installing = true;
            dialog.IsPrimaryButtonEnabled = false;
            failure.IsOpen = false;
            progress.IsIndeterminate = false;
            progress.Value = 0;
            Ui.SetVisible(progress, true);
            status.Text = "正在下载…";
            try
            {
                await manager!.DownloadUpdatesAsync(update,
                    value => owner.DispatcherQueue.TryEnqueue(() => progress.Value = value), cancellation.Token);
                // Like Sparkle on macOS, the relaunch waits for a running hardware operation.
                if (store.IsBusy)
                {
                    status.Text = "将在当前硬件操作完成后重新启动…";
                    progress.IsIndeterminate = true;
                }
                while (store.IsBusy) await Task.Delay(TimeSpan.FromSeconds(1), cancellation.Token);
                status.Text = "正在重新启动…";
                AppSettings.PendingUpdate = "";
                manager.ApplyUpdatesAndRestart(target);
            }
            catch (OperationCanceledException) { }
            catch (Exception error)
            {
                Ui.SetVisible(progress, false);
                status.Text = "";
                failure.Title = "更新失败";
                failure.Message = Describe(error);
                failure.IsOpen = true;
                dialog.IsPrimaryButtonEnabled = true;
            }
            finally { installing = false; }
        }

        dialog.PrimaryButtonClick += (sender, args) =>
        {
            args.Cancel = true;
            if (!installing) installation = Install();
        };
        dialog.CloseButtonClick += (sender, args) => cancellation.Cancel();
        try { await dialog.Show(owner); }
        finally
        {
            cancellation.Cancel();
            if (installation is not null) await installation;
        }
    }

    private async Task ShowMessage(string title, string message)
    {
        var text = Ui.Text(message, wrap: true);
        text.IsTextSelectionEnabled = true;
        await new ContentDialog { Title = title, Content = text, CloseButtonText = "好" }.Show(owner);
    }

    private static string Describe(Exception error) => error switch
    {
        // The first release has no feed yet, as on macOS.
        HttpRequestException { StatusCode: HttpStatusCode.NotFound } => "暂时无法获取更新信息，请稍后重试。",
        HttpRequestException or TaskCanceledException => "无法连接到更新服务器，请检查网络后重试。",
        _ => error.Message,
    };

    /// Release notes are the GitHub release body. Headings and list items get light
    /// structure; everything else stays plain text.
    private static FrameworkElement Notes(string? markdown)
    {
        var lines = Ui.Column(4);
        foreach (var raw in (markdown ?? "").Replace("\r\n", "\n").Split('\n'))
        {
            var line = raw.Trim().Replace("**", "").Replace("`", "");
            if (line.Length == 0) continue;
            if (line.StartsWith('#'))
            {
                var heading = Ui.Text(line.TrimStart('#').Trim(), 14, semibold: true, wrap: true);
                heading.Margin = new Thickness(0, lines.Children.Count == 0 ? 0 : 8, 0, 0);
                lines.Children.Add(heading);
            }
            else if (line.StartsWith("- ") || line.StartsWith("* "))
                lines.Children.Add(Ui.Text("•  " + line[2..], 13, wrap: true));
            else lines.Children.Add(Ui.Text(line, 13, wrap: true));
        }
        if (lines.Children.Count == 0) lines.Children.Add(Ui.Secondary("此版本未提供发布说明。", 13));
        return new Border
        {
            Style = Ui.Style("WellStyle"),
            CornerRadius = new CornerRadius(6),
            Child = new ScrollViewer { Content = lines, MaxHeight = 260, Padding = new Thickness(12, 8, 12, 8) },
        };
    }
}
