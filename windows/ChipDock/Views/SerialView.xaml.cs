using System.Collections.ObjectModel;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using ChipDock.Core;

namespace ChipDock;

public sealed partial class SerialView : UserControl
{
    private readonly ObservableCollection<TerminalFrame> frames = [];
    private readonly ObservableCollection<TerminalFrame> visibleFrames = [];
    private readonly DispatcherQueueTimer timer;
    private readonly SerialAutoSender autoSender = new();
    private SerialStatus status = new() { connected = false, path = null, config = null, rx_bytes = 0, tx_bytes = 0, opened_at = null, last_error = null };
    private bool busy, sending, polling, refreshing, updating = true, started, closed;
    private int ticks, retainedBytes, connectionRevision;
    private double composerHeight = 190;
    private static readonly string[] Formats = ["hex", "ascii", "gbk", "utf-8"];
    private static readonly string[] Endings = ["none", "lf", "cr", "crlf"];
    private readonly SerialPreferences preferences = SerialPreferences.Current;

    public SerialView()
    {
        InitializeComponent();
        Terminal.ItemsSource = visibleFrames;
        BaudPicker.ItemsSource = new[] { "1200", "2400", "4800", "9600", "19200", "38400", "57600", "115200", "230400", "460800", "921600" };
        DataBitsPicker.ItemsSource = new byte[] { 5, 6, 7, 8 }; StopBitsPicker.ItemsSource = new byte[] { 1, 2 };
        ParityPicker.ItemsSource = new[] { "无", "奇校验", "偶校验" }; FlowPicker.ItemsSource = new[] { "无", "XON / XOFF", "RTS / CTS" };
        BaudPicker.SelectedItem = preferences.BaudRate.ToString(System.Globalization.CultureInfo.InvariantCulture);
        BaudPicker.Text = preferences.BaudRate.ToString(System.Globalization.CultureInfo.InvariantCulture);
        DataBitsPicker.SelectedItem = preferences.DataBits; if (DataBitsPicker.SelectedIndex < 0) DataBitsPicker.SelectedItem = (byte)8;
        StopBitsPicker.SelectedItem = preferences.StopBits; if (StopBitsPicker.SelectedIndex < 0) StopBitsPicker.SelectedItem = (byte)1;
        ParityPicker.SelectedIndex = Math.Clamp(preferences.Parity, 0, 2); FlowPicker.SelectedIndex = Math.Clamp(preferences.FlowControl, 0, 2);
        DisplayPicker.ItemsSource = SendFormatPicker.ItemsSource = new[] { "HEX", "ASCII", "GBK", "UTF-8" };
        DisplayPicker.SelectedIndex = IndexOrDefault(Formats, preferences.DisplayFormat, 3);
        SendFormatPicker.SelectedIndex = IndexOrDefault(Formats, preferences.SendFormat, 3);
        EndingPicker.ItemsSource = new[] { "无", "LF", "CR", "CRLF" }; EndingPicker.SelectedIndex = IndexOrDefault(Endings, preferences.LineEnding, 0);
        ChecksumPicker.ItemsSource = new[] { "无校验（默认）", "校验和（SUM8）", "奇校验", "偶校验", "异或校验（XOR8）", "CRC16/MODBUS" };
        ChecksumPicker.SelectedIndex = Math.Clamp(preferences.Checksum, 0, 5);
        AutoSendInterval.Value = Math.Clamp(preferences.AutoSendIntervalMilliseconds, 10, 3_600_000);
        AutoScrollToggle.IsChecked = preferences.AutoScroll;
        timer = DispatcherQueue.CreateTimer(); timer.Interval = TimeSpan.FromMilliseconds(100); timer.Tick += OnTick;
        autoSender.Changed += RefreshState;
        autoSender.Failed += error => { if (!closed) ShowError(error.Message); };
        BaudPicker.RegisterPropertyChangedCallback(ComboBox.TextProperty, (_, _) => { if (!updating) RefreshState(); });
        updating = false; RefreshState();
    }

    private static int IndexOrDefault(string[] values, string value, int fallback) => Array.IndexOf(values, value) is var index && index >= 0 ? index : fallback;
    private async void OnLoaded(object sender, RoutedEventArgs args)
    {
        if (started) return; started = true;
        await RefreshPorts();
        if (!closed) timer.Start();
    }
    private async void OnTick(DispatcherQueueTimer sender, object args)
    {
        if (closed || polling || busy) return;
        polling = true; var revision = connectionRevision;
        try
        {
            var batch = await SerialBackend.Poll();
            if (closed) return;
            if (!busy && revision == connectionRevision)
            {
                if (batch.status.last_error is { } error && error != status.last_error) ShowError(error);
                status = batch.status;
                if (!status.connected) autoSender.Stop();
            }
            foreach (var frame in batch.frames)
            {
                var item = new TerminalFrame(frame, preferences.DisplayFormat); frames.Add(item); retainedBytes += item.MemoryCost;
            }
            while (frames.Count > 2000 || retainedBytes > 8 * 1024 * 1024) { retainedBytes -= frames[0].MemoryCost; frames.RemoveAt(0); }
            if (batch.dropped_frames > 0) ShowError($"显示缓冲已丢弃 {batch.dropped_frames} 条旧记录；字节统计保持完整。");
            if (batch.frames.Count > 0) RefreshTerminal();
            RefreshState();
            if (++ticks % 10 == 0) await RefreshPorts(silent: true);
        }
        catch (Exception error) { autoSender.Stop(); if (!closed) ShowError(error.Message); }
        finally { polling = false; }
    }
    internal async Task RefreshPorts(bool silent = false)
    {
        if (!CanRefresh) return;
        refreshing = true; RefreshState();
        var revision = connectionRevision;
        try
        {
            var ports = await SerialBackend.Ports();
            // A scan started before connect/disconnect must not replace its device selection.
            if (closed || busy || revision != connectionRevision) return;
            var choices = ports.Select(p => new PortChoice(p.path, p.display_name, p)).ToList();
            var selected = status.connected ? status.path : preferences.SelectedPath;
            if (status.connected && !choices.Any(p => p.Path == selected) && selected is not null) choices.Add(new(selected, selected + "（已移除）", null));
            // Preserve the selected device and keyboard focus between hotplug scans.
            if (PortPicker.ItemsSource is List<PortChoice> current && current.Count == choices.Count && current.Zip(choices).All(pair => SamePort(pair.First, pair.Second))) return;
            updating = true;
            PortPicker.ItemsSource = choices;
            PortPicker.SelectedItem = choices.FirstOrDefault(p => p.Path == selected) ?? choices.FirstOrDefault();
            if (!status.connected) preferences.SelectedPath = (PortPicker.SelectedItem as PortChoice)?.Path ?? "";
        }
        catch (Exception error) { if (!silent && !closed) ShowError(error.Message); }
        finally { updating = false; refreshing = false; if (!closed) RefreshState(); }
    }
    private async void OnConnect(object sender, RoutedEventArgs args) => await ToggleConnection();
    internal async Task ToggleConnection()
    {
        if (!CanToggleConnection) return;
        autoSender.Stop();
        busy = true; connectionRevision++; RefreshState();
        try
        {
            SaveSerialPreferences(); ErrorBar.IsOpen = false;
            status = status.connected ? await SerialBackend.Disconnect() : await SerialBackend.Connect(preferences.SelectedPath, new SerialConfig
            {
                baud_rate = preferences.BaudRate, data_bits = preferences.DataBits, stop_bits = preferences.StopBits,
                parity = preferences.Parity switch { 1 => Parity.Odd, 2 => Parity.Even, _ => Parity.None },
                flow_control = preferences.FlowControl switch { 1 => FlowControl.Software, 2 => FlowControl.Hardware, _ => FlowControl.None }
            });

        }
        catch (Exception error) { if (!closed) ShowError(error.Message); }
        finally { busy = false; if (!closed) RefreshState(); }
    }
    private async void OnSend(object sender, RoutedEventArgs args) => await Send();
    internal async Task Send()
    {
        if (!CanSend) return;
        await Transmit();
    }
    private async Task<bool> Transmit()
    {
        if (!CanTransmit) return false;
        var revision = connectionRevision;
        sending = true; RefreshState();
        var format = SendFormatPicker.SelectedIndex switch { 0 => Core.SendFormat.Hex, 1 => Core.SendFormat.Ascii, 2 => Core.SendFormat.Gbk, _ => Core.SendFormat.Utf8 };
        var ending = EndingPicker.SelectedIndex switch { 1 => Core.LineEnding.Lf, 2 => Core.LineEnding.Cr, 3 => Core.LineEnding.Crlf, _ => Core.LineEnding.None };
        var checksum = ChecksumPicker.SelectedIndex switch
        {
            1 => ChecksumAlgorithm.Sum8, 2 => ChecksumAlgorithm.OddParity, 3 => ChecksumAlgorithm.EvenParity,
            4 => ChecksumAlgorithm.Xor8, 5 => ChecksumAlgorithm.Crc16Modbus, _ => ChecksumAlgorithm.None
        };
        try
        {
            await SerialBackend.Send(InputBox.Text, format, ending, checksum);
            return !closed && status.connected && !busy && revision == connectionRevision;
        }
        catch (Exception error) { if (!closed && revision == connectionRevision) ShowError(error.Message); return false; }
        finally { sending = false; if (!closed) RefreshState(); }
    }
    private void OnAutoSend(object sender, RoutedEventArgs args)
    {
        if (autoSender.IsRunning) { autoSender.Stop(); return; }
        if (CanSend && TryGetAutoSendInterval(out var interval)) autoSender.Start(interval, Transmit);
    }
    private bool TryGetAutoSendInterval(out int interval)
    {
        var value = AutoSendInterval.Value;
        interval = 0;
        if (!double.IsFinite(value) || value != Math.Truncate(value) || value is < 10 or > 3_600_000) return false;
        interval = (int)value;
        return true;
    }
    private void OnAutoSendIntervalChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (updating) return;
        try
        {
            if (TryGetAutoSendInterval(out var interval)) { preferences.AutoSendIntervalMilliseconds = interval; preferences.Save(); }
            RefreshState();
        }
        catch (Exception error) { ShowError(error.Message); }
    }
    private bool TryGetBaudRate(out uint baud) => uint.TryParse(BaudPicker.Text.Trim(), System.Globalization.NumberStyles.None,
        System.Globalization.CultureInfo.InvariantCulture, out baud) && baud > 0;
    private void OnBaudTextSubmitted(ComboBox sender, ComboBoxTextSubmittedEventArgs args)
    {
        if (updating) return;
        sender.Text = args.Text.Trim();
        if (!TryGetBaudRate(out _))
        {
            args.Handled = true;
            ShowError("波特率必须是 1–4294967295 的整数。"); RefreshState(); return;
        }
        // WinUI must commit valid custom text to SelectedItem to retain it after focus changes.
        args.Handled = false;
        try { SaveSerialPreferences(); RefreshState(); }
        catch (Exception error) { ShowError(error.Message); }
    }
    private void RefreshState()
    {
        if (closed) return;
        PortPicker.IsEnabled = !busy && !status.connected && PortPicker.Items.Count > 0;
        foreach (var picker in new[] { BaudPicker, DataBitsPicker, StopBitsPicker, ParityPicker, FlowPicker })
            picker.IsEnabled = !busy && !status.connected;
        ConnectButton.IsEnabled = CanToggleConnection;
        ConnectLabel.Text = busy ? "正在处理…" : status.connected ? "断开连接" : "连接串口";
        ConnectIcon.Symbol = status.connected ? Symbol.Stop : Symbol.Link;
        SendButton.IsEnabled = CanSend;
        SendLabel.Text = sending ? "发送中…" : "发送";
        var automatic = autoSender.IsRunning;
        SendOptions.IsEnabled = !automatic;
        InputBox.IsReadOnly = automatic;
        AutoSendInterval.IsEnabled = !automatic;
        AutoSendButton.IsEnabled = automatic || (CanSend && TryGetAutoSendInterval(out _));
        AutoSendLabel.Text = automatic ? "停止自动发送" : "自动发送";
        AutoSendIcon.Glyph = automatic ? "\uE71A" : "\uE8EE";
        InputCount.Text = InputBox.Text.Length == 0 ? "" : $"{new System.Globalization.StringInfo(InputBox.Text).LengthInTextElements:N0} 字符";
        EndingPicker.IsEnabled = SendFormatPicker.SelectedIndex != 0;
        InputBox.PlaceholderText = SendFormatPicker.SelectedIndex == 0 ? "十六进制数据，例如 AA 01 FF" : "输入文本或设备指令…";
        ToolTipService.SetToolTip(InputBox, SendFormatPicker.SelectedIndex == 0 ? "HEX 按原始字节发送，不追加行尾。" : "Ctrl+Enter 发送 · Enter 换行");
        EmptyState.Visibility = visibleFrames.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        var filteredEmpty = frames.Count > 0 && visibleFrames.Count == 0;
        EmptyIcon.Glyph = filteredEmpty ? "\uE721" : "\uE756";
        EmptyTitle.Text = filteredEmpty ? "没有匹配的数据" : status.connected ? "等待设备数据" : "暂无收发记录";
        EmptyDescription.Text = filteredEmpty ? "尝试其他搜索内容，或显示全部方向。"
            : status.connected ? "设备数据会实时显示在这里。" : "在左侧选择串口并连接，即可开始收发。";
        ResetFiltersButton.Visibility = filteredEmpty ? Visibility.Visible : Visibility.Collapsed;
        FrameCount.Text = TerminalSearch.Text.Length == 0 && DirectionPicker.SelectedIndex == 0
            ? $"{frames.Count} 条" : $"{visibleFrames.Count}/{frames.Count} 条";
        ClearButton.IsEnabled = frames.Count > 0;
        var parity = preferences.Parity switch { 1 => "O", 2 => "E", _ => "N" };
        ConnectionStatus.Text = busy ? "正在处理…" : status.connected
            ? $"已连接 · {status.path} · {preferences.BaudRate} · {preferences.DataBits}{parity}{preferences.StopBits}" : "未连接";
        ByteStatus.Text = $"RX {status.rx_bytes:N0} B    TX {status.tx_bytes:N0} B";
        RefreshButton.IsEnabled = CanRefresh;
        var blocked = UpdateBlocked;
        if (blocked != lastUpdateBlocked || CanSend != lastCanSend || CanToggleConnection != lastCanToggle || refreshing != lastRefreshing)
        {
            lastUpdateBlocked = blocked; lastCanSend = CanSend; lastCanToggle = CanToggleConnection; lastRefreshing = refreshing;
            ActivityChanged?.Invoke();
        }
    }
    private void SaveSerialPreferences()
    {
        if (TryGetBaudRate(out var baud)) preferences.BaudRate = baud;
        preferences.DataBits = DataBitsPicker.SelectedItem is byte bits ? bits : (byte)8;
        preferences.StopBits = StopBitsPicker.SelectedItem is byte stops ? stops : (byte)1;
        preferences.Parity = Math.Max(0, ParityPicker.SelectedIndex); preferences.FlowControl = Math.Max(0, FlowPicker.SelectedIndex);
        preferences.SelectedPath = (PortPicker.SelectedItem as PortChoice)?.Path ?? "";
        preferences.DisplayFormat = Formats[Math.Clamp(DisplayPicker.SelectedIndex, 0, 3)]; preferences.SendFormat = Formats[Math.Clamp(SendFormatPicker.SelectedIndex, 0, 3)];
        preferences.LineEnding = Endings[Math.Clamp(EndingPicker.SelectedIndex, 0, 3)]; preferences.AutoScroll = AutoScrollToggle.IsChecked == true;
        preferences.Checksum = Math.Clamp(ChecksumPicker.SelectedIndex, 0, 5);
        preferences.Save();
    }
    private void OnSelectionChanged(object sender, SelectionChangedEventArgs args)
    {
        if (updating) return;
        try
        {
            if (ReferenceEquals(sender, BaudPicker) && BaudPicker.SelectedItem is string rate) BaudPicker.Text = rate;
            SaveSerialPreferences(); RefreshState();
        }
        catch (Exception error) { ShowError(error.Message); }
    }
    private void OnDisplayChanged(object sender, SelectionChangedEventArgs args)
    {
        if (updating) return;
        OnSelectionChanged(sender, args);
        foreach (var frame in frames) frame.SetFormat(preferences.DisplayFormat);
        RefreshTerminal(); RefreshState();
    }
    private void OnAutoScroll(object sender, RoutedEventArgs args)
    {
        if (updating) return;
        try { SaveSerialPreferences(); ScrollToLatest(); } catch (Exception error) { ShowError(error.Message); }
    }
    private void OnInputChanged(object sender, TextChangedEventArgs args) { if (!updating) RefreshState(); }
    private void OnClear(object sender, RoutedEventArgs args) { frames.Clear(); visibleFrames.Clear(); retainedBytes = 0; RefreshState(); }
    private async void OnRefresh(object sender, RoutedEventArgs args) => await RefreshPorts();
    private void RefreshTerminal()
    {
        var query = TerminalSearch.Text;
        var direction = DirectionPicker.SelectedIndex;
        var matches = frames.Where(frame =>
            (direction <= 0 || frame.IsReceive == (direction == 1))
            && (query.Length == 0 || frame.DisplayText.Contains(query, StringComparison.CurrentCultureIgnoreCase))).ToList();
        var retained = matches.ToHashSet();
        // Keep existing rows alive so incoming data doesn't reset selection or scroll position.
        for (var index = visibleFrames.Count - 1; index >= 0; index--)
            if (!retained.Contains(visibleFrames[index])) visibleFrames.RemoveAt(index);
        for (var index = 0; index < matches.Count; index++)
            if (index >= visibleFrames.Count || !ReferenceEquals(visibleFrames[index], matches[index]))
                visibleFrames.Insert(index, matches[index]);
        ScrollToLatest();
    }
    private void ScrollToLatest()
    {
        if (preferences.AutoScroll && visibleFrames.Count > 0) Terminal.ScrollIntoView(visibleFrames[^1]);
    }
    private void OnSearchChanged(object sender, TextChangedEventArgs args)
    {
        if (updating) return;
        RefreshTerminal(); RefreshState();
    }
    private void OnFilterChanged(object sender, SelectionChangedEventArgs args)
    {
        if (updating) return;
        RefreshTerminal(); RefreshState();
    }
    private void OnResetFilters(object sender, RoutedEventArgs args)
    {
        updating = true;
        TerminalSearch.Text = ""; DirectionPicker.SelectedIndex = 0;
        updating = false;
        RefreshTerminal(); RefreshState();
    }
    private void OnToggleConnectionPane(object sender, RoutedEventArgs args)
    {
        var expanded = ConnectionPaneToggle.IsChecked == true;
        ConnectionPanel.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
        ConnectionColumn.Width = new GridLength(expanded ? 216 : 0);
        ToolTipService.SetToolTip(ConnectionPaneToggle, expanded ? "收起连接区" : "展开连接区");
    }
    private void OnToggleComposerPane(object sender, RoutedEventArgs args)
    {
        var expanded = ComposerPaneToggle.IsChecked == true;
        ComposerPanel.Visibility = ComposerSplitter.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
        ComposerRow.MinHeight = expanded ? 164 : 0;
        ComposerSplitterRow.Height = new GridLength(expanded ? 5 : 0);
        if (expanded) SetComposerHeight(composerHeight);
        else ComposerRow.Height = new GridLength(0);
        ToolTipService.SetToolTip(ComposerPaneToggle, expanded ? "收起发送区" : "展开发送区");
    }
    private void OnComposerResize(object sender, DragDeltaEventArgs args)
        => SetComposerHeight(ComposerRow.ActualHeight - args.VerticalChange);
    private void OnSplitterKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key is not (Windows.System.VirtualKey.Up or Windows.System.VirtualKey.Down)) return;
        SetComposerHeight(ComposerRow.ActualHeight + (args.Key == Windows.System.VirtualKey.Up ? 16 : -16));
        args.Handled = true;
    }
    private void SetComposerHeight(double height)
    {
        var maximum = Math.Min(320, Math.Max(164, Workspace.ActualHeight - ErrorBar.ActualHeight - 185));
        composerHeight = Math.Clamp(height, 164, maximum);
        ComposerRow.Height = new GridLength(composerHeight);
    }
    private void OnRootSizeChanged(object sender, SizeChangedEventArgs args)
    {
        if (Workspace is not null && ComposerRow is not null && ErrorBar is not null
            && ComposerPaneToggle?.IsChecked == true && Workspace.ActualHeight > 0)
            SetComposerHeight(composerHeight);
    }
    private void ShowError(string message) { ErrorBar.Message = message; ErrorBar.IsOpen = true; }
    private async void OnConnectShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { args.Handled = true; await ToggleConnection(); }
    private async void OnSendShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { args.Handled = true; await Send(); }
    private async void OnRefreshShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { args.Handled = true; await RefreshPorts(); }
    private void OnClearShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { args.Handled = true; OnClear(sender, new()); }
    private void OnFindShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true; TerminalSearch.Focus(FocusState.Programmatic); TerminalSearch.SelectAll();
    }
    internal bool CanDisconnect => status.connected && !busy;
    internal Task Disconnect() => CanDisconnect ? ToggleConnection() : Task.CompletedTask;
    internal bool CanRefresh => !refreshing && !busy && !closed;
    internal bool CanToggleConnection => !closed && !busy && (status.connected || (PortPicker.SelectedItem is PortChoice && TryGetBaudRate(out _)));
    private bool CanTransmit => !closed && status.connected && !sending && !busy && InputBox.Text.Length > 0;
    internal bool CanSend => CanTransmit && !autoSender.IsRunning;
    internal bool UpdateBlocked => status.connected || busy || sending || autoSender.IsRunning;
    private bool lastUpdateBlocked, lastCanSend, lastCanToggle, lastRefreshing;
    internal event Action? ActivityChanged;
    internal void ClearTerminal() => OnClear(this, new());
    internal void Find() { TerminalSearch.Focus(FocusState.Programmatic); TerminalSearch.SelectAll(); }
    internal void Shutdown()
    {
        closed = true;
        autoSender.Stop();
        timer.Stop();
        try { SerialBackend.Shutdown(); }
        catch (Exception error) { System.Diagnostics.Trace.TraceError("{0}", error); }
    }
    private sealed record PortChoice(string Path, string DisplayName, PortInfo? Info);
    private static bool SamePort(PortChoice left, PortChoice right) => left.Path == right.Path && left.DisplayName == right.DisplayName
        && left.Info?.manufacturer == right.Info?.manufacturer && left.Info?.serial_number == right.Info?.serial_number
        && left.Info?.vid == right.Info?.vid && left.Info?.pid == right.Info?.pid;
}
