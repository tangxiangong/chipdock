using FlashDesk.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using static FlashDesk.Formatting;

namespace FlashDesk;

/// Probe, chip and link settings. Edits apply to the window immediately, as on macOS.
internal sealed class TargetSetupDialog : ContentDialog
{
    private static readonly uint[] SpeedPresets = [100, 1000, 4000, 10000, 24000];

    private readonly WorkbenchStore store;
    private readonly TextBlock state = Ui.Text(size: 12);
    private readonly InfoBar failure = new() { Severity = InfoBarSeverity.Error, IsOpen = false };
    private readonly ComboBox probe = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    private readonly Button scan;
    private readonly TextBlock probeFooter = Ui.Text(size: 12);
    private readonly AutoSuggestBox chip = new() { Header = "芯片", PlaceholderText = "自动识别", QueryIcon = new SymbolIcon(Symbol.Find) };
    private readonly RadioButtons protocol = new() { Header = "协议", MaxColumns = 2 };
    private readonly TextBox speed = new() { Width = 110 };
    private readonly StackPanel speedError = Ui.Row(6, Ui.Glyph(Glyphs.Warning, 12, "CriticalGlyphStyle"), Ui.Text("请输入大于 0 的速度", 12, "CriticalTextStyle"));
    private readonly CheckBox connectUnderReset = new() { Content = "复位下连接" };
    private readonly StackPanel identified = new() { Spacing = 6 };
    private readonly Grid identifiedFacts = new() { ColumnSpacing = 16, RowSpacing = 4 };
    private readonly TextBlock progressMessage = Ui.Secondary("正在连接…", 13, wrap: true);
    private readonly StackPanel progress;
    private readonly StackPanel form;
    private readonly ContentControl formControls;
    private List<ProbeSummary>? shownProbes;
    private CancellationTokenSource? chipSearch;
    private bool updating;

    internal TargetSetupDialog(WorkbenchStore store)
    {
        this.store = store;
        progress = Ui.Row(8, new ProgressRing { Width = 16, Height = 16, IsActive = true }, progressMessage);
        progressMessage.MaxWidth = 340;
        Title = "目标配置";
        CloseButtonText = "取消";
        DefaultButton = ContentDialogButton.Primary;

        probe.SelectionChanged += (_, _) =>
        {
            if (updating || probe.SelectedItem is not ComboBoxItem { Tag: string identifier }) return;
            store.SelectedProbe = identifier;
        };
        Ui.Name(probe, "调试探针");
        scan = Ui.IconButton(Glyphs.Refresh, "扫描调试探针（Ctrl+R）", () => _ = store.ScanProbes());
        var probeRow = new Grid { ColumnSpacing = 4 };
        probeRow.ColumnDefinitions.Add(new());
        probeRow.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        Grid.SetColumn(scan, 1);
        probeRow.Children.Add(probe);
        probeRow.Children.Add(scan);

        chip.TextChanged += (sender, args) => { if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput) _ = SearchChips(chip.Text); };
        chip.SuggestionChosen += (_, args) => { if (args.SelectedItem is string name) chip.Text = name; };
        chip.QuerySubmitted += (_, args) => CommitChip(args.ChosenSuggestion as string, pickFirst: true);
        chip.LostFocus += (_, _) => CommitChip(null, pickFirst: false);
        chip.FontFamily = (Microsoft.UI.Xaml.Media.FontFamily)Application.Current.Resources["MonoFontFamily"];

        protocol.Items.Add("SWD");
        protocol.Items.Add("JTAG");
        protocol.SelectionChanged += (_, _) =>
        {
            if (updating || protocol.SelectedIndex < 0) return;
            store.WireProtocol = protocol.SelectedIndex == 1 ? WireProtocol.Jtag : WireProtocol.Swd;
        };

        speed.TextChanged += (_, _) => { if (!updating) store.Speed = speed.Text.Trim(); };
        Ui.Name(speed, "速度");
        var presets = new MenuFlyout();
        foreach (var value in SpeedPresets) presets.Items.Add(Ui.MenuItem($"{value} kHz", null, () => store.Speed = value.ToString()));
        var presetButton = new DropDownButton { Content = "预设", Flyout = presets };
        var speedRow = Ui.Row(8, speed, Ui.Secondary("kHz"), presetButton);
        foreach (var child in speedRow.Children) ((FrameworkElement)child).VerticalAlignment = VerticalAlignment.Center;

        connectUnderReset.Click += (_, _) => store.ConnectUnderReset = connectUnderReset.IsChecked == true;

        identified.Children.Add(Ui.Text("已识别", 14, semibold: true));
        identified.Children.Add(identifiedFacts);
        identifiedFacts.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        identifiedFacts.ColumnDefinitions.Add(new());

        form = Ui.Column(16,
            Ui.Column(4, Ui.Text("调试探针"), probeRow, probeFooter),
            chip,
            protocol,
            Ui.Column(4, Ui.Text("速度"), speedRow, speedError),
            connectUnderReset,
            identified);
        formControls = new ContentControl { Content = form, HorizontalContentAlignment = HorizontalAlignment.Stretch };
        Content = new ScrollViewer
        {
            Content = Ui.Column(16, state, failure, formControls, progress),
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            MinWidth = 400,
        };

        PrimaryButtonClick += async (_, args) =>
        {
            var deferral = args.GetDeferral();
            failure.IsOpen = false;
            await store.Connect();
            // Stay open on failure so the error shows next to the settings that caused it;
            // close when identification left candidates, so the chip picker can follow.
            args.Cancel = store.Connection is null && store.TargetCandidates.Count == 0;
            deferral.Complete();
        };
        SecondaryButtonClick += (_, args) => { args.Cancel = true; store.Disconnect(); };
        Closing += (_, args) => { if (store.IsBusy) args.Cancel = true; };
        Opened += (_, _) => { store.Changed += Update; Update(); };
        Closed += (_, _) => { store.Changed -= Update; chipSearch?.Cancel(); };
    }

    internal void ShowFailure(UserFacingError error)
    {
        failure.Title = error.Message;
        failure.Message = string.Join(Environment.NewLine, new[] { error.Detail, error.Recovery }.Where(text => text.Length > 0));
        failure.IsOpen = true;
    }

    private async Task SearchChips(string query)
    {
        chipSearch?.Cancel();
        var cancellation = chipSearch = new CancellationTokenSource();
        if (query.Trim().Length == 0) { chip.ItemsSource = null; return; }
        try
        {
            await Task.Delay(200, cancellation.Token);
            var found = await RustCore.SearchChips(query.Trim(), 50);
            if (!cancellation.IsCancellationRequested) chip.ItemsSource = found;
        }
        catch (OperationCanceledException) { }
        catch (Exception) { if (!cancellation.IsCancellationRequested) chip.ItemsSource = null; }
    }

    /// Only supported chip names are committed, so partial input never reaches the core.
    /// Enter takes the best match; leaving the box without a match restores the current chip.
    private void CommitChip(string? chosen, bool pickFirst)
    {
        var text = chip.Text.Trim();
        var suggestions = chip.ItemsSource as IEnumerable<string> ?? [];
        var name = chosen
            ?? (text.Length == 0 ? "" : null)
            ?? suggestions.FirstOrDefault(candidate => candidate.Equals(text, StringComparison.OrdinalIgnoreCase))
            ?? (pickFirst ? suggestions.FirstOrDefault() : null);
        if (name is not null) store.Chip = name;
        if (chip.Text != store.Chip) chip.Text = store.Chip;
    }

    private void Update()
    {
        updating = true;
        var connected = store.Connection is not null;
        state.Text = connected ? $"已连接 {store.TargetName}" : "未连接";
        state.Style = Ui.Style(connected ? "SuccessTextStyle" : "SecondaryTextStyle");
        PrimaryButtonText = connected ? "重新连接" : "连接";
        SecondaryButtonText = connected ? "断开" : "";
        IsPrimaryButtonEnabled = store.CanConnect;
        IsSecondaryButtonEnabled = store.CanDisconnect;

        if (!ReferenceEquals(shownProbes, store.Probes) || probe.Items.Count == 0)
        {
            shownProbes = store.Probes;
            probe.Items.Clear();
            probe.Items.Add(new ComboBoxItem { Content = "自动选择", Tag = "" });
            foreach (var summary in store.Probes) probe.Items.Add(new ComboBoxItem { Content = summary.DisplayName(), Tag = summary.identifier });
        }
        var selected = probe.Items.Cast<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == store.SelectedProbe);
        if (selected is null && store.SelectedProbe.Length > 0)
        {
            // A profile may name a probe that has not been scanned yet.
            selected = new ComboBoxItem { Content = store.SelectedProbe, Tag = store.SelectedProbe };
            probe.Items.Add(selected);
        }
        probe.SelectedItem = selected ?? probe.Items[0];
        Ui.SetVisible(probeFooter, store.HasScanned);
        probeFooter.Text = store.Probes.Count == 0
            ? "未发现调试探针。请检查 USB 数据线及驱动，并关闭其他调试工具；串口型烧录器需安装对应的串口驱动。"
            : $"已发现 {store.Probes.Count} 个调试探针；连接时会检查并修复支持的 USB 调试接口驱动。";
        probeFooter.Style = Ui.Style(store.Probes.Count == 0 ? "CriticalTextStyle" : "SecondaryTextStyle");

        if (chip.FocusState == FocusState.Unfocused) chip.Text = store.Chip;
        protocol.SelectedIndex = store.WireProtocol.IsJtag ? 1 : 0;
        Ui.SetText(speed, store.Speed);
        Ui.SetVisible(speedError, store.ValidSpeed is null);
        connectUnderReset.IsChecked = store.ConnectUnderReset;

        Ui.SetVisible(identified, store.TargetInformation is not null);
        identifiedFacts.Children.Clear();
        identifiedFacts.RowDefinitions.Clear();
        if (store.TargetInformation is { } information)
        {
            AddFact("型号", information.device_type);
            if (information.cpu is { } cpu) AddFact("内核", cpu);
            if (information.flash_size_bytes is { } bytes) AddFact("Flash", ByteCountLabel(bytes));
        }

        formControls.IsEnabled = !store.IsBusy;
        form.Opacity = store.IsBusy ? 0.6 : 1;
        Ui.SetVisible(progress, store.Operation == WorkbenchOperation.Connect);
        progressMessage.Text = store.Status;
        updating = false;
    }

    private void AddFact(string title, string value)
    {
        var row = identifiedFacts.RowDefinitions.Count;
        identifiedFacts.RowDefinitions.Add(new() { Height = GridLength.Auto });
        var label = Ui.Secondary(title, 13);
        var text = Ui.Text(value, 13);
        text.IsTextSelectionEnabled = true;
        Grid.SetRow(label, row);
        Grid.SetRow(text, row);
        Grid.SetColumn(text, 1);
        identifiedFacts.Children.Add(label);
        identifiedFacts.Children.Add(text);
    }
}
