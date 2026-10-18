using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;

namespace FlashDesk;

internal sealed class SettingsDialog : ContentDialog
{
    internal SettingsDialog(WorkbenchStore store, MainWindow window)
    {
        Title = "设置";
        CloseButtonText = "关闭";
        DefaultButton = ContentDialogButton.Close;

        var appearance = new ComboBox { Header = "主题", MinWidth = 200 };
        foreach (var value in Enum.GetValues<Appearance>()) appearance.Items.Add(value.Title());
        appearance.SelectedIndex = (int)AppSettings.Appearance;
        appearance.SelectionChanged += (_, _) =>
        {
            if (appearance.SelectedIndex < 0) return;
            AppSettings.Appearance = (Appearance)appearance.SelectedIndex;
            window.ApplyTheme();
            RequestedTheme = AppSettings.Appearance.Theme();
        };

        var verify = new CheckBox { Content = "写入后校验", IsChecked = AppSettings.DefaultVerify };
        verify.Click += (_, _) => AppSettings.DefaultVerify = verify.IsChecked == true;
        var reset = new CheckBox { Content = "完成后复位", IsChecked = AppSettings.DefaultReset };
        reset.Click += (_, _) => AppSettings.DefaultReset = reset.IsChecked == true;

        var updater = window.Updater;
        var available = updater.UnavailableReason is null;
        var checkNow = new Button { Content = "检查更新…", IsEnabled = updater.CanCheck };
        // Close first: WinUI shows one dialog at a time, and the update dialog follows.
        checkNow.Click += (_, _) => { Hide(); _ = updater.Check(); };
        var automaticChecks = new CheckBox { Content = "自动检查更新", IsChecked = updater.AutomaticChecks, IsEnabled = available };
        var automaticDownloads = new CheckBox
        {
            Content = "自动下载并安装更新",
            IsChecked = updater.AutomaticDownloads,
            IsEnabled = available && updater.AutomaticChecks,
        };
        automaticChecks.Click += (_, _) =>
        {
            updater.AutomaticChecks = automaticChecks.IsChecked == true;
            automaticDownloads.IsEnabled = updater.AutomaticChecks;
        };
        automaticDownloads.Click += (_, _) => updater.AutomaticDownloads = automaticDownloads.IsChecked == true;
        var updateNote = Ui.Caption(updater.UnavailableReason
            ?? (store.IsBusy ? "请等待当前硬件操作完成后检查更新。" : "自动下载的更新会在退出 FlashDesk 时安装。"));
        updateNote.TextWrapping = TextWrapping.Wrap;

        var icon = new Image
        {
            Source = new BitmapImage(new Uri("ms-appx:///Assets/FlashDesk.png")),
            Width = 56,
            Height = 56,
        };
        var links = Ui.Row(4,
            Link("源代码", "https://github.com/tangxiangong/flashdesk"),
            Link("发布记录", "https://github.com/tangxiangong/flashdesk/releases"),
            Link("probe-rs 文档", "https://probe.rs/docs/"));
        var identity = Ui.Column(2,
            Ui.Text("FlashDesk", 18, semibold: true),
            Ui.Secondary($"版本 {store.CoreVersion}", 13),
            Ui.Caption("WinUI 3 · Rust · Interoptopus"));
        identity.VerticalAlignment = VerticalAlignment.Center;
        var about = Ui.Column(8, Ui.Row(16, icon, identity), links, Ui.Caption("MIT OR Apache-2.0"));

        Content = new ScrollViewer
        {
            Content = Ui.Column(24,
                Section("外观", appearance),
                Section("新会话的烧录默认值", verify, reset),
                Section("更新", checkNow, automaticChecks, automaticDownloads, updateNote),
                Section("关于", about)),
            MinWidth = 400,
        };
    }

    private static StackPanel Section(string title, params UIElement[] children)
    {
        var section = Ui.Column(8, Ui.Text(title, 14, semibold: true));
        foreach (var child in children) section.Children.Add(child);
        return section;
    }

    private static HyperlinkButton Link(string text, string url) => new() { Content = text, NavigateUri = new Uri(url) };
}
