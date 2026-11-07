using System.Text.Json;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChipDock;

public sealed partial class SerialView
{
    private async Task<JsonElement> Workbench(object action) =>
        JsonSerializer.Deserialize<JsonElement>(await SerialBackend.Workbench(SessionId, JsonSerializer.Serialize(action)));

    private Task<JsonElement> ApplyProtocol() =>
        Workbench(new { action = "protocol", rules = JsonSerializer.Deserialize<JsonElement>(preferences.ProtocolRules) });

    private async Task ApplyFraming(string mode, string value)
    {
        var config = new Dictionary<string, object> { ["mode"] = mode };
        if (mode is "idle" or "fixed")
        {
            if (!int.TryParse(value, out var count) || count <= 0) throw new InvalidOperationException("请输入正整数");
            config[mode == "idle" ? "ms" : "bytes"] = count;
        }
        if (mode is "header" or "tail") config["hex"] = value;
        await Workbench(new { action = "framing", config });
        preferences.FramingMode = mode;
        preferences.FramingValue = value;
        preferences.Save();
    }

    private void OnSerialTools(object sender, RoutedEventArgs args)
    {
        var menu = new MenuFlyout();
        foreach (var (title, work) in new (string, Func<Task>)[]
        {
            ("发送校验", ShowChecksum), ("接收分帧", ShowFraming), ("Modbus RTU", ShowModbus), ("持续采集", ShowCapture),
            ("常用包库", ShowPackets), ("发送文件", ShowSendFile), ("宏命令", ShowMacro), ("自动应答", ShowReplies),
            ("打开实时曲线", () => { OpenPlot?.Invoke(SessionId); return Task.CompletedTask; })
        })
        {
            var item = new MenuFlyoutItem { Text = title };
            item.Click += async (_, _) => { try { await work(); } catch (Exception error) { ShowError(error.Message); } };
            menu.Items.Add(item);
        }
        menu.ShowAt((FrameworkElement)sender);
    }

    private Task ShowChecksum()
    {
        var input = Field("试算内容", InputBox.Text);
        var format = Choice("编码", Formats, SendFormatPicker.SelectedIndex);
        var algorithms = new[] { "sum8", "oddParity", "evenParity", "xor8", "crc16Modbus", "crc16Ccitt", "crc32", "lrc" };
        var checksum = Choice("算法", algorithms, Math.Clamp(preferences.Checksum - 1, 0, algorithms.Length - 1));
        var prefix = Field("跳过开头字节", preferences.SkipPrefix.ToString());
        var suffix = Field("跳过结尾字节", preferences.SkipSuffix.ToString());
        var hexEnding = new CheckBox { Content = "HEX 发送时允许附加行尾", IsChecked = preferences.HexLineEnding };
        return ToolDialog("发送校验", [input, format, checksum, prefix, suffix, hexEnding], "计算", async _ =>
        {
            var reply = await Workbench(new { action = "checksum", input = input.Text, format = format.SelectedItem,
                algorithm = checksum.SelectedItem, options = new { skipPrefix = uint.Parse(prefix.Text), skipSuffix = uint.Parse(suffix.Text) } });
            return reply.GetProperty("hex").GetString() ?? "";
        }, "应用设置", _ =>
        {
            var start = uint.Parse(prefix.Text); var end = uint.Parse(suffix.Text);
            preferences.SkipPrefix = start; preferences.SkipSuffix = end;
            preferences.HexLineEnding = hexEnding.IsChecked == true;
            preferences.Save(); RefreshState();
            return Task.FromResult("已应用");
        });
    }

    private Task ShowFraming()
    {
        var modes = new[] { "read", "idle", "header", "tail", "fixed" };
        var mode = Choice("分帧方式", ["读取块", "静默间隔", "帧头", "帧尾", "固定长度"], IndexOrDefault(modes, preferences.FramingMode, 0));
        var value = Field("间隔 ms／标记 HEX／固定字节数", preferences.FramingValue);
        return ToolDialog("接收分帧", [mode, value], "应用", async _ =>
        {
            await ApplyFraming(modes[mode.SelectedIndex], value.Text);
            return "已应用分帧设置";
        });
    }

    private Task ShowModbus()
    {
        var codes = new[] { 1, 2, 3, 4, 5, 6, 15, 16 };
        var station = Field("站号", "1");
        var function = Choice("功能码", codes.Select(value => value.ToString("X2")).ToArray(), 2);
        var address = Field("起始地址", "0");
        var quantity = Field("读取数量", "1");
        var values = Field("写入值（十进制，以逗号分隔）");
        var timeout = Field("超时 (ms)", "1000");
        Task<string> Execute(bool send) => RunModbus(send);
        async Task<string> RunModbus(bool send)
        {
            var code = codes[function.SelectedIndex];
            var timeoutMs = send ? int.Parse(timeout.Text) : 0;
            if (send && timeoutMs is < 10 or > 60000)
                throw new InvalidOperationException("Modbus 超时须为 10–60000 毫秒");
            var response = await Workbench(new { action = "modbus", timeout_ms = timeoutMs,
                request = new { station = int.Parse(station.Text), function = code, address = int.Parse(address.Text),
                    quantity = code <= 4 ? int.Parse(quantity.Text) : 0,
                    values = code <= 4 ? [] : values.Text.Split(',', StringSplitOptions.TrimEntries).Select(text => ushort.Parse(text)).ToArray() } });
            return response.ToString();
        }
        return ToolDialog("Modbus RTU", [station, function, address, quantity, values, timeout],
            "生成请求", _ => Execute(false), status.connected ? "发送并等待" : null, _ => Execute(true));
    }

    private static TextBox Field(string title, string text = "") => new() { Header = title, Text = text };
    private static ComboBox Choice(string title, string[] values, int index = 0) =>
        new() { Header = title, ItemsSource = values, SelectedIndex = index, HorizontalAlignment = HorizontalAlignment.Stretch };

    private async Task ToolDialog(string title, UIElement[] fields, string primary,
        Func<ContentDialog, Task<string>> execute, string? secondary = null,
        Func<ContentDialog, Task<string>>? other = null, bool trackJob = false)
    {
        var result = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        var panel = new StackPanel { Spacing = 8, MinWidth = 320 };
        foreach (var field in fields) panel.Children.Add(field);
        panel.Children.Add(result);
        var dialog = new ContentDialog { Title = title, Content = new ScrollViewer { Content = panel, MaxHeight = 560 },
            PrimaryButtonText = primary, SecondaryButtonText = secondary ?? "", CloseButtonText = "关闭" };
        async Task Invoke(ContentDialogButtonClickEventArgs args, Func<ContentDialog, Task<string>> work)
        {
            args.Cancel = true;
            var deferral = args.GetDeferral();
            dialog.IsPrimaryButtonEnabled = dialog.IsSecondaryButtonEnabled = false;
            try { result.Text = await work(dialog); }
            catch (Exception error) { result.Text = error.Message; }
            finally { dialog.IsPrimaryButtonEnabled = dialog.IsSecondaryButtonEnabled = true; deferral.Complete(); }
        }
        dialog.PrimaryButtonClick += async (_, args) => await Invoke(args, execute);
        if (other is not null) dialog.SecondaryButtonClick += async (_, args) => await Invoke(args, other);
        var progress = new TextBlock { TextWrapping = TextWrapping.Wrap };
        void RefreshProgress(Microsoft.UI.Dispatching.DispatcherQueueTimer _, object args)
        {
            progress.Text = status.job_total > 0
                ? $"已完成 {status.job_done}/{status.job_total} · {status.job_error ?? (status.job_kind is null ? "完成" : "发送中")}"
                : "";
        }
        if (trackJob)
        {
            panel.Children.Add(progress);
            timer.Tick += RefreshProgress;
        }
        try { await dialog.Show(this); }
        finally { if (trackJob) timer.Tick -= RefreshProgress; }
    }
}

public sealed partial class SerialView
{
    internal Func<string, string, Task<string?>>? PickExport { get; set; }
    internal Func<Task<string?>>? PickFolder { get; set; }
    internal Func<Task<string?>>? PickFile { get; set; }
    private bool searchRegex, searchCase;
    private static readonly string[] Palette = ["default", "red", "orange", "green", "blue", "purple"];

    private void OnViewOptions(object sender, RoutedEventArgs args)
    {
        var menu = new MenuFlyout();
        void Action(string title, Action work)
        {
            var item = new MenuFlyoutItem { Text = title };
            item.Click += (_, _) => work(); menu.Items.Add(item);
        }
        void Toggle(string title, bool value, Action<bool> work)
        {
            var item = new ToggleMenuFlyoutItem { Text = title, IsChecked = value };
            item.Click += (_, _) => { work(item.IsChecked); RefreshTerminal(); RefreshState(); };
            menu.Items.Add(item);
        }
        Action(pausedFrames is null ? "暂停显示" : "继续显示", () =>
        {
            pausedFrames = pausedFrames is null ? frames.ToList() : null;
            if (pausedFrames is null) ParsedTerminal.ItemsSource = protocolRecords.ToArray();
            RefreshTerminal(); RefreshState();
        });
        Toggle("文本与 HEX 混显", preferences.Mixed, value => { preferences.Mixed = value; SaveDisplay(); });
        Toggle("显示时间戳", preferences.Timestamps, value => { preferences.Timestamps = value; SaveDisplay(); });
        Toggle("区分大小写", searchCase, value => searchCase = value);
        Toggle("正则搜索", searchRegex, value => searchRegex = value);
        Action("上一条命中", () => Navigate(-1));
        Action("下一条命中", () => Navigate(1));
        menu.Items.Add(new MenuFlyoutSeparator());
        Action("着色规则…", () => _ = ShowColors());
        Action("记录表…", () => _ = ShowRecords());
        Action("重置计数", () => _ = ResetCounters());
        Toggle("清屏时重置计数", preferences.ResetOnClear, value => { preferences.ResetOnClear = value; SaveDisplay(); });
        menu.ShowAt((FrameworkElement)sender);
    }

    private void SaveDisplay()
    {
        try { preferences.Save(); }
        catch (Exception error) { ShowError(error.Message); }
    }

    private async Task ResetCounters()
    {
        try { await Workbench(new { action = "resetCounters" }); }
        catch (Exception error) { ShowError(error.Message); }
    }

    private static Func<string, bool>? BuildMatcher(string pattern, bool regex, bool caseSensitive)
    {
        if (!regex) return text => text.Contains(pattern, caseSensitive ? StringComparison.Ordinal : StringComparison.OrdinalIgnoreCase);
        if (pattern.Length > 256) return null;
        try
        {
            var options = System.Text.RegularExpressions.RegexOptions.CultureInvariant;
            if (!caseSensitive) options |= System.Text.RegularExpressions.RegexOptions.IgnoreCase;
            var compiled = new System.Text.RegularExpressions.Regex(pattern, options, TimeSpan.FromMilliseconds(8));
            return text =>
            {
                try { return compiled.IsMatch(text); }
                catch (System.Text.RegularExpressions.RegexMatchTimeoutException) { return false; }
            };
        }
        catch (ArgumentException) { return null; }
    }

    private static Microsoft.UI.Xaml.Media.Brush? ColorFor(string label) => label switch
    {
        "red" => Ui.Brush(Microsoft.UI.Colors.IndianRed),
        "orange" => Ui.Brush(Microsoft.UI.Colors.DarkOrange),
        "green" => Ui.Brush(Microsoft.UI.Colors.SeaGreen),
        "blue" => Ui.Brush(Microsoft.UI.Colors.DodgerBlue),
        "purple" => Ui.Brush(Microsoft.UI.Colors.MediumPurple),
        _ => null
    };

    private List<(Func<string, bool> Match, Microsoft.UI.Xaml.Media.Brush? Brush)> ColorRules(string? source = null)
    {
        var result = new List<(Func<string, bool>, Microsoft.UI.Xaml.Media.Brush?)>();
        foreach (var line in (source ?? preferences.ColorRules).Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            if (result.Count >= 32) throw new InvalidOperationException("最多 32 条着色规则");
            var fields = line.Split(' ', 3, StringSplitOptions.RemoveEmptyEntries);
            if (fields.Length != 3 || !Palette.Contains(fields[0]) || fields[1] is not ("regex" or "keyword") ||
                fields[2].Length > 256 || BuildMatcher(fields[2], fields[1] == "regex", false) is not { } match)
                throw new InvalidOperationException("每行使用「颜色 keyword 或 regex 匹配内容」格式");
            result.Add((match, ColorFor(fields[0])));
        }
        return result;
    }

    private Task ShowColors()
    {
        var presets = Choice("预设", ["自定义", "ESP-IDF", "日志等级", "AT 响应"]);
        var rules = Field("按优先级逐行填写，例 orange keyword WARN", preferences.ColorRules);
        rules.AcceptsReturn = true; rules.MinHeight = 120; rules.TextWrapping = TextWrapping.Wrap;
        var tx = Choice("TX 颜色", Palette, IndexOrDefault(Palette, preferences.TxColor, 0));
        presets.SelectionChanged += (_, _) =>
        {
            rules.Text = presets.SelectedIndex switch
            {
                1 => "orange regex ^[EW] \\(\nblue regex ^[IDV] \\(",
                2 => "red regex ERROR|FATAL\norange keyword WARN\nblue keyword INFO",
                3 => "red keyword ERROR\ngreen keyword OK",
                _ => preferences.ColorRules
            };
        };
        return ToolDialog("终端着色", [presets, rules, tx], "应用", dialog =>
        {
            _ = ColorRules(rules.Text);
            preferences.ColorRules = rules.Text; preferences.TxColor = (string)tx.SelectedItem;
            SaveDisplay(); RefreshTerminal();
            return Task.FromResult("已应用");
        });
    }

    private void Navigate(int step)
    {
        if (TerminalSearch.Text.Length == 0 || visibleFrames.Count == 0) return;
        var selected = Terminal.SelectedItem is TerminalFrame frame ? visibleFrames.IndexOf(frame) : step > 0 ? -1 : 0;
        ScrollTo(visibleFrames[(selected + step + visibleFrames.Count) % visibleFrames.Count]);
    }

    private void ScrollTo(TerminalFrame frame)
    {
        preferences.AutoScroll = false; AutoScrollToggle.IsChecked = false;
        Terminal.SelectedItem = frame;
        Terminal.ScrollIntoView(frame);
    }

    private async Task ShowRecords()
    {
        var direction = Choice("方向", ["全部", "RX", "TX"]);
        var sort = Choice("排序", ["序号", "时间", "方向", "来源", "长度", "HEX", "文本", "校验"]);
        var descending = new CheckBox { Content = "降序" };
        var filter = Field("筛选 HEX、文本或来源");
        var header = new Grid { ColumnSpacing = 8, Padding = new Thickness(12, 4, 12, 4) };
        var columns = new (string, double)[] { ("序号", 54), ("时间", 92), ("方向", 38), ("来源", 72),
            ("长度", 54), ("HEX", 132), ("文本", 140), ("校验", 50) };
        foreach (var (title, width) in columns)
        {
            var label = new TextBlock { Text = title, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold };
            Grid.SetColumn(label, header.ColumnDefinitions.Count);
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(width) });
            header.Children.Add(label);
        }
        var list = new ListView { Height = 290, Header = header, ItemTemplate = (DataTemplate)Resources["SerialAnalysisRow"] };
        var copy = new Button { Content = "复制所选记录" };
        var snapshot = new List<TerminalFrame>(frames);
        string Describe(TerminalFrame item) =>
            $"#{item.Record.sequence} · {item.Time} · {item.Direction} · {item.Source} · {item.Size} B · 校验 {item.ChecksumText}\n{item.Hex}\n{item.PlainText}";
        void Refresh()
        {
            var visible = snapshot.Where(item => (direction.SelectedIndex == 0 || item.IsReceive == (direction.SelectedIndex == 1))
                && Describe(item).Contains(filter.Text, StringComparison.OrdinalIgnoreCase));
            var ordered = sort.SelectedIndex switch
            {
                1 => visible.OrderBy(item => item.Record.timestamp),
                2 => visible.OrderBy(item => item.Direction),
                3 => visible.OrderBy(item => item.Record.source),
                4 => visible.OrderBy(item => item.Record.bytes.Count),
                5 => visible.OrderBy(item => item.Record.hex),
                6 => visible.OrderBy(item => item.Record.text.utf8),
                7 => visible.OrderBy(item => item.ChecksumText),
                _ => visible.OrderBy(item => item.Record.sequence)
            };
            list.ItemsSource = (descending.IsChecked == true ? ordered.Reverse() : ordered).ToArray();
        }
        copy.Click += (_, _) =>
        {
            if (list.SelectedItem is TerminalFrame item)
            {
                var package = new Windows.ApplicationModel.DataTransfer.DataPackage();
                package.SetText(Describe(item));
                Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
            }
        };
        direction.SelectionChanged += (_, _) => Refresh();
        sort.SelectionChanged += (_, _) => Refresh();
        descending.Click += (_, _) => Refresh();
        filter.TextChanged += (_, _) => Refresh();
        Refresh();
        var content = new StackPanel { Width = Math.Min(770, XamlRoot.Size.Width - 72), Spacing = 8 };
        foreach (var field in new UIElement[] { direction, sort, descending, filter, list, copy }) content.Children.Add(field);
        var dialog = new ContentDialog { Title = "串口记录表", Content = content, PrimaryButtonText = "在终端定位", CloseButtonText = "关闭" };
        dialog.Resources["ContentDialogMaxWidth"] = 820d;
        if (await dialog.Show(this) == ContentDialogResult.Primary && list.SelectedItem is TerminalFrame record)
        {
            var target = frames.FirstOrDefault(frame => frame.Record.sequence == record.Record.sequence);
            if (target is null) { ShowError("该记录已超出保留范围"); return; }
            pausedFrames = null;
            if (ProtocolDisplay.SelectedIndex == 1) ProtocolDisplay.SelectedIndex = 2;
            OnResetFilters(this, new RoutedEventArgs());
            ScrollTo(target);
        }
    }

    private Task ShowCapture()
    {
        var type = Choice("采集内容", ["收发日志", "原始 RX"]);
        var format = Choice("日志格式", ["txt", "csv", "bin"], 1);
        var path = Field("新文件路径");
        var roll = Field("按大小滚动 (MiB)", "16");
        var browse = new Button { Content = "选择目录" };
        browse.Click += async (_, _) =>
        {
            if (PickFolder is { } picker && await picker() is { } folder)
            {
                var extension = type.SelectedIndex == 1 ? "bin" : (string)format.SelectedItem;
                path.Text = Path.Combine(folder, $"ChipDock-{SessionId}-{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}.{extension}");
            }
        };
        return ToolDialog("持续采集", [type, format, path, browse, roll], "开始", async _ =>
        {
            var size = int.Parse(roll.Text);
            if (size is < 1 or > 16384) throw new InvalidOperationException("滚动大小须为 1–16384 MiB");
            var raw = type.SelectedIndex == 1;
            await Workbench(new { action = "capture", raw, settings = new { path = path.Text, format = raw ? "bin" : format.SelectedItem,
                rollBytes = size * 1024L * 1024L } });
            return "正在采集";
        }, "停止", async _ =>
        {
            await Workbench(new { action = "capture", raw = type.SelectedIndex == 1 });
            return "已停止采集";
        });
    }
}
