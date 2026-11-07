using System.Globalization;
using System.Text;
using System.Text.Json;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;

namespace ChipDock;

internal sealed class SerialPlotWindow : Window
{
    private readonly ulong session;
    private readonly Func<string, string, Task<string?>> save;
    private readonly Canvas canvas = new() { MinHeight = 210 };
    private readonly ComboBox mode = new() { Header = "输入格式", ItemsSource = new[] { "numbers", "kv", "json", "binary" }, SelectedIndex = 0 };
    private readonly TextBox fields = new() { Header = "字段名（逗号分隔）" };
    private readonly TextBox template = new() { Header = "协议规则名（可选）" };
    private readonly TextBox binary = new() { Header = "二进制字段 JSON", Text = """[{"name":"value","offset":0,"type":"u16","littleEndian":false,"scale":1,"bias":0}]""" };
    private readonly TextBox sample = new() { Header = "样例文本或 HEX" };
    private readonly TextBox filter = new() { Header = "只显示字段（可选）" };
    private readonly TextBlock result = new() { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
    private readonly DispatcherQueueTimer timer;
    private readonly List<PlotSample> samples = [];
    private JsonElement config = JsonSerializer.Deserialize<JsonElement>("""{"mode":"numbers"}""");
    private ulong cursor, dropped, index;
    private int revision;
    private bool busy, paused, closed;

    internal SerialPlotWindow(ulong session, Func<string, string, Task<string?>> save)
    {
        this.session = session; this.save = save;
        Title = $"实时曲线 · 会话 {session}";
        AppWindow.Resize(new Windows.Graphics.SizeInt32(880, 640));
        var root = new Grid { Padding = new Thickness(16), RowSpacing = 10 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition());
        var options = Row(mode, fields, template, Button("应用", () => Configure(false)));
        var samplesRow = Row(sample, Button("试算", () => Configure(true)), filter);
        var controls = Row(Button("暂停／继续", TogglePause), Button("清空", Clear), Button("导出 CSV…", Export), result);
        binary.Visibility = Visibility.Collapsed;
        mode.SelectionChanged += (_, _) =>
        {
            binary.Visibility = mode.SelectedIndex == 3 ? Visibility.Visible : Visibility.Collapsed;
            fields.IsEnabled = mode.SelectedIndex != 3;
        };
        var upper = new StackPanel { Spacing = 6 };
        upper.Children.Add(options); upper.Children.Add(binary);
        root.Children.Add(upper);
        Grid.SetRow(samplesRow, 1); root.Children.Add(samplesRow);
        Grid.SetRow(controls, 2); root.Children.Add(controls);
        Grid.SetRow(canvas, 3); root.Children.Add(canvas);
        Content = root;
        canvas.SizeChanged += (_, _) => Draw();
        filter.TextChanged += (_, _) => Draw();
        timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(120);
        timer.Tick += (_, _) => _ = Poll();
        Closed += (_, _) => { closed = true; timer.Stop(); };
        timer.Start();
    }

    private static StackPanel Row(params UIElement[] controls)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        foreach (var item in controls) row.Children.Add(item);
        return row;
    }

    private Button Button(string name, Func<Task> click)
    {
        var button = new Button { Content = name, VerticalAlignment = VerticalAlignment.Bottom };
        button.Click += async (_, _) => { try { await click(); } catch (Exception error) { result.Text = error.Message; } };
        return button;
    }

    private Task TogglePause() { paused = !paused; result.Text = paused ? "已暂停" : ""; return Task.CompletedTask; }
    private Task Clear() { revision++; samples.Clear(); dropped = 0; result.Text = ""; Draw(); return Task.CompletedTask; }

    private async Task<JsonElement> Request(object payload) =>
        JsonSerializer.Deserialize<JsonElement>(await SerialBackend.Workbench(session, JsonSerializer.Serialize(payload)));

    private async Task Configure(bool preview)
    {
        var kind = (string)mode.SelectedItem;
        var settings = new Dictionary<string, object> { ["mode"] = kind, ["template"] = template.Text };
        if (kind == "binary") settings["binary"] = JsonSerializer.Deserialize<JsonElement>(binary.Text);
        else settings["fields"] = fields.Text.Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries);
        var response = await Request(new { action = "plot", config = settings, sample = preview ? sample.Text : "", hex = kind == "binary" });
        if (preview) result.Text = response.GetProperty("values").ToString();
        else
        {
            config = JsonSerializer.SerializeToElement(settings);
            await Clear();
        }
    }

    private async Task Poll()
    {
        if (closed || paused || busy) return;
        busy = true;
        var at = revision;
        try
        {
            var reply = await Request(new { action = "plot", config, cursor });
            if (closed || paused || at != revision) return;
            cursor = reply.GetProperty("nextCursor").GetUInt64();
            dropped += reply.GetProperty("droppedFrames").GetUInt64();
            if (dropped > 0) result.Text = $"已超出保留范围 {dropped} 条记录";
            var points = reply.GetProperty("points");
            if (points.GetArrayLength() == 0) return;
            foreach (var reading in points.EnumerateArray())
            {
                index++;
                foreach (var field in reading.GetProperty("values").EnumerateObject())
                    samples.Add(new(index, reading.GetProperty("seq").GetUInt64(), reading.GetProperty("timestamp").GetString() ?? "",
                        field.Name, field.Value.GetDouble()));
            }
            if (samples.Count > 4096) samples.RemoveRange(0, samples.Count - 4096);
            Draw();
        }
        catch (Exception error)
        {
            if (!closed) { paused = true; result.Text = error.Message; }
        }
        finally { busy = false; }
    }

    private void Draw()
    {
        canvas.Children.Clear();
        var width = canvas.ActualWidth - 56; var height = canvas.ActualHeight - 32;
        var shown = samples.Where(sample => filter.Text.Length == 0 || sample.Field == filter.Text).ToArray();
        if (shown.Length == 0 || width <= 0 || height <= 0) return;
        var min = shown.Min(sample => sample.Value); var max = shown.Max(sample => sample.Value);
        var first = shown.Min(sample => sample.Index); var last = shown.Max(sample => sample.Index);
        var palette = new[] { Microsoft.UI.Colors.DodgerBlue, Microsoft.UI.Colors.SeaGreen, Microsoft.UI.Colors.DarkOrange,
            Microsoft.UI.Colors.MediumPurple, Microsoft.UI.Colors.IndianRed };
        var offset = 0;
        foreach (var series in shown.GroupBy(sample => sample.Field))
        {
            var color = Ui.Brush(palette[offset++ % palette.Length]);
            var line = new Polyline { StrokeThickness = 2, Stroke = color };
            foreach (var point in series)
                line.Points.Add(new Point(46 + width * (point.Index - first) / Math.Max(1UL, last - first),
                    12 + height * (max == min ? 0.5 : 1 - (point.Value - min) / (max - min))));
            canvas.Children.Add(line);
            var label = new TextBlock { Text = series.Key, Foreground = color, FontSize = 11 };
            Canvas.SetLeft(label, 46 + (offset - 1) * 76);
            Canvas.SetTop(label, height + 16);
            canvas.Children.Add(label);
        }
        var upper = new TextBlock { Text = max.ToString("G5", CultureInfo.InvariantCulture) };
        var lower = new TextBlock { Text = min.ToString("G5", CultureInfo.InvariantCulture) };
        Canvas.SetTop(lower, height);
        canvas.Children.Add(upper); canvas.Children.Add(lower);
    }

    private async Task Export()
    {
        if (samples.Count == 0 || await save("ChipDock-Plot", "csv") is not { } path) return;
        var csv = new StringBuilder("sample,sequence,timestamp,field,value\r\n");
        foreach (var sample in samples)
            csv.Append(CultureInfo.InvariantCulture, $"{sample.Index},{sample.Sequence},{sample.Timestamp},\"{sample.Field.Replace("\"", "\"\"")}\",{sample.Value}\r\n");
        await File.WriteAllTextAsync(path, csv.ToString());
    }

    private sealed record PlotSample(ulong Index, ulong Sequence, string Timestamp, string Field, double Value);
}
