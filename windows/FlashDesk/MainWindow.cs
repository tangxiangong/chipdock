using FlashDesk.Core;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using System.Diagnostics;
using System.Runtime.InteropServices;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.System;

namespace FlashDesk;

internal sealed class MainWindow : Window
{
    private const double InspectorWidth = 400;

    internal WorkbenchStore Store { get; } = new();
    internal Updater Updater { get; }

    private readonly Grid root = new();
    private readonly TextBlock titleText = Ui.Text("FlashDesk", 12);
    private readonly ActivityView activity;
    private readonly FlashView flash;
    private readonly LogsView logs;
    private readonly MemoryView memory;
    private readonly Grid inspector = new() { Visibility = Visibility.Collapsed };
    private readonly Border inspectorDivider = Ui.Divider(vertical: true);
    private readonly ColumnDefinition inspectorColumn = new() { Width = new GridLength(0) };
    private readonly SelectorBar selector = new();
    private readonly SelectorBarItem logsTab = new() { Text = "日志", Icon = new FontIcon { Glyph = Glyphs.Logs } };
    private readonly SelectorBarItem memoryTab = new() { Text = "内存", Icon = new FontIcon { Glyph = Glyphs.Memory } };
    private readonly Border dropOverlay = new() { Style = Ui.Style("DropOverlayStyle"), Visibility = Visibility.Collapsed, IsHitTestVisible = false };
    private readonly ToggleButton logsButton = new() { Padding = new Thickness(10, 6, 10, 6) };
    private readonly ToggleButton memoryButton = new() { Padding = new Thickness(10, 6, 10, 6) };
    private readonly InfoBadge errorBadge = new() { HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, -6, -8, 0) };
    private readonly DropDownButton profilesButton = new() { Content = Ui.Row(8, Ui.Glyph(Glyphs.Bookmark, 14), Ui.Text("配置")) };

    private readonly MenuFlyoutItem openItem, targetSetupItem, scanItem, connectItem, disconnectItem, chipItem, saveProfileItem;
    private readonly MenuFlyoutItem flashItem, readItem, eraseItem, checkUpdatesItem;
    private readonly ToggleMenuFlyoutItem logsItem, memoryItem;
    private readonly MenuFlyoutSubItem recentMenu = new() { Text = "打开最近使用的" };
    private readonly MenuFlyoutSubItem loadProfileMenu = new() { Text = "载入配置" };
    private List<RecentFirmwareRecord>? shownRecentFiles;
    private List<ProfileRecord>? shownProfiles;

    private InspectorPane? openPane;
    private TargetSetupDialog? targetDialog;
    private bool dropTargeted;

    internal MainWindow()
    {
        Title = "FlashDesk";
        ExtendsContentIntoTitleBar = true;
        SystemBackdrop = MicaController.IsSupported() ? new MicaBackdrop() : new DesktopAcrylicBackdrop();
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "FlashDesk.ico"));
        var scale = Scale;
        AppWindow.Resize(new Windows.Graphics.SizeInt32((int)(1160 * scale), (int)(780 * scale)));
        SetMinimumSize();

        Updater = new Updater(Store, root);
        activity = new ActivityView(Store);
        flash = new FlashView(Store, this);
        logs = new LogsView(Store, this);
        memory = new MemoryView(Store, this);

        root.RowDefinitions.Add(new() { Height = new GridLength(48) });
        root.RowDefinitions.Add(new() { Height = GridLength.Auto });
        root.RowDefinitions.Add(new());

        var titleBar = BuildTitleBar();
        root.Children.Add(titleBar);
        SetTitleBar(titleBar);

        var menu = new MenuBar();
        openItem = Ui.MenuItem("打开固件…", Glyphs.OpenFile, () => _ = PickFirmware(), "Ctrl+O");
        targetSetupItem = Ui.MenuItem("目标配置…", Glyphs.Edit, () => _ = ShowTargetSetup());
        scanItem = Ui.MenuItem("扫描探针", Glyphs.Refresh, () => _ = Store.ScanProbes(), "Ctrl+R");
        connectItem = Ui.MenuItem("连接目标", Glyphs.Link, () => _ = Store.Connect(), "Ctrl+K");
        disconnectItem = Ui.MenuItem("断开目标", Glyphs.Cancel, Store.Disconnect, "Ctrl+Shift+K");
        chipItem = Ui.MenuItem("选择芯片…", Glyphs.Chip, () => _ = ShowChipPicker());
        saveProfileItem = Ui.MenuItem("保存配置…", Glyphs.Save, () => _ = ShowSaveProfile(), "Ctrl+Shift+S");
        flashItem = Ui.MenuItem("烧录固件", Glyphs.Flash, () => _ = Store.Flash(), "Ctrl+Enter");
        readItem = Ui.MenuItem("读取内存", Glyphs.Download, ReadMemory, "Ctrl+Shift+R");
        eraseItem = Ui.MenuItem("擦除整个芯片…", Glyphs.Delete, () => _ = ConfirmErase());
        logsItem = new ToggleMenuFlyoutItem { Text = "日志", KeyboardAcceleratorTextOverride = "Ctrl+J" };
        logsItem.Click += (_, _) => TogglePane(InspectorPane.Logs);
        memoryItem = new ToggleMenuFlyoutItem { Text = "内存", KeyboardAcceleratorTextOverride = "Ctrl+Alt+M" };
        memoryItem.Click += (_, _) => TogglePane(InspectorPane.Memory);
        checkUpdatesItem = Ui.MenuItem("检查更新…", Glyphs.Refresh, () => _ = Updater.Check());
        menu.Items.Add(MenuGroup("文件", openItem, recentMenu, new MenuFlyoutSeparator(),
            Ui.MenuItem("退出", null, Quit, "Alt+F4")));
        menu.Items.Add(MenuGroup("目标", targetSetupItem, new MenuFlyoutSeparator(), scanItem, connectItem, disconnectItem,
            new MenuFlyoutSeparator(), chipItem, saveProfileItem, loadProfileMenu));
        menu.Items.Add(MenuGroup("操作", flashItem, readItem, new MenuFlyoutSeparator(), eraseItem));
        menu.Items.Add(MenuGroup("查看", logsItem, memoryItem, new MenuFlyoutSeparator(),
            Ui.MenuItem("设置…", Glyphs.Settings, () => _ = ShowSettings(), "Ctrl+,")));
        menu.Items.Add(MenuGroup("帮助",
            Ui.MenuItem("FlashDesk 源代码", null, () => OpenUri("https://github.com/tangxiangong/flashdesk")),
            Ui.MenuItem("probe-rs 文档", null, () => OpenUri("https://probe.rs/docs/")),
            new MenuFlyoutSeparator(),
            checkUpdatesItem,
            Ui.MenuItem("关于 FlashDesk", Glyphs.Info, () => _ = ShowSettings())));

        var toolbar = new Grid { Padding = new Thickness(4, 0, 12, 8), ColumnSpacing = 12 };
        toolbar.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        toolbar.ColumnDefinitions.Add(new());
        toolbar.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        Grid.SetColumn(activity, 1);
        var tools = Ui.Row(4, profilesButton, ToolSeparator(), PaneButton(logsButton, Glyphs.Logs, "日志（Ctrl+J）", InspectorPane.Logs, errorBadge),
            PaneButton(memoryButton, Glyphs.Memory, "内存（Ctrl+Alt+M）", InspectorPane.Memory, null),
            Ui.IconButton(Glyphs.Settings, "设置", () => _ = ShowSettings()));
        tools.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(tools, 2);
        toolbar.Children.Add(menu);
        toolbar.Children.Add(activity);
        toolbar.Children.Add(tools);
        Grid.SetRow(toolbar, 1);
        root.Children.Add(toolbar);

        var profilesMenu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        profilesMenu.Opening += (_, _) => BuildProfilesMenu(profilesMenu);
        profilesButton.Flyout = profilesMenu;
        ToolTipService.SetToolTip(profilesButton, "存储或载入配置");

        var content = new Grid();
        content.ColumnDefinitions.Add(new() { MinWidth = 420 });
        content.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        content.ColumnDefinitions.Add(inspectorColumn);
        content.Children.Add(flash);
        content.Children.Add(dropOverlay);
        Grid.SetColumn(inspectorDivider, 1);
        Ui.SetVisible(inspectorDivider, false);
        content.Children.Add(inspectorDivider);
        BuildInspector();
        Grid.SetColumn(inspector, 2);
        content.Children.Add(inspector);
        var layer = new Border { Style = Ui.Style("ContentLayerStyle"), Child = content };
        Grid.SetRow(layer, 2);
        root.Children.Add(layer);

        root.AllowDrop = true;
        root.DragOver += DragOverWindow;
        root.DragLeave += (_, _) => SetDropTargeted(false);
        root.Drop += DropOnWindow;
        InstallKeyboardAccelerators();

        Content = root;
        ApplyTheme();
        root.ActualThemeChanged += (_, _) => UpdateTitleBarTheme();
        root.Loaded += async (_, _) =>
        {
            UpdateTitleBarTheme();
            root.XamlRoot.Changed += (_, _) => SetMinimumSize();
            await Store.Start();
            _ = Updater.Start();
        };
        Activated += (_, args) => titleText.Style = args.WindowActivationState == WindowActivationState.Deactivated ? Ui.Style("TertiaryTextStyle") : null;
        AppWindow.Closing += (sender, args) =>
        {
            if (!Store.IsBusy)
            {
                Updater.ApplyOnExit();
                return;
            }
            args.Cancel = true;
            _ = ExplainBusyQuit();
        };

        Store.Changed += Update;
        Updater.Changed += Update;
        Store.Failed += ShowFailure;
        // WinUI shows one dialog at a time: an open target dialog closes itself
        // when candidates arrive, and the picker queues behind it.
        Store.ChipPickerRequested += () => _ = ShowChipPicker();
        Update();
    }

    private double Scale => GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;

    private void SetMinimumSize()
    {
        if (AppWindow.Presenter is not OverlappedPresenter presenter) return;
        var scale = Scale;
        presenter.PreferredMinimumWidth = (int)(900 * scale);
        presenter.PreferredMinimumHeight = (int)(600 * scale);
    }

    internal void ApplyTheme() => root.RequestedTheme = AppSettings.Appearance.Theme();

    private void UpdateTitleBarTheme() => AppWindow.TitleBar.PreferredTheme =
        root.ActualTheme == ElementTheme.Dark ? TitleBarTheme.Dark : TitleBarTheme.Light;

    private Grid BuildTitleBar()
    {
        var bar = new Grid { Padding = new Thickness(16, 0, 0, 0), ColumnSpacing = 12, Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent) };
        bar.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        bar.ColumnDefinitions.Add(new());
        var icon = new Image { Source = new BitmapImage(new Uri("ms-appx:///Assets/FlashDesk.png")), Width = 16, Height = 16, VerticalAlignment = VerticalAlignment.Center };
        titleText.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(titleText, 1);
        bar.Children.Add(icon);
        bar.Children.Add(titleText);
        return bar;
    }

    private static MenuBarItem MenuGroup(string title, params MenuFlyoutItemBase[] items)
    {
        var item = new MenuBarItem { Title = title };
        foreach (var child in items) item.Items.Add(child);
        return item;
    }

    private static Border ToolSeparator() => new()
    {
        Style = Ui.Style("DividerStyle"),
        Width = 1,
        Height = 20,
        Margin = new Thickness(6, 0, 6, 0),
        VerticalAlignment = VerticalAlignment.Center,
    };

    private ToggleButton PaneButton(ToggleButton button, string glyph, string tip, InspectorPane pane, InfoBadge? badge)
    {
        var content = new Grid();
        content.Children.Add(Ui.Glyph(glyph, 16));
        if (badge is not null)
        {
            badge.Style = Ui.Style("CriticalValueInfoBadgeStyle");
            content.Children.Add(badge);
        }
        button.Content = content;
        button.Click += (_, _) => TogglePane(pane);
        ToolTipService.SetToolTip(button, tip);
        Ui.Name(button, tip);
        return button;
    }

    private void BuildInspector()
    {
        selector.Items.Add(logsTab);
        selector.Items.Add(memoryTab);
        selector.SelectionChanged += (_, _) =>
        {
            var pane = selector.SelectedItem == memoryTab ? InspectorPane.Memory : InspectorPane.Logs;
            if (openPane is not null && openPane != pane) OpenPane(pane);
        };
        var close = Ui.IconButton(Glyphs.Close, "关闭面板", ClosePane);
        close.VerticalAlignment = VerticalAlignment.Center;
        var header = new Grid { Padding = new Thickness(8, 4, 8, 0) };
        header.ColumnDefinitions.Add(new());
        header.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        Grid.SetColumn(close, 1);
        header.Children.Add(selector);
        header.Children.Add(close);
        inspector.RowDefinitions.Add(new() { Height = GridLength.Auto });
        inspector.RowDefinitions.Add(new());
        Grid.SetRow(logs, 1);
        Grid.SetRow(memory, 1);
        inspector.Children.Add(header);
        inspector.Children.Add(logs);
        inspector.Children.Add(memory);
    }

    private void TogglePane(InspectorPane pane)
    {
        if (openPane == pane) ClosePane();
        else OpenPane(pane);
    }

    private void OpenPane(InspectorPane pane)
    {
        openPane = pane;
        inspectorColumn.Width = new GridLength(InspectorWidth);
        Ui.SetVisible(inspector, true);
        Ui.SetVisible(inspectorDivider, true);
        Ui.SetVisible(logs, pane == InspectorPane.Logs);
        Ui.SetVisible(memory, pane == InspectorPane.Memory);
        selector.SelectedItem = pane == InspectorPane.Logs ? logsTab : memoryTab;
        Store.LogsVisible = pane == InspectorPane.Logs;
        Update();
    }

    private void ClosePane()
    {
        openPane = null;
        inspectorColumn.Width = new GridLength(0);
        Ui.SetVisible(inspector, false);
        Ui.SetVisible(inspectorDivider, false);
        Store.LogsVisible = false;
        Update();
    }

    private void ReadMemory()
    {
        OpenPane(InspectorPane.Memory);
        _ = Store.ReadMemory();
    }

    private void InstallKeyboardAccelerators()
    {
        const VirtualKeyModifiers Control = VirtualKeyModifiers.Control;
        const VirtualKeyModifiers Shift = VirtualKeyModifiers.Control | VirtualKeyModifiers.Shift;
        // Accelerators live on the root, not on menu items, so they work before a menu first opens.
        var accelerators = new[]
        {
            Ui.Accelerator(VirtualKey.O, Control, () => { if (!Store.IsBusy) _ = PickFirmware(); }),
            Ui.Accelerator(VirtualKey.R, Control, () => { if (!Store.IsBusy) _ = Store.ScanProbes(); }),
            Ui.Accelerator(VirtualKey.K, Control, () => _ = Store.Connect()),
            Ui.Accelerator(VirtualKey.K, Shift, Store.Disconnect),
            Ui.Accelerator(VirtualKey.S, Shift, () => { if (!Store.IsBusy) _ = ShowSaveProfile(); }),
            Ui.Accelerator(VirtualKey.Enter, Control, () => _ = Store.Flash()),
            Ui.Accelerator(VirtualKey.R, Shift, () => { if (Store.CanReadMemory) ReadMemory(); }),
            Ui.Accelerator(VirtualKey.J, Control, () => TogglePane(InspectorPane.Logs)),
            Ui.Accelerator(VirtualKey.M, Control | VirtualKeyModifiers.Menu, () => TogglePane(InspectorPane.Memory)),
            Ui.Accelerator((VirtualKey)188, Control, () => _ = ShowSettings()),
        };
        foreach (var accelerator in accelerators) root.KeyboardAccelerators.Add(accelerator);
        root.KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
    }

    private void Update()
    {
        var name = Store.FirmwarePath is { } path ? Path.GetFileName(path) : null;
        AppWindow.Title = titleText.Text = name is null ? "FlashDesk" : $"{name} - FlashDesk";

        var busy = Store.IsBusy;
        openItem.IsEnabled = !busy;
        recentMenu.IsEnabled = !busy && Store.RecentFiles.Count > 0;
        scanItem.IsEnabled = !busy;
        connectItem.IsEnabled = Store.CanConnect;
        disconnectItem.IsEnabled = Store.CanDisconnect;
        chipItem.IsEnabled = saveProfileItem.IsEnabled = !busy;
        loadProfileMenu.IsEnabled = !busy && Store.Profiles.Count > 0;
        flashItem.Text = Store.DryRun ? "开始预检查" : "烧录固件";
        flashItem.IsEnabled = Store.CanFlash;
        readItem.IsEnabled = Store.CanReadMemory;
        eraseItem.IsEnabled = Store.CanErase;
        checkUpdatesItem.IsEnabled = Updater.CanCheck;
        logsItem.IsChecked = openPane == InspectorPane.Logs;
        logsButton.IsChecked = logsItem.IsChecked;
        memoryItem.IsChecked = openPane == InspectorPane.Memory;
        memoryButton.IsChecked = memoryItem.IsChecked;
        profilesButton.IsEnabled = !busy;
        errorBadge.Value = Store.UnseenErrors;
        Ui.SetVisible(errorBadge, Store.UnseenErrors > 0);
        ToolTipService.SetToolTip(logsButton, Store.UnseenErrors > 0 ? $"{Store.UnseenErrors} 条新错误（Ctrl+J）" : "日志（Ctrl+J）");

        if (!ReferenceEquals(shownRecentFiles, Store.RecentFiles))
        {
            shownRecentFiles = Store.RecentFiles;
            recentMenu.Items.Clear();
            foreach (var file in Store.RecentFiles) recentMenu.Items.Add(Ui.MenuItem(Path.GetFileName(file.path), null, () => Store.OpenRecent(file)));
            recentMenu.Items.Add(new MenuFlyoutSeparator());
            recentMenu.Items.Add(Ui.MenuItem("清除菜单", null, () => _ = Store.ClearRecentFiles()));
        }
        if (!ReferenceEquals(shownProfiles, Store.Profiles))
        {
            shownProfiles = Store.Profiles;
            loadProfileMenu.Items.Clear();
            foreach (var profile in Store.Profiles) loadProfileMenu.Items.Add(Ui.MenuItem(profile.name, Glyphs.Bookmark, () => Store.ApplyProfile(profile)));
        }

        activity.Update();
        flash.Update();
        if (openPane == InspectorPane.Memory) memory.Update();
    }

    private void BuildProfilesMenu(MenuFlyout menu)
    {
        menu.Items.Clear();
        menu.Items.Add(Ui.MenuItem("存储当前配置…", Glyphs.Save, () => _ = ShowSaveProfile(), "Ctrl+Shift+S"));
        if (Store.Profiles.Count == 0) return;
        menu.Items.Add(new MenuFlyoutSeparator());
        foreach (var profile in Store.Profiles) menu.Items.Add(Ui.MenuItem(profile.name, Glyphs.Bookmark, () => Store.ApplyProfile(profile)));
        menu.Items.Add(new MenuFlyoutSeparator());
        var delete = new MenuFlyoutSubItem { Text = "删除配置", Icon = new FontIcon { Glyph = Glyphs.Delete } };
        foreach (var profile in Store.Profiles) delete.Items.Add(Ui.MenuItem(profile.name, null, () => _ = Store.DeleteProfile(profile)));
        menu.Items.Add(delete);
    }

    // Dialogs and pickers

    internal async Task PickFirmware()
    {
        if (Store.IsBusy) return;
        var picker = new FileOpenPicker { SuggestedStartLocation = PickerLocationId.DocumentsLibrary };
        foreach (var extension in Program.FirmwareExtensions) picker.FileTypeFilter.Add(extension);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        try
        {
            if (await picker.PickSingleFileAsync() is { } file) Store.SelectFirmware(file.Path);
        }
        catch (Exception error) { Store.Report(error, "打开固件"); }
    }

    internal async Task Export(byte[] data, string name, string kind, string extension, string operation)
    {
        var picker = new FileSavePicker { SuggestedFileName = name, SuggestedStartLocation = PickerLocationId.DocumentsLibrary };
        picker.FileTypeChoices.Add(kind, [extension]);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        try
        {
            if (await picker.PickSaveFileAsync() is not { } file) return;
            await File.WriteAllBytesAsync(file.Path, data);
            Store.AppendLog(operation, file.Path, ActivityLevel.Success);
        }
        catch (Exception error) { Store.Report(error, operation); }
    }

    internal async Task ShowTargetSetup()
    {
        if (targetDialog is not null) return;
        targetDialog = new TargetSetupDialog(Store);
        try { await targetDialog.Show(root); }
        finally { targetDialog = null; }
    }

    internal async Task ShowChipPicker()
    {
        var reconnect = Store.TargetCandidates.Count > 0;
        if (await new ChipPickerDialog(Store).Pick(root) is not { } chip) return;
        Store.Chip = chip;
        if (reconnect) await Store.Connect();
    }

    private Task ShowSaveProfile() => new SaveProfileDialog(Store).Show(root);

    private Task ShowSettings() => new SettingsDialog(Store, this).Show(root);

    internal async Task ConfirmErase()
    {
        if (!Store.CanErase) return;
        var dialog = new ContentDialog
        {
            Title = $"擦除 {Store.TargetName} 的全部 Flash？",
            Content = Ui.Text("芯片中的固件与数据将被永久删除，此操作无法撤销。", wrap: true),
            PrimaryButtonText = "擦除全部 Flash",
            CloseButtonText = "取消",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.Show(root) == ContentDialogResult.Primary) await Store.Erase();
    }

    private async void ShowFailure(UserFacingError error)
    {
        if (targetDialog is not null) { targetDialog.ShowFailure(error); return; }
        var detail = Ui.Text(string.Join(Environment.NewLine + Environment.NewLine,
            new[] { error.Detail, error.Recovery }.Where(text => text.Length > 0)), wrap: true);
        detail.IsTextSelectionEnabled = true;
        var dialog = new ContentDialog
        {
            Title = error.Message,
            Content = detail,
            PrimaryButtonText = "查看日志",
            CloseButtonText = "确定",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.Show(root) == ContentDialogResult.Primary) OpenPane(InspectorPane.Logs);
    }

    private async Task ExplainBusyQuit()
    {
        var dialog = new ContentDialog
        {
            Title = $"“{Store.Operation?.Title()}”正在进行",
            Content = Ui.Text("中断可能使芯片处于不完整的状态。请等待操作完成后再退出 FlashDesk。", wrap: true),
            CloseButtonText = "好",
        };
        await dialog.Show(root);
    }

    private void Quit()
    {
        if (Store.IsBusy) { _ = ExplainBusyQuit(); return; }
        // Window.Close does not raise AppWindow.Closing; ApplyOnExit runs at most once either way.
        Updater.ApplyOnExit();
        Close();
    }

    internal static void RevealInExplorer(string path)
    {
        try { Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{path}\"") { UseShellExecute = true }); }
        catch (Exception) { }
    }

    private static void OpenUri(string url) => _ = Launcher.LaunchUriAsync(new Uri(url));

    // Drag and drop

    private void DragOverWindow(object sender, DragEventArgs args)
    {
        if (Store.IsBusy || !args.DataView.Contains(StandardDataFormats.StorageItems)) return;
        args.AcceptedOperation = DataPackageOperation.Copy;
        args.DragUIOverride.Caption = "打开固件";
        SetDropTargeted(true);
    }

    private async void DropOnWindow(object sender, DragEventArgs args)
    {
        SetDropTargeted(false);
        if (Store.IsBusy || !args.DataView.Contains(StandardDataFormats.StorageItems)) return;
        var deferral = args.GetDeferral();
        try
        {
            var items = await args.DataView.GetStorageItemsAsync();
            if (items.OfType<StorageFile>().FirstOrDefault() is { } file) Store.SelectFirmware(file.Path);
        }
        catch (Exception error) { Store.Report(error, "打开固件"); }
        finally { deferral.Complete(); }
    }

    private void SetDropTargeted(bool targeted)
    {
        if (dropTargeted == targeted) return;
        dropTargeted = targeted;
        flash.SetDropTargeted(targeted);
        Ui.SetVisible(dropOverlay, targeted && Store.FirmwarePath is not null);
    }

    [DllImport("user32.dll", ExactSpelling = true)]
    private static extern uint GetDpiForWindow(nint window);
}

/// The toolbar centre: the latest status, or live progress while a hardware job runs.
internal sealed class ActivityView : UserControl
{
    private readonly WorkbenchStore store;
    private readonly ProgressRing ring = new() { Width = 14, Height = 14, IsActive = true };
    private readonly TextBlock message = Ui.Secondary(size: 13);
    private readonly ProgressBar bar = new() { Width = 90, Maximum = 1, VerticalAlignment = VerticalAlignment.Center };
    private readonly TextBlock percent = Ui.Secondary(size: 13);

    internal ActivityView(WorkbenchStore store)
    {
        this.store = store;
        message.TextTrimming = TextTrimming.CharacterEllipsis;
        message.MaxWidth = 360;
        var row = Ui.Row(8, ring, message, bar, percent);
        row.HorizontalAlignment = HorizontalAlignment.Center;
        row.VerticalAlignment = VerticalAlignment.Center;
        foreach (var child in row.Children) ((FrameworkElement)child).VerticalAlignment = VerticalAlignment.Center;
        Content = row;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(message, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
    }

    internal void Update()
    {
        var progress = store.IsBusy && store.LatestEvent is { } latest && !latest.stage.IsTerminal() ? latest : null;
        message.Text = progress?.message ?? store.Status;
        ToolTipService.SetToolTip(message, message.Text);
        var fraction = progress?.progress;
        Ui.SetVisible(ring, store.IsBusy && fraction is null);
        Ui.SetVisible(bar, fraction is not null);
        Ui.SetVisible(percent, fraction is not null);
        if (fraction is { } value)
        {
            bar.Value = value;
            percent.Text = $"{(int)(value * 100)}%";
        }
    }
}
