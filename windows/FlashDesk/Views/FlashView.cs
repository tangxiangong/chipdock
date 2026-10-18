using FlashDesk.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using static FlashDesk.Formatting;

namespace FlashDesk;

/// Main area: a centred column reading top to bottom — firmware, settings,
/// then the single primary action.
internal sealed class FlashView : UserControl
{
    private readonly WorkbenchStore store;
    private readonly MainWindow window;

    private readonly Border summary = new() { Style = Ui.Style("CardStyle"), Padding = new Thickness(20), CornerRadius = new CornerRadius(8) };
    private readonly Border formatTile = new() { Style = Ui.Style("WellStyle"), Width = 44, Height = 44, CornerRadius = new CornerRadius(8) };
    private readonly FontIcon formatIcon = Ui.Glyph(Glyphs.Document, 20);
    private readonly TextBlock firmwareName = Ui.Text(size: 20, semibold: true);
    private readonly TextBlock firmwareDirectory = Ui.Secondary(size: 13);
    private readonly Button firmwareMenu = new() { Content = Ui.Glyph(Glyphs.More, 16), Padding = new Thickness(8) };
    private readonly StackPanel analyzing = Ui.Row(8, new ProgressRing { Width = 16, Height = 16, IsActive = true }, Ui.Secondary("正在分析固件…"));
    private readonly InfoBar firmwareError = new() { Severity = InfoBarSeverity.Warning, IsClosable = false, IsOpen = true };
    private readonly FirmwareLayoutView layout = new();
    private readonly Grid facts = new() { ColumnSpacing = 36, RowSpacing = 14 };
    private readonly StackPanel usageDetails;

    private readonly Grid dropZone = new() { MinHeight = 190 };
    private readonly Rectangle dropBorder = new() { RadiusX = 8, RadiusY = 8, StrokeThickness = 1.5, StrokeDashArray = [4, 3] };
    private readonly FontIcon dropIcon = Ui.Glyph(Glyphs.Download, 30, "SecondaryGlyphStyle");
    private readonly Button chooseFile;
    private readonly StackPanel recentSection = new() { Spacing = 6 };
    private readonly ListView recentList = new() { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true };
    private List<RecentFirmwareRecord>? shownRecentFiles;
    private FirmwareUsage? shownUsage;
    private List<MemoryRegionLayout>? shownRegions;
    private readonly StackPanel firmwareSection = new();

    private readonly Ellipse connectionDot = new() { Width = 8, Height = 8, VerticalAlignment = VerticalAlignment.Center };
    private readonly TextBlock targetName = Ui.Text(semibold: true);
    private readonly TextBlock targetState = Ui.Secondary("未连接");
    private readonly TextBlock targetDetail = Ui.Secondary(size: 12);
    private readonly Button changeTarget;
    private readonly TextBox baseAddress = new() { Width = 150, PlaceholderText = "0x08000000" };
    private readonly StackPanel baseAddressError = Ui.Row(6, Ui.Glyph(Glyphs.Warning, 12, "CriticalGlyphStyle"), Ui.Text("无效的十六进制地址", 12, "CriticalTextStyle"));
    private readonly Border baseAddressCard;
    private readonly ComboBox eraseStrategy = new() { MinWidth = 150 };
    private readonly TextBlock eraseDetail = Ui.Secondary(size: 12);
    private readonly ToggleSwitch verify = Switch(), resetAfter = Switch(), dryRun = Switch();

    private readonly TextBlock hint = Ui.Secondary(size: 13);
    private readonly ProgressRing busy = new() { Width = 20, Height = 20, IsActive = true };
    private readonly Button action = new() { Style = Ui.Style("AccentButtonStyle"), Padding = new Thickness(20, 8, 20, 8), MinWidth = 140 };
    private readonly FontIcon actionIcon = Ui.Glyph(Glyphs.Link, 14);
    private readonly TextBlock actionText = Ui.Text();

    internal FlashView(WorkbenchStore store, MainWindow window)
    {
        this.store = store;
        this.window = window;

        firmwareName.MaxLines = 1;
        firmwareName.TextTrimming = TextTrimming.CharacterEllipsis;
        formatTile.Child = formatIcon;
        formatIcon.HorizontalAlignment = HorizontalAlignment.Center;
        firmwareMenu.Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        firmwareMenu.BorderBrush = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        firmwareMenu.VerticalAlignment = VerticalAlignment.Center;
        ToolTipService.SetToolTip(firmwareMenu, "固件操作");
        Ui.Name(firmwareMenu, "固件操作");
        firmwareMenu.Flyout = FirmwareMenu();
        var heading = new Grid { ColumnSpacing = 14 };
        heading.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        heading.ColumnDefinitions.Add(new());
        heading.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var names = Ui.Column(2, firmwareName, firmwareDirectory);
        names.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(names, 1);
        Grid.SetColumn(firmwareMenu, 2);
        heading.Children.Add(formatTile);
        heading.Children.Add(names);
        heading.Children.Add(firmwareMenu);
        usageDetails = Ui.Column(18, layout, facts);
        summary.Child = Ui.Column(20, heading, analyzing, firmwareError, usageDetails);
        summary.SizeChanged += (_, args) => { if (Math.Abs(args.NewSize.Width - args.PreviousSize.Width) > 1) ArrangeFacts(); };

        chooseFile = Ui.Button(Glyphs.Folder, "选择文件…", () => _ = window.PickFirmware());
        chooseFile.HorizontalAlignment = HorizontalAlignment.Center;
        chooseFile.Margin = new Thickness(0, 8, 0, 0);
        var dropTitle = Ui.Text("将固件拖到这里", 18, semibold: true);
        var dropSubtitle = Ui.Secondary("支持 ELF、AXF、HEX 与 BIN 文件", 13);
        foreach (var element in new FrameworkElement[] { dropIcon, dropTitle, dropSubtitle }) element.HorizontalAlignment = HorizontalAlignment.Center;
        dropIcon.Margin = new Thickness(0, 0, 0, 4);
        var dropContent = Ui.Column(6, dropIcon, dropTitle, dropSubtitle, chooseFile);
        dropContent.VerticalAlignment = VerticalAlignment.Center;
        dropContent.Padding = new Thickness(24);
        dropZone.Children.Add(dropBorder);
        dropZone.Children.Add(dropContent);
        SetDropTargeted(false);

        Ui.CompactItems(recentList, new Thickness(12, 0, 12, 0), 36);
        recentList.ItemClick += (_, args) => { if ((args.ClickedItem as FrameworkElement)?.Tag is RecentFirmwareRecord file) store.OpenRecent(file); };
        recentSection.Children.Add(Ui.Text("最近使用", 14, semibold: true));
        recentSection.Children.Add(new Border { Style = Ui.Style("CardStyle"), Padding = new Thickness(0, 4, 0, 4), Child = recentList });
        recentSection.Margin = new Thickness(0, 22, 0, 0);

        changeTarget = Ui.Button(Glyphs.Edit, "更改…", () => _ = window.ShowTargetSetup());
        var targetTitle = Ui.Row(8, connectionDot, targetName, targetState);
        var targetCard = Card(Glyphs.Chip, Ui.Column(2, targetTitle, targetDetail), changeTarget);

        baseAddress.FontFamily = (FontFamily)Application.Current.Resources["MonoFontFamily"];
        baseAddress.TextChanged += (_, _) => store.BinBaseAddress = baseAddress.Text;
        Ui.Name(baseAddress, "基地址");
        var baseAddressText = Ui.Column(2, Ui.Text("基地址"), Ui.Secondary("BIN 文件写入的起始地址", 12), baseAddressError);
        baseAddressCard = Card(Glyphs.Memory, baseAddressText, baseAddress);

        foreach (var strategy in Enum.GetValues<EraseStrategy>()) eraseStrategy.Items.Add(strategy.Title());
        eraseStrategy.SelectionChanged += (_, _) => { if (eraseStrategy.SelectedIndex >= 0) store.EraseStrategy = (EraseStrategy)eraseStrategy.SelectedIndex; };
        Ui.Name(eraseStrategy, "擦除");
        var eraseCard = Card(Glyphs.Erase, Ui.Column(2, Ui.Text("擦除"), eraseDetail), eraseStrategy);

        verify.Toggled += (_, _) => store.Verify = verify.IsOn;
        resetAfter.Toggled += (_, _) => store.ResetAfter = resetAfter.IsOn;
        dryRun.Toggled += (_, _) => store.DryRun = dryRun.IsOn;
        Ui.Name(verify, "校验写入的数据");
        Ui.Name(resetAfter, "复位并运行程序");
        Ui.Name(dryRun, "仅预检查，不写入芯片");

        var settings = Ui.Column(4,
            targetCard,
            baseAddressCard,
            eraseCard,
            Card(Glyphs.Verify, Description("校验写入的数据", "写入后逐字节比对"), verify),
            Card(Glyphs.Reset, Description("复位并运行程序", "完成后复位目标芯片"), resetAfter),
            Card(Glyphs.Shield, Description("仅预检查，不写入芯片", "检查连接、地址与 Flash 映射"), dryRun));

        action.Content = Ui.Row(8, actionIcon, actionText);
        action.Click += (_, _) => _ = store.Connection is null ? store.Connect() : store.Flash();
        var actionRow = new Grid { ColumnSpacing = 16, Margin = new Thickness(0, 26, 0, 0) };
        actionRow.ColumnDefinitions.Add(new());
        actionRow.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        actionRow.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        hint.VerticalAlignment = busy.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(busy, 1);
        Grid.SetColumn(action, 2);
        actionRow.Children.Add(hint);
        actionRow.Children.Add(busy);
        actionRow.Children.Add(action);

        var settingsTitle = Ui.Text("烧录设置", 14, semibold: true);
        settingsTitle.Margin = new Thickness(0, 28, 0, 8);
        var column = Ui.Column(0, firmwareSection, settingsTitle, settings, actionRow);
        column.MaxWidth = 640;
        column.Padding = new Thickness(32, 28, 32, 28);
        Content = new ScrollViewer { Content = column };
    }

    private MenuFlyout FirmwareMenu()
    {
        var menu = new MenuFlyout();
        menu.Items.Add(Ui.MenuItem("更换固件…", Glyphs.Swap, () => _ = window.PickFirmware()));
        menu.Items.Add(Ui.MenuItem("在资源管理器中显示", Glyphs.Folder, () => { if (store.FirmwarePath is { } path) MainWindow.RevealInExplorer(path); }));
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(Ui.MenuItem("关闭固件", Glyphs.Cancel, store.ClearFirmware));
        return menu;
    }

    private static ToggleSwitch Switch() => new() { OnContent = "", OffContent = "", MinWidth = 0 };

    private static StackPanel Description(string title, string detail) => Ui.Column(2, Ui.Text(title), Ui.Secondary(detail, 12));

    /// A Windows Settings-style row: icon, title and description, then the control.
    private static Border Card(string glyph, FrameworkElement text, FrameworkElement control)
    {
        var grid = new Grid { ColumnSpacing = 16, MinHeight = 40 };
        grid.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new());
        grid.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var icon = Ui.Glyph(glyph, 18);
        icon.VerticalAlignment = text.VerticalAlignment = control.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(text, 1);
        Grid.SetColumn(control, 2);
        grid.Children.Add(icon);
        grid.Children.Add(text);
        grid.Children.Add(control);
        return new Border { Style = Ui.Style("CardStyle"), Padding = new Thickness(16, 12, 16, 12), Child = grid };
    }

    private static StackPanel Fact(string title, string value, bool mono = false)
    {
        var text = mono ? Ui.Mono(value, 14) : Ui.Text(value);
        text.IsTextSelectionEnabled = true;
        return Ui.Column(3, Ui.Caption(title), text);
    }

    internal void SetDropTargeted(bool targeted)
    {
        dropBorder.Style = Ui.Style(targeted ? "DropZoneTargetedStyle" : "DropZoneStyle");
        dropIcon.Style = Ui.Style(targeted ? "AccentGlyphStyle" : "SecondaryGlyphStyle");
    }

    internal void Update()
    {
        var path = store.FirmwarePath;
        // Swap sections only when they change, so focus inside them survives updates.
        UIElement[] sections = path is not null ? [summary] : store.RecentFiles.Count > 0 ? [dropZone, recentSection] : [dropZone];
        if (!firmwareSection.Children.SequenceEqual(sections))
        {
            firmwareSection.Children.Clear();
            foreach (var section in sections) firmwareSection.Children.Add(section);
        }
        if (path is null) UpdateRecentFiles();
        else UpdateSummary(path);
        chooseFile.IsEnabled = recentList.IsEnabled = firmwareMenu.IsEnabled = !store.IsBusy;

        connectionDot.Style = Ui.Style(store.Connection is null ? "IdleDotStyle" : "ConnectedDotStyle");
        targetName.Text = store.TargetName;
        Ui.SetVisible(targetState, store.Connection is null);
        targetDetail.Text = TargetDetail();
        changeTarget.IsEnabled = !store.IsBusy;

        Ui.SetVisible(baseAddressCard, store.IsBin);
        Ui.SetText(baseAddress, store.BinBaseAddress);
        Ui.SetVisible(baseAddressError, ParseAddress(store.BinBaseAddress) is null);
        baseAddress.IsEnabled = !store.IsBusy;

        if (eraseStrategy.SelectedIndex != (int)store.EraseStrategy) eraseStrategy.SelectedIndex = (int)store.EraseStrategy;
        eraseDetail.Text = store.EraseStrategy.Detail();
        if (verify.IsOn != store.Verify) verify.IsOn = store.Verify;
        if (resetAfter.IsOn != store.ResetAfter) resetAfter.IsOn = store.ResetAfter;
        if (dryRun.IsOn != store.DryRun) dryRun.IsOn = store.DryRun;
        eraseStrategy.IsEnabled = verify.IsEnabled = resetAfter.IsEnabled = dryRun.IsEnabled = !store.IsBusy;

        UpdateAction();
    }

    private void UpdateSummary(string path)
    {
        var tint = store.FirmwareUsage?.format.Tint();
        if (tint is { } color)
        {
            formatTile.Background = Ui.Brush(color, 0.14);
            formatIcon.Foreground = Ui.Brush(color);
        }
        else
        {
            formatTile.ClearValue(Border.BackgroundProperty);
            formatIcon.ClearValue(FontIcon.ForegroundProperty);
        }
        firmwareName.Text = System.IO.Path.GetFileName(path);
        firmwareDirectory.Text = DisplayDirectory(path);
        ToolTipService.SetToolTip(firmwareDirectory, path);

        Ui.SetVisible(analyzing, store.AnalyzingFirmware);
        Ui.SetVisible(firmwareError, !store.AnalyzingFirmware && store.FirmwareError is not null);
        firmwareError.Message = store.FirmwareError ?? "";
        var usage = store.AnalyzingFirmware || store.FirmwareError is not null ? null : store.FirmwareUsage;
        Ui.SetVisible(usageDetails, usage is not null);
        if (usage is null || (ReferenceEquals(usage, shownUsage) && ReferenceEquals(store.MemoryRegions, shownRegions))) return;
        shownUsage = usage;
        shownRegions = store.MemoryRegions;
        layout.Update(usage, store.MemoryRegions);
        var last = usage.end_address is { } end ? end > 0 ? end - 1 : 0 : (ulong?)null;
        facts.Children.Clear();
        facts.Children.Add(Fact("格式", usage.format.Title()));
        facts.Children.Add(Fact("写入大小", ByteCountLabel(usage.used_bytes)));
        facts.Children.Add(Fact("区段", usage.segments.Count.ToString()));
        facts.Children.Add(Fact("地址范围", $"{AddressLabel(usage.start_address)} – {AddressLabel(last)}", mono: true));
        ArrangeFacts();
    }

    /// Four facts on one line when they fit, otherwise two rows of two.
    private void ArrangeFacts()
    {
        var wide = summary.ActualWidth == 0 || summary.ActualWidth >= 560;
        facts.ColumnDefinitions.Clear();
        facts.RowDefinitions.Clear();
        var columns = wide ? 4 : 2;
        for (var index = 0; index < columns; index++) facts.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        for (var index = 0; index < facts.Children.Count; index++)
        {
            var element = (FrameworkElement)facts.Children[index];
            Grid.SetColumn(element, index % columns);
            Grid.SetRow(element, index / columns);
            if (index / columns >= facts.RowDefinitions.Count) facts.RowDefinitions.Add(new() { Height = GridLength.Auto });
        }
    }

    private void UpdateRecentFiles()
    {
        if (ReferenceEquals(shownRecentFiles, store.RecentFiles)) return;
        shownRecentFiles = store.RecentFiles;
        recentList.Items.Clear();
        foreach (var file in store.RecentFiles.Take(5)) recentList.Items.Add(RecentRow(file));
    }

    private Grid RecentRow(RecentFirmwareRecord file)
    {
        var row = new Grid { ColumnSpacing = 10, Tag = file, Padding = new Thickness(0, 6, 0, 6) };
        row.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new());
        row.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var icon = Ui.Glyph(Glyphs.Document, 14);
        icon.Foreground = Ui.Brush(file.format.Tint());
        var name = Ui.Text(System.IO.Path.GetFileName(file.path));
        var directory = Ui.Secondary(DisplayDirectory(file.path));
        directory.TextTrimming = TextTrimming.CharacterEllipsis;
        var time = Ui.Secondary(RelativeTime(FromUnixMilliseconds(file.used_at_ms)), 13);
        Grid.SetColumn(name, 1);
        Grid.SetColumn(directory, 2);
        Grid.SetColumn(time, 3);
        foreach (var element in new FrameworkElement[] { icon, name, directory, time }) { element.VerticalAlignment = VerticalAlignment.Center; row.Children.Add(element); }
        ToolTipService.SetToolTip(row, file.path);
        Ui.Name(row, System.IO.Path.GetFileName(file.path));
        var menu = new MenuFlyout();
        menu.Items.Add(Ui.MenuItem("打开", Glyphs.OpenFile, () => store.OpenRecent(file)));
        menu.Items.Add(Ui.MenuItem("在资源管理器中显示", Glyphs.Folder, () => MainWindow.RevealInExplorer(file.path)));
        row.ContextFlyout = menu;
        return row;
    }

    private string TargetDetail()
    {
        if (store.Connection is { } connection)
        {
            var speed = connection.speed_khz is { } khz ? $"{khz} kHz" : $"{store.Speed} kHz";
            return $"{connection.protocol.Title()} · {speed} · {connection.probe}";
        }
        var probe = store.Probes.FirstOrDefault(candidate => candidate.identifier == store.SelectedProbe)?.DisplayName() ?? "自动选择探针";
        return $"{store.WireProtocol.Title()} · {store.Speed} kHz · {probe}";
    }

    private void UpdateAction()
    {
        var needsConnection = store.Connection is null;
        Ui.SetVisible(busy, store.IsBusy);
        Ui.SetVisible(actionIcon, !store.IsBusy);
        actionText.Text = store.Operation switch
        {
            WorkbenchOperation.Connect => "正在连接…",
            WorkbenchOperation.Flash => store.DryRun ? "正在预检查…" : "正在烧录…",
            _ when needsConnection => "连接目标",
            _ => store.DryRun ? "开始预检查" : $"烧录到 {store.TargetName}",
        };
        actionIcon.Glyph = needsConnection ? Glyphs.Link : store.DryRun ? Glyphs.Shield : Glyphs.Flash;
        action.IsEnabled = needsConnection ? store.CanConnect : store.CanFlash;
        Ui.Name(action, actionText.Text);
        hint.Text = store.IsBusy ? ""
            : needsConnection ? "连接后即可烧录（Ctrl+K）"
            : store.FirmwarePath is null ? "选择固件后即可烧录"
            : "按 Ctrl+Enter 开始";
    }
}
