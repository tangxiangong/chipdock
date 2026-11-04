using System.Collections.ObjectModel;
using System.Text.Json;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using ChipDock.Core;
using Windows.ApplicationModel.DataTransfer;
using VirtualKey = Windows.System.VirtualKey;
using VirtualKeyModifiers = Windows.System.VirtualKeyModifiers;

namespace ChipDock;

public sealed partial class SerialView : UserControl
{
    private string protocolBuffer = "", commandHex = "", pendingResponse = "";
    private DateTimeOffset? responseDeadline, responseStartedAt;
    private readonly List<string> protocolRecords = [];
    private readonly List<string> commandHistory = [];
    private int historyIndex, previewRevision, protocolRevision;
    private bool IsCommand => CommandMode.IsChecked == true;
    private async Task<JsonElement> Protocol(string operation, string input = "", string incoming = "", string? rules = null)
    {
        var json = JsonSerializer.Serialize(new { rules = JsonSerializer.Deserialize<JsonElement>(rules ?? preferences.ProtocolRules), operation, input, incoming, buffer = operation == "decode" ? protocolBuffer : "" });
        return JsonSerializer.Deserialize<JsonElement>(await SerialBackend.Program(json));
    }
    private void ResetProtocol() { protocolBuffer = ""; pendingResponse = ""; responseDeadline = null; responseStartedAt = null; }
    private async void OnCommandMode(object sender, RoutedEventArgs args)
    {
        autoSender.Stop(); InputBox.AcceptsReturn = !IsCommand;
        CommandPreviewViewport.Visibility = IsCommand ? Visibility.Visible : Visibility.Collapsed;
        await PreviewCommand(); RefreshState();
    }
    private async Task PreviewCommand()
    {
        var revision = ++previewRevision; commandHex = "";
        if (!IsCommand) { CommandPreview.Text = ""; return; }
        try {
            var result = await Protocol("encode", InputBox.Text);
            if (revision != previewRevision || !IsCommand) return;
            commandHex = result.GetProperty("hex").GetString() ?? "";
            CommandPreview.Text = "待发送：" + commandHex;
        } catch (Exception error) { if (revision == previewRevision) CommandPreview.Text = error.Message; }
        RefreshState();
    }
    private async void OnCommandKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (!IsCommand) return;
        if (args.Key == VirtualKey.Enter) { args.Handled = true; await Send(); }
        else if (args.Key is VirtualKey.Up or VirtualKey.Down) {
            args.Handled = true; historyIndex = Math.Clamp(historyIndex + (args.Key == VirtualKey.Up ? -1 : 1), 0, commandHistory.Count);
            InputBox.Text = historyIndex == commandHistory.Count ? "" : commandHistory[historyIndex];
        } else if (args.Key == VirtualKey.Tab) {
            var rules = JsonSerializer.Deserialize<JsonElement>(preferences.ProtocolRules);
            var matches = rules.EnumerateArray().Where(r => (!r.TryGetProperty("receive", out var rx) || !rx.GetBoolean()) && (r.GetProperty("name").GetString() ?? "").StartsWith(InputBox.Text, StringComparison.Ordinal)).ToArray();
            if (matches.Length == 1) { InputBox.Text = matches[0].GetProperty("name").GetString() + " "; args.Handled = true; }
        }
    }
    private void OnProtocolDisplay(object sender, SelectionChangedEventArgs args)
    {
        if (updating || ParsedRow is null) return;
        RefreshProtocolDisplay();
    }
    private void RefreshProtocolDisplay()
    {
        var hasRules = JsonSerializer.Deserialize<JsonElement>(preferences.ProtocolRules).GetArrayLength() > 0;
        ProtocolDisplay.Visibility = hasRules ? Visibility.Visible : Visibility.Collapsed;
        ParsedRow.Height = !hasRules || ProtocolDisplay.SelectedIndex == 0 ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        RawRow.Height = hasRules && ProtocolDisplay.SelectedIndex == 1 ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
    }
    private async void OnProtocolRules(object sender, RoutedEventArgs args)
    {
        var rules = JsonSerializer.Deserialize<List<ProtocolRule>>(preferences.ProtocolRules) ?? [];
        var list = new ListView { SelectionMode = ListViewSelectionMode.Single, Margin = new Thickness(0, 8, 0, 0) };
        var ruleCount = new TextBlock { Style = Ui.Style("CaptionSecondaryTextStyle") };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(list, "协议规则列表");
        var mono = new Microsoft.UI.Xaml.Media.FontFamily("Consolas");
        var name = new TextBox { Header = "命令名称 / Alias", PlaceholderText = "例如 motor-speed", FontFamily = mono };
        var receive = new ToggleSwitch { Header = "规则类型", OffContent = "发送命令", OnContent = "接收解析" };
        var template = new TextBox { Header = "帧模板", PlaceholderText = "AA BB {speed:u16be} {checksum:sum8}", AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MinHeight = 72, MaxHeight = 140, FontFamily = mono };
        var display = new TextBox { Header = "显示模板 · 可选", PlaceholderText = "例如 电机速度：{speed} rpm" };
        var checksumStart = new TextBox { Header = "校验起点 · 从 0 开始", Text = "0" };
        var checksumEnd = new TextBox { Header = "校验终点 · 不包含", PlaceholderText = "自动，直到校验位前" };
        var delimiter = new TextBox { Header = "文本分隔符 HEX · 可选", PlaceholderText = "例如 0D 0A", FontFamily = mono };
        var response = new TextBox { Header = "关联接收规则 · 可选", PlaceholderText = "接收规则的名称" };
        var timeout = new TextBox { Header = "等待超时 · ms", Text = "1000" };
        var test = new TextBox { Header = "输入命令 · 包含 alias", PlaceholderText = "motor-speed 45", FontFamily = mono };
        var output = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true, FontFamily = mono };
        var notice = new InfoBar { IsOpen = false, IsClosable = true, Severity = InfoBarSeverity.Error };
        void Report(Exception error) { notice.Severity = InfoBarSeverity.Error; notice.Message = error.Message; notice.IsOpen = true; }
        static TextBlock Hint(string text) => new() { Text = text, TextWrapping = TextWrapping.Wrap, Style = Ui.Style("CaptionSecondaryTextStyle") };
        static StackPanel Stack(params UIElement[] items) { var panel = new StackPanel { Spacing = 12 }; foreach (var item in items) panel.Children.Add(item); return panel; }
        static StackPanel Heading(string glyph, string title) {
            var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
            panel.Children.Add(new FontIcon { Glyph = glyph, FontSize = 16, Style = Ui.Style("AccentGlyphStyle") });
            panel.Children.Add(new TextBlock { Text = title, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, FontSize = 14 });
            return panel;
        }
        var fieldPairs = new List<Grid>();
        Grid Pair(FrameworkElement first, FrameworkElement second) {
            var grid = new Grid { ColumnSpacing = 16, RowSpacing = 12 };
            grid.ColumnDefinitions.Add(new ColumnDefinition()); grid.ColumnDefinitions.Add(new ColumnDefinition());
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto }); grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            grid.Children.Add(first); grid.Children.Add(second); Grid.SetColumn(second, 1);
            fieldPairs.Add(grid); return grid;
        }
        static Border Section(string glyph, string title, params UIElement[] items) {
            var panel = Stack(Heading(glyph, title));
            foreach (var item in items) panel.Children.Add(item);
            var border = new Border { Child = panel, Padding = new Thickness(20), CornerRadius = new CornerRadius(8), BorderThickness = new Thickness(1) };
            border.Style = Ui.Style("CardStyle");
            return border;
        }
        var syntax = new Expander { Header = "模板语法与示例", HorizontalAlignment = HorizontalAlignment.Stretch,
            Content = Stack(Hint("AA BB {speed:u16be:0:3000} {checksum:sum8}"), Hint("整数：u8 / i8，u16 / i16 / u32 / i32 配合 be（大端）或 le（小端）。参数顺序与模板一致。"), Hint("校验：sum8、xor8、crc16modbus，放在帧尾。")) };
        var receiveOptions = Stack(delimiter, Hint("文本解析时清空帧模板，显示模板使用 {text}。单帧最多 4096 字节。"));
        var sendOptions = Stack(Pair(response, timeout));
        var run = new Button { Content = "生成预览", MinWidth = 112, HorizontalAlignment = HorizontalAlignment.Right };
        var outputPlaceholder = Hint("输入命令后生成预览，结果将在这里显示。");
        var outputContent = new Grid(); outputContent.Children.Add(outputPlaceholder); outputContent.Children.Add(output);
        var outputWell = new Border { Style = Ui.Style("WellStyle"), CornerRadius = new CornerRadius(6), Padding = new Thickness(14), MinHeight = 64, Child = outputContent };
        output.RegisterPropertyChangedCallback(TextBlock.TextProperty, (_, _) => outputPlaceholder.Visibility = string.IsNullOrEmpty(output.Text) ? Visibility.Visible : Visibility.Collapsed);
        var testSection = Section("\uE9D9", "离线试算", Hint("预览发送字节或解析接收数据，不会向设备发送。"), test, run, outputWell);
        var selected = -1;
        var editingRevision = 0;
        const string fragmentFormat = "ChipDock.FrameFragment.v1";
        var fragments = new (string Title, string Token)[] {
            ("帧头 AA BB", "AA BB"), ("u8", "{value:u8}"), ("u16be", "{value:u16be}"),
            ("u16le", "{value:u16le}"), ("i16be", "{value:i16be}"), ("u32be", "{value:u32be}"),
            ("SUM8", "{checksum:sum8}"), ("XOR8", "{checksum:xor8}"), ("CRC16-Modbus", "{checksum:crc16modbus}"),
        };
        void InsertFragment(string token) {
            if (!fragments.Any(fragment => fragment.Token == token)) return;
            var tokens = template.Text.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries).ToList();
            if (token.StartsWith("{checksum:", StringComparison.Ordinal)) {
                tokens.RemoveAll(value => value.StartsWith("{checksum:", StringComparison.Ordinal)); tokens.Add(token);
            } else if (token == "AA BB") { tokens.InsertRange(0, new[] { "AA", "BB" }); }
            else {
                var fieldName = "value"; var suffix = 2;
                while (tokens.Any(value => value.StartsWith("{" + fieldName + ":", StringComparison.Ordinal))) fieldName = "value" + suffix++;
                var fragment = token.Replace("{value:", "{" + fieldName + ":");
                var index = tokens.FindIndex(value => value.StartsWith("{checksum:", StringComparison.Ordinal));
                tokens.Insert(index < 0 ? tokens.Count : index, fragment);
            }
            template.Text = string.Join(" ", tokens);
        }
        var fragmentGrid = new Grid { ColumnSpacing = 8, RowSpacing = 8 };
        for (var column = 0; column < 3; column++) fragmentGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        for (var row = 0; row < 3; row++) fragmentGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        for (var index = 0; index < fragments.Length; index++) {
            var fragment = fragments[index];
            var button = new Button { Content = fragment.Title, CanDrag = true, HorizontalAlignment = HorizontalAlignment.Stretch, Padding = new Thickness(8, 6, 8, 6), FontSize = 12, FontFamily = mono, CornerRadius = new CornerRadius(6) };
            button.Click += (_, _) => InsertFragment(fragment.Token);
            button.DragStarting += (_, e) => { e.Data.SetData(fragmentFormat, fragment.Token); e.Data.RequestedOperation = DataPackageOperation.Copy; };
            ToolTipService.SetToolTip(button, fragment.Token); Grid.SetColumn(button, index % 3); Grid.SetRow(button, index / 3); fragmentGrid.Children.Add(button);
        }
        template.AllowDrop = true;
        template.DragOver += (_, e) => {
            if (!e.DataView.Contains(fragmentFormat)) return;
            e.AcceptedOperation = DataPackageOperation.Copy; e.DragUIOverride.Caption = "插入帧模板片段"; e.DragUIOverride.IsCaptionVisible = true; e.Handled = true;
        };
        template.Drop += async (_, e) => {
            if (!e.DataView.Contains(fragmentFormat)) return;
            var deferral = e.GetDeferral(); e.Handled = true;
            var target = selected; var revision = editingRevision;
            try { if (await e.DataView.GetDataAsync(fragmentFormat) is string token && target == selected && revision == editingRevision) { InsertFragment(token); e.AcceptedOperation = DataPackageOperation.Copy; } }
            catch (Exception error) { Report(error); }
            finally { deferral.Complete(); }
        };
        var advanced = new Expander { Header = Heading("\uE713", "校验与反馈"), HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch, Content = Stack(Pair(checksumStart, checksumEnd), receiveOptions, sendOptions) };
        var editor = Stack(Section("\uE8D2", "规则信息", Pair(name, receive)),
            Section("\uE943", "帧模板", template, Hint("点击或拖入片段，快速组合数据帧。"), fragmentGrid, display, syntax), advanced, testSection);
        editor.Spacing = 16;
        var empty = Stack(new FontIcon { Glyph = "\uE8D2", FontSize = 36, Style = Ui.Style("AccentGlyphStyle"), HorizontalAlignment = HorizontalAlignment.Left },
            new TextBlock { Text = "从一条规则开始", FontSize = 20, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold },
            Hint("将常用指令保存为命令，或把设备返回的数据转换为易读的结果。"));
        empty.Margin = new Thickness(24, 36, 24, 36);
        var detail = new Grid(); detail.Children.Add(editor); detail.Children.Add(empty);
        var changingSelection = false;
        var delete = new Button { Content = new SymbolIcon(Symbol.Delete) };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(delete, "删除所选规则");
        ToolTipService.SetToolTip(delete, "删除所选规则");
        void UpdateKind() {
            receiveOptions.Visibility = receive.IsOn ? Visibility.Visible : Visibility.Collapsed;
            sendOptions.Visibility = receive.IsOn ? Visibility.Collapsed : Visibility.Visible;
            test.Header = receive.IsOn ? "输入接收数据 · HEX" : "输入命令 · 包含 alias";
            test.PlaceholderText = receive.IsOn ? "CC DD 00 2D D6" : "motor-speed 45";
            run.Content = receive.IsOn ? "解析数据" : "生成预览";
            outputPlaceholder.Text = receive.IsOn ? "输入接收数据后解析，结果将在这里显示。" : "输入命令后生成预览，结果将在这里显示。";
        }
        void Store() {
            if (selected < 0 || selected >= rules.Count) return;
            if (!int.TryParse(checksumStart.Text, out var startValue) || !int.TryParse(timeout.Text, out var timeoutValue))
                throw new InvalidOperationException("校验起点和反馈超时必须为整数。");
            int? endValue = null;
            if (!string.IsNullOrWhiteSpace(checksumEnd.Text)) {
                if (!int.TryParse(checksumEnd.Text, out var value)) throw new InvalidOperationException("校验终点必须为整数或留空。");
                endValue = value;
            }
            rules[selected] = new ProtocolRule { name = name.Text, receive = receive.IsOn, template = template.Text, display = display.Text,
                checksum_start = startValue, checksum_end = endValue, delimiter = delimiter.Text, response = response.Text, timeout_ms = timeoutValue };
        }
        void Load(int index) {
            selected = index; editingRevision++; output.Text = ""; test.Text = ""; notice.IsOpen = false;
            var hasSelection = index >= 0 && index < rules.Count;
            editor.Visibility = hasSelection ? Visibility.Visible : Visibility.Collapsed;
            empty.Visibility = hasSelection ? Visibility.Collapsed : Visibility.Visible;
            delete.IsEnabled = hasSelection;
            if (!hasSelection) return;
            var r = rules[index]; name.Text = r.name; receive.IsOn = r.receive; template.Text = r.template; display.Text = r.display;
            checksumStart.Text = r.checksum_start.ToString(); checksumEnd.Text = r.checksum_end?.ToString() ?? "";
            delimiter.Text = r.delimiter; response.Text = r.response; timeout.Text = r.timeout_ms.ToString(); UpdateKind();
        }
        void Refresh(int index) {
            changingSelection = true;
            list.Items.Clear();
            ruleCount.Text = $"{rules.Count} 条规则";
            foreach (var rule in rules) {
                var label = Stack(new TextBlock { Text = rule.name, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextTrimming = TextTrimming.CharacterEllipsis }, Hint(rule.receive ? "接收解析" : "发送命令"));
                label.Spacing = 3;
                var row = new Grid { ColumnSpacing = 12 };
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto }); row.ColumnDefinitions.Add(new ColumnDefinition());
                row.Children.Add(new FontIcon { Glyph = rule.receive ? "\uE896" : "\uE898", FontSize = 16, Style = Ui.Style("AccentGlyphStyle") });
                Grid.SetColumn(label, 1); row.Children.Add(label);
                var item = new ListViewItem { Content = row, HorizontalContentAlignment = HorizontalAlignment.Stretch, Padding = new Thickness(12, 10, 12, 10), Margin = new Thickness(0, 2, 0, 2), CornerRadius = new CornerRadius(6) };
                ToolTipService.SetToolTip(item, rule.name); list.Items.Add(item);
            }
            list.SelectedIndex = index; Load(index); changingSelection = false;
        }
        list.SelectionChanged += (_, _) => {
            if (changingSelection) return;
            try { var next = list.SelectedIndex; Store(); Refresh(next); }
            catch (Exception error) { changingSelection = true; list.SelectedIndex = selected; changingSelection = false; Report(error); }
        };
        receive.Toggled += (_, _) => {
            if (!changingSelection) {
                if (receive.IsOn) response.Text = ""; else delimiter.Text = "";
            }
            UpdateKind(); editingRevision++; output.Text = "";
        };
        foreach (var input in new[] { name, template, display, checksumStart, checksumEnd, delimiter, response, timeout, test })
            input.TextChanged += (_, _) => { editingRevision++; output.Text = ""; };
        var addMenu = new MenuFlyout();
        foreach (var isReceive in new[] { false, true }) {
            var item = new MenuFlyoutItem { Text = isReceive ? "接收规则" : "发送命令", Icon = new SymbolIcon(isReceive ? Symbol.Download : Symbol.Upload) };
            item.Click += (_, _) => { try { Store(); rules.Add(new ProtocolRule { name = (isReceive ? "receive-" : "command-") + Guid.NewGuid().ToString("N")[..8], receive = isReceive }); Refresh(rules.Count - 1); } catch (Exception error) { Report(error); } };
            addMenu.Items.Add(item);
        }
        var add = new Button { Content = "＋ 新增规则", Flyout = addMenu, HorizontalAlignment = HorizontalAlignment.Stretch };
        var emptyAdd = new Button { Content = "＋ 新增规则", Style = Ui.Style("AccentButtonStyle"), Margin = new Thickness(0, 8, 0, 0) };
        emptyAdd.Click += (_, _) => addMenu.ShowAt(emptyAdd);
        empty.Children.Add(emptyAdd);
        delete.Click += (_, _) => { if (selected >= 0) { rules.RemoveAt(selected); Refresh(Math.Min(selected, rules.Count - 1)); } };
        run.Click += async (_, _) => {
            var revision = editingRevision;
            try {
                Store(); run.IsEnabled = false;
                var json = JsonSerializer.Serialize(new { rules, operation = receive.IsOn ? "decode" : "encode", input = test.Text, incoming = receive.IsOn ? test.Text : "" });
                var result = JsonSerializer.Deserialize<JsonElement>(await SerialBackend.Program(json));
                if (revision != editingRevision) return;
                var lines = new List<string>();
                if (result.GetProperty("hex").GetString() is { Length: > 0 } hex) lines.Add(hex);
                foreach (var record in result.GetProperty("records").EnumerateArray())
                    lines.Add($"{record.GetProperty("name").GetString()}: {record.GetProperty("message").GetString()} [{record.GetProperty("hex").GetString()}]");
                if (result.GetProperty("buffer").GetString() is { Length: > 0 } buffer) lines.Add("等待剩余字节：" + buffer);
                output.Text = string.Join("\n", lines); notice.IsOpen = false;
            } catch (Exception error) { if (revision == editingRevision) Report(error); }
            finally { run.IsEnabled = true; }
        };
        var transfer = new TextBox { Header = "JSON 内容", PlaceholderText = "粘贴要导入的 JSON，或点击导出生成当前配置", AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, Height = 120, FontFamily = mono };
        var export = new Button { Content = "导出 JSON" };
        export.Click += (_, _) => { try { Store(); transfer.Text = JsonSerializer.Serialize(rules, new JsonSerializerOptions { WriteIndented = true }); } catch (Exception error) { Report(error); } };
        var import = new Button { Content = "导入并替换编辑列表" };
        import.Click += async (_, _) => {
            try {
                var json = transfer.Text;
                var revision = editingRevision;
                if (json.Length > 1024 * 1024) throw new InvalidOperationException("配置过大");
                import.IsEnabled = false;
                await SerialBackend.Program(JsonSerializer.Serialize(new { rules = JsonSerializer.Deserialize<JsonElement>(json), operation = "validate" }));
                if (revision != editingRevision || transfer.Text != json) throw new InvalidOperationException("导入期间配置已改变，请重新导入。");
                rules = JsonSerializer.Deserialize<List<ProtocolRule>>(json) ?? []; Refresh(rules.Count > 0 ? 0 : -1);
                notice.Severity = InfoBarSeverity.Informational; notice.Message = "已替换编辑列表；保存后生效，取消可放弃。"; notice.IsOpen = true;
            } catch (Exception error) { Report(error); }
            finally { import.IsEnabled = true; }
        };
        var transferButtons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        transferButtons.Children.Add(export); transferButtons.Children.Add(import);
        var transferSection = new Expander { Header = Heading("\uE8B7", "导入与导出"), HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch, Content = Stack(Hint("使用 JSON 备份或共享规则。导入后，点击保存规则生效。"), transfer, transferButtons) };
        var sidebar = new Grid { Padding = new Thickness(12), RowSpacing = 12 };
        sidebar.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto }); sidebar.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) }); sidebar.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        var sidebarHeading = Stack(new TextBlock { Text = "规则库", FontSize = 16, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold }, ruleCount);
        sidebarHeading.Spacing = 4; sidebarHeading.Margin = new Thickness(8, 4, 0, 0); sidebar.Children.Add(sidebarHeading);
        Grid.SetRow(list, 1); sidebar.Children.Add(list);
        var actions = new Grid { ColumnSpacing = 8 };
        actions.ColumnDefinitions.Add(new ColumnDefinition()); actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        actions.Children.Add(add); Grid.SetColumn(delete, 1); actions.Children.Add(delete); Grid.SetRow(actions, 2); sidebar.Children.Add(actions);
        var sidebarCard = new Border { Style = Ui.Style("CardStyle"), CornerRadius = new CornerRadius(8), Child = sidebar };
        var detailStack = Stack(detail, transferSection); detailStack.Spacing = 16;
        var details = new ScrollViewer { Content = detailStack, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Padding = new Thickness(0, 0, 12, 4) };
        var columns = new Grid { ColumnSpacing = 20, RowSpacing = 16 };
        columns.ColumnDefinitions.Add(new ColumnDefinition()); columns.ColumnDefinitions.Add(new ColumnDefinition());
        columns.RowDefinitions.Add(new RowDefinition()); columns.RowDefinitions.Add(new RowDefinition());
        columns.Children.Add(sidebarCard); columns.Children.Add(details);
        var content = new Grid { RowSpacing = 12, MinWidth = 0 };
        content.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) }); content.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        content.Children.Add(columns); Grid.SetRow(notice, 1); content.Children.Add(notice);
        var title = Stack(new TextBlock { Text = "命令与接收规则", FontSize = 20, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold }, Hint("定义发送指令，解析设备反馈")); title.Spacing = 4;
        var contentHost = new ContentControl { Content = content, HorizontalContentAlignment = HorizontalAlignment.Stretch, VerticalContentAlignment = VerticalAlignment.Stretch };
        var dialog = new ContentDialog { XamlRoot = XamlRoot, Title = title, Content = contentHost, PrimaryButtonText = "保存规则", CloseButtonText = "取消", DefaultButton = ContentDialogButton.Primary };
        dialog.Resources["ContentDialogMaxWidth"] = 960d;
        void Resize() {
            var width = Math.Max(0, Math.Min(860, XamlRoot.Size.Width - 80));
            content.Width = width; content.Height = Math.Max(0, Math.Min(680, XamlRoot.Size.Height - 220));
            var narrow = width < 680;
            columns.ColumnDefinitions[0].Width = narrow ? new GridLength(1, GridUnitType.Star) : new GridLength(224);
            columns.ColumnDefinitions[1].Width = narrow ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
            columns.RowDefinitions[0].Height = narrow ? new GridLength(Math.Min(200, content.Height * 0.4)) : new GridLength(1, GridUnitType.Star);
            columns.RowDefinitions[1].Height = narrow ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
            Grid.SetColumn(details, narrow ? 0 : 1); Grid.SetRow(details, narrow ? 1 : 0);
            transferButtons.Orientation = narrow ? Orientation.Vertical : Orientation.Horizontal;
            var editorWidth = narrow ? width - 12 : width - 256;
            foreach (var pair in fieldPairs) {
                var stacked = editorWidth < 460;
                pair.ColumnDefinitions[1].Width = stacked ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
                var second = (FrameworkElement)pair.Children[1];
                Grid.SetColumn(second, stacked ? 0 : 1); Grid.SetRow(second, stacked ? 1 : 0);
            }
        }
        void RootChanged(XamlRoot root, XamlRootChangedEventArgs change) => Resize();
        dialog.PrimaryButtonClick += async (_, e) => {
            var deferral = e.GetDeferral(); contentHost.IsEnabled = false; dialog.IsPrimaryButtonEnabled = false; dialog.IsSecondaryButtonEnabled = false;
            try {
                Store(); var json = JsonSerializer.Serialize(rules);
                await SerialBackend.Program(JsonSerializer.Serialize(new { rules, operation = "validate" }));
                var old = preferences.ProtocolRules; preferences.ProtocolRules = json;
                try { preferences.Save(); } catch { preferences.ProtocolRules = old; throw; }
                protocolRevision++; ResetProtocol(); RefreshProtocolDisplay(); await PreviewCommand();
            } catch (Exception error) { e.Cancel = true; Report(error); }
            finally { contentHost.IsEnabled = true; dialog.IsPrimaryButtonEnabled = true; dialog.IsSecondaryButtonEnabled = true; deferral.Complete(); }
        };
        Refresh(rules.Count > 0 ? 0 : -1); Resize();
        XamlRoot.Changed += RootChanged;
        try { await dialog.Show(this); }
        finally { XamlRoot.Changed -= RootChanged; }
    }
    private sealed class ProtocolRule {
        public string name { get; set; } = "command";
        public string template { get; set; } = "AA {value:u16be} {checksum:sum8}";
        public string display { get; set; } = "";
        public bool receive { get; set; }
        public int checksum_start { get; set; }
        public int? checksum_end { get; set; }
        public string delimiter { get; set; } = "";
        public string response { get; set; } = "";
        public int timeout_ms { get; set; } = 1000;
    }
    private readonly ObservableCollection<TerminalFrame> frames = [];
    private readonly ObservableCollection<TerminalFrame> visibleFrames = [];
    private readonly DispatcherQueueTimer timer;
    private readonly SerialAutoSender autoSender = new();
    private SerialStatus status = new() { connected = false, path = null, config = null, rx_bytes = 0, tx_bytes = 0, opened_at = null, last_error = null };
    private bool busy, sending, polling, refreshing, exporting, updating = true, started, closed;
    private int ticks, retainedBytes, connectionRevision;
    private double composerHeight = 280;
    private static readonly string[] Formats = ["hex", "ascii", "gbk", "utf-8"];
    private static readonly string[] Endings = ["none", "lf", "cr", "crlf"];
    private readonly SerialPreferences preferences = SerialPreferences.Current;
    private readonly Windows.UI.ViewManagement.UISettings uiSettings = new();
    private bool resizingLayout;
    private bool? compactLayout, connectionOverlay;

    public SerialView()
    {
        InitializeComponent();
        Terminal.ItemsSource = visibleFrames;
        var terminalMenu = new MenuFlyout();
        var copySelected = Ui.MenuItem("复制所选记录", Glyphs.Copy, () => CopyFrames(selectedOnly: true), "Ctrl+C");
        var copyAll = Ui.MenuItem("复制全部显示记录", Glyphs.Copy, () => CopyFrames(selectedOnly: false));
        terminalMenu.Items.Add(copySelected);
        terminalMenu.Items.Add(copyAll);
        terminalMenu.Opening += (_, _) =>
        {
            copySelected.IsEnabled = Terminal.SelectedItems.Count > 0;
            copyAll.IsEnabled = visibleFrames.Count > 0;
        };
        Terminal.ContextFlyout = terminalMenu;
        Terminal.KeyboardAccelerators.Add(Ui.Accelerator(VirtualKey.C, VirtualKeyModifiers.Control, () =>
        {
            // Preserve the TextBlock's native copy when the user selected characters.
            var focused = FocusManager.GetFocusedElement(XamlRoot) as TextBlock;
            if (!string.IsNullOrEmpty(focused?.SelectedText)) focused.CopySelectionToClipboard();
            else CopyFrames(selectedOnly: true);
        }));
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
        uiSettings.TextScaleFactorChanged += OnTextScaleChanged;
        timer = DispatcherQueue.CreateTimer(); timer.Interval = TimeSpan.FromMilliseconds(100); timer.Tick += OnTick;
        autoSender.Changed += RefreshState;
        autoSender.Failed += error => { if (!closed) ShowError(error.Message); };
        BaudPicker.RegisterPropertyChangedCallback(ComboBox.TextProperty, (_, _) => { if (!updating) RefreshState(); });
        updating = false; RefreshProtocolDisplay(); RefreshState();
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
                if (!status.connected) { autoSender.Stop(); ResetProtocol(); }
            }
            if (batch.dropped_frames > 0) protocolBuffer = "";
            foreach (var frame in batch.frames) {
                var item = new TerminalFrame(frame, preferences.DisplayFormat); frames.Add(item); retainedBytes += item.MemoryCost;
            }
            var configRevision = protocolRevision;
            foreach (var frame in batch.frames)
            {
                if (busy || revision != connectionRevision || configRevision != protocolRevision) break;
                try {
                if (frame.direction.IsReceive && JsonSerializer.Deserialize<JsonElement>(preferences.ProtocolRules).EnumerateArray().Any(r => r.TryGetProperty("receive", out var rx) && rx.GetBoolean())) {
                    var decoded = await Protocol("decode", incoming: frame.hex);
                    if (busy || revision != connectionRevision || configRevision != protocolRevision) break;
                    protocolBuffer = decoded.GetProperty("buffer").GetString() ?? "";
                    foreach (var record in decoded.GetProperty("records").EnumerateArray()) {
                        var name = record.GetProperty("name").GetString();
                        protocolRecords.Add($"{frame.timestamp} RX {name}: {record.GetProperty("message").GetString()} [{record.GetProperty("hex").GetString()}]");
                        if (responseDeadline is { } deadline && responseStartedAt is { } began && DateTimeOffset.TryParse(frame.timestamp, out var received) && received >= began && received <= deadline && name == pendingResponse && record.GetProperty("valid").GetBoolean()) {
                            CommandPreview.Text = "收到匹配反馈：" + name + "（未验证请求序号）"; pendingResponse = ""; responseDeadline = null;
                        }
                    }
                    if (protocolRecords.Count > 2000) protocolRecords.RemoveRange(0, protocolRecords.Count-2000);
                }
                } catch (Exception error) {
                    if (revision == connectionRevision && configRevision == protocolRevision) { protocolBuffer = ""; ShowError(error.Message); }
                    break;
                }
            }
            while (frames.Count > 2000 || retainedBytes > 8 * 1024 * 1024) { retainedBytes -= frames[0].MemoryCost; frames.RemoveAt(0); }
            if (batch.dropped_frames > 0) ShowError($"显示缓冲已丢弃 {batch.dropped_frames} 条旧记录；字节统计保持完整。");
            if (batch.frames.Count > 0) RefreshTerminal();
            if (responseDeadline is { } expired && DateTimeOffset.UtcNow > expired) { CommandPreview.Text = "等待反馈超时：" + pendingResponse; responseDeadline = null; pendingResponse = ""; }
            if (batch.frames.Count > 0) ParsedTerminal.ItemsSource = protocolRecords.ToArray();
            if (!status.connected) ResetProtocol();
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
        busy = true; connectionRevision++; ResetProtocol(); RefreshState();
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
            if (IsCommand) {
                var command = InputBox.Text; var configRevision = protocolRevision; var result = await Protocol("encode", command);
                if (revision != connectionRevision || configRevision != protocolRevision || busy || !status.connected) return false;
                var hex = result.GetProperty("hex").GetString() ?? "";
                responseStartedAt = DateTimeOffset.FromUnixTimeMilliseconds(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
                pendingResponse = result.GetProperty("response").GetString() ?? "";
                responseDeadline = pendingResponse.Length == 0 ? null : DateTimeOffset.UtcNow.AddMilliseconds(result.GetProperty("timeout_ms").GetInt32());
                CommandPreview.Text = pendingResponse.Length == 0 ? hex : "等待反馈：" + pendingResponse;
                await SerialBackend.Send(hex, Core.SendFormat.Hex, Core.LineEnding.None, ChecksumAlgorithm.None);
                commandHistory.Add(command); if (commandHistory.Count > 100) commandHistory.RemoveAt(0); historyIndex = commandHistory.Count;
                protocolRecords.Add(DateTimeOffset.Now.ToString("HH:mm:ss.fff") + " TX " + command + " [" + hex + "]");
                if (protocolRecords.Count > 2000) protocolRecords.RemoveAt(0);
            } else await SerialBackend.Send(InputBox.Text, format, ending, checksum);
            return !closed && status.connected && !busy && revision == connectionRevision;
        }
        catch (Exception error) { ResetProtocol(); if (!closed && revision == connectionRevision) ShowError(error.Message); return false; }
        finally { sending = false; if (!closed) RefreshState(); }
    }
    private void OnAutoSend(object sender, RoutedEventArgs args)
    {
        if (IsCommand) return;
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
        CommandMode.IsEnabled = !autoSender.IsRunning && !sending;

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
        SendFormatPicker.IsEnabled = ChecksumPicker.IsEnabled = !automatic && !IsCommand;
        InputBox.IsReadOnly = automatic;
        AutoSendInterval.IsEnabled = !automatic;
        AutoSendButton.IsEnabled = automatic || (!IsCommand && CanSend && TryGetAutoSendInterval(out _));
        AutoSendLabel.Text = automatic ? "停止自动发送" : "自动发送";
        AutoSendIcon.Glyph = automatic ? "\uE71A" : "\uE8EE";
        InputCount.Text = InputBox.Text.Length == 0 ? "" : $"{new System.Globalization.StringInfo(InputBox.Text).LengthInTextElements:N0} 字符";
        EndingPicker.IsEnabled = !IsCommand && !automatic && SendFormatPicker.SelectedIndex != 0;
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
        ExportButton.IsEnabled = frames.Count > 0 && !exporting && ExportRequested is not null;
        var parity = preferences.Parity switch { 1 => "O", 2 => "E", _ => "N" };
        Ui.SetLiveText(ConnectionStatus, busy ? "正在处理…" : status.connected
            ? $"已连接 · {status.path} · {preferences.BaudRate} · {preferences.DataBits}{parity}{preferences.StopBits}" : "未连接");
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
    private async void OnInputChanged(object sender, TextChangedEventArgs args) { if (updating) return; await PreviewCommand(); RefreshState(); }
    private void OnClear(object sender, RoutedEventArgs args) { protocolRecords.Clear(); ParsedTerminal.ItemsSource = null; frames.Clear(); visibleFrames.Clear(); retainedBytes = 0; RefreshState(); }
    private async void OnExport(object sender, RoutedEventArgs args)
    {
        if (frames.Count == 0 || exporting || ExportRequested is not { } export) return;
        exporting = true; RefreshState();
        try
        {
            var data = JsonSerializer.SerializeToUtf8Bytes(frames.Select(frame => frame.ExportRecord), new JsonSerializerOptions { WriteIndented = true });
            await export(data);
        }
        catch (Exception error) { ShowError($"导出串口记录失败：{error.Message}"); }
        finally { exporting = false; RefreshState(); }
    }
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
        SerialSurface.IsPaneOpen = expanded;
        ToolTipService.SetToolTip(ConnectionPaneToggle, expanded ? "收起连接区" : "展开连接区");
    }
    private void OnConnectionPaneClosed(SplitView sender, object args)
    {
        ConnectionPaneToggle.IsChecked = false;
        ToolTipService.SetToolTip(ConnectionPaneToggle, "展开连接区");
    }
    private void OnToggleComposerPane(object sender, RoutedEventArgs args)
    {
        var expanded = ComposerPaneToggle.IsChecked == true;
        ComposerPanel.Visibility = ComposerSplitter.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
        ComposerRow.MinHeight = expanded ? 232 : 0;
        ComposerSplitterRow.Height = new GridLength(expanded ? 5 : 0);
        if (expanded) SetComposerHeight(composerHeight);
        else ComposerRow.Height = new GridLength(0);
        ComposerPaneChevron.Glyph = expanded ? Glyphs.ChevronDown : "\uE70E";
        ComposerPaneLabel.Text = expanded ? "收起发送区" : "展开发送区";
        Ui.Name(ComposerPaneToggle, ComposerPaneLabel.Text);
        ToolTipService.SetToolTip(ComposerPaneToggle, ComposerPaneLabel.Text);
        UpdateResponsiveLayout();
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
        var inputHeight = 120 * uiSettings.TextScaleFactor;
        InputBox.MinHeight = inputHeight;
        var previewHeight = CommandPreviewViewport.Visibility == Visibility.Visible ? CommandPreviewViewport.ActualHeight : 0;
        var minimum = Math.Max(232, SendOptions.ActualHeight + ComposerActions.ActualHeight + inputHeight + previewHeight);
        var maximum = Math.Max(minimum, Math.Min(480 * uiSettings.TextScaleFactor,
            Workspace.ActualHeight - ErrorBar.ActualHeight - ReceiveToolbar.ActualHeight - ReceiveOptions.ActualHeight - 120));
        ComposerRow.MinHeight = minimum;
        composerHeight = Math.Clamp(height, minimum, maximum);
        ComposerRow.Height = new GridLength(composerHeight);
    }
    private void OnRootSizeChanged(object sender, SizeChangedEventArgs args) => UpdateResponsiveLayout();
    private void OnTextScaleChanged(Windows.UI.ViewManagement.UISettings sender, object args)
        => DispatcherQueue.TryEnqueue(UpdateResponsiveLayout);

    private void UpdateResponsiveLayout()
    {
        if (resizingLayout || SerialViewport is null || SerialSurface is null || ComposerActions is null || Workspace.ActualWidth <= 0) return;
        resizingLayout = true;
        try
        {
            var textScale = uiSettings.TextScaleFactor;
            var overlay = Root.ActualWidth < 1000 * textScale;
            SerialSurface.OpenPaneLength = Math.Min(Root.ActualWidth, Math.Max(216, 216 * textScale));
            if (connectionOverlay != overlay)
            {
                connectionOverlay = overlay;
                SerialSurface.DisplayMode = overlay ? SplitViewDisplayMode.Overlay : SplitViewDisplayMode.Inline;
                SerialSurface.IsPaneOpen = !overlay;
                ConnectionPaneToggle.IsChecked = !overlay;
                ToolTipService.SetToolTip(ConnectionPaneToggle, overlay ? "展开连接区" : "收起连接区");
            }
            var compact = Workspace.ActualWidth < 850 * textScale;
            if (compactLayout != compact)
            {
                compactLayout = compact;
                Grid.SetRow(DirectionPicker, compact ? 1 : 0);
                Grid.SetColumn(DirectionPicker, compact ? 0 : 4);
                Grid.SetColumnSpan(DirectionPicker, compact ? 3 : 1);
                Grid.SetRow(TerminalSearch, compact ? 1 : 0);
                Grid.SetColumn(TerminalSearch, compact ? 3 : 5);
                Grid.SetColumnSpan(TerminalSearch, compact ? 3 : 1);
                DirectionPicker.Width = compact ? double.NaN : 112;
                DirectionPicker.HorizontalAlignment = HorizontalAlignment.Stretch;

                Grid.SetRow(AutoScrollToggle, compact ? 1 : 0);
                Grid.SetColumn(AutoScrollToggle, compact ? 0 : 3);
                Grid.SetColumnSpan(AutoScrollToggle, compact ? 2 : 1);
                Grid.SetRow(ExportButton, compact ? 1 : 0);
                Grid.SetRow(ClearButton, compact ? 1 : 0);
                Grid.SetRow(ComposerPaneToggle, compact ? 1 : 0);

                Grid.SetColumnSpan(SendHeading, compact ? 6 : 1);
                Ui.SetVisible(SendEncodingLabel, !compact);
                Ui.SetVisible(SendEndingLabel, !compact);
                for (var index = 0; index < SendOptions.ColumnDefinitions.Count; index++)
                    SendOptions.ColumnDefinitions[index].Width = compact || index == 0 ? new GridLength(1, GridUnitType.Star) : GridLength.Auto;
                SetPickerLayout(ChecksumPicker, compact ? 1 : 0, compact ? 0 : 1, compact ? 6 : 1, compact ? "校验算法" : null, compact ? double.NaN : 156);
                SetPickerLayout(SendFormatPicker, compact ? 2 : 0, compact ? 0 : 3, compact ? 3 : 1, compact ? "编码" : null, compact ? double.NaN : 96);
                SetPickerLayout(EndingPicker, compact ? 2 : 0, compact ? 3 : 5, compact ? 3 : 1, compact ? "行尾" : null, compact ? double.NaN : 80);

                Grid.SetColumnSpan(IntervalOptions, compact ? 3 : 1);
                Grid.SetRow(AutoSendButton, compact ? 1 : 0);
                Grid.SetColumn(AutoSendButton, compact ? 0 : 1);
                Grid.SetColumnSpan(AutoSendButton, compact ? 3 : 1);
                AutoSendButton.HorizontalAlignment = HorizontalAlignment.Left;
                Grid.SetColumnSpan(InputCount, compact ? 2 : 1);
                InputCount.HorizontalAlignment = HorizontalAlignment.Right;
                Grid.SetRow(SendButton, compact ? 1 : 0);
                Grid.SetColumn(SendButton, compact ? 3 : 4);
                Grid.SetColumnSpan(SendButton, compact ? 2 : 1);
                SendButton.HorizontalAlignment = HorizontalAlignment.Right;
            }
            var expanded = ComposerPaneToggle.IsChecked == true;
            if (expanded) SetComposerHeight(composerHeight);
            var minimumSurface = expanded
                ? Math.Max(520 * textScale, composerHeight + ReceiveToolbar.ActualHeight + ReceiveOptions.ActualHeight + 125 * textScale)
                : 300 * textScale;
            SerialSurface.Height = Math.Max(SerialViewport.ActualHeight, minimumSurface + ErrorBar.ActualHeight);
        }
        finally { resizingLayout = false; }
    }

    private static void SetPickerLayout(ComboBox picker, int row, int column, int span, string? header, double width)
    {
        Grid.SetRow(picker, row);
        Grid.SetColumn(picker, column);
        Grid.SetColumnSpan(picker, span);
        picker.Header = header;
        picker.Width = width;
        picker.HorizontalAlignment = HorizontalAlignment.Stretch;
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
    private bool CanTransmit => !closed && status.connected && !sending && !busy && InputBox.Text.Length > 0 && (!IsCommand || commandHex.Length > 0);
    internal bool CanSend => CanTransmit && !autoSender.IsRunning;
    internal bool UpdateBlocked => status.connected || busy || sending || autoSender.IsRunning;
    private bool lastUpdateBlocked, lastCanSend, lastCanToggle, lastRefreshing;
    internal event Action? ActivityChanged;
    internal event Func<byte[], Task>? ExportRequested;
    internal void ClearTerminal() => OnClear(this, new());
    internal void Find() { TerminalSearch.Focus(FocusState.Programmatic); TerminalSearch.SelectAll(); }
    private void CopyFrames(bool selectedOnly)
    {
        var selected = Terminal.SelectedItems.OfType<TerminalFrame>().ToHashSet();
        var records = visibleFrames.Where(frame => !selectedOnly || selected.Contains(frame));
        var text = string.Join(Environment.NewLine, records.Select(frame => $"{frame.Time}\t{frame.Direction}\t{frame.Size} B\t{frame.DisplayText}"));
        if (text.Length == 0) return;
        var package = new DataPackage();
        package.SetText(text);
        Clipboard.SetContent(package);
    }
    internal void Shutdown()
    {
        closed = true;
        autoSender.Stop();
        timer.Stop();
        uiSettings.TextScaleFactorChanged -= OnTextScaleChanged;
        try { SerialBackend.Shutdown(); }
        catch (Exception error) { System.Diagnostics.Trace.TraceError("{0}", error); }
    }
    private sealed record PortChoice(string Path, string DisplayName, PortInfo? Info);
    private static bool SamePort(PortChoice left, PortChoice right) => left.Path == right.Path && left.DisplayName == right.DisplayName
        && left.Info?.manufacturer == right.Info?.manufacturer && left.Info?.serial_number == right.Info?.serial_number
        && left.Info?.vid == right.Info?.vid && left.Info?.pid == right.Info?.pid;
}
