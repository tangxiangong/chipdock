using System.Text.Json;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChipDock;

public sealed partial class SerialView
{
    internal Action<ulong>? OpenPlot { get; set; }

    private static readonly string[] ChecksumNames =
        ["none", "sum8", "oddParity", "evenParity", "xor8", "crc16Modbus", "crc16Ccitt", "crc32", "lrc"];

    private static JsonElement ParseArray(string text)
    {
        if (System.Text.Encoding.UTF8.GetByteCount(text) > 1024 * 1024)
            throw new InvalidOperationException("配置超过 1 MiB");
        var json = JsonSerializer.Deserialize<JsonElement>(text);
        return json.ValueKind == JsonValueKind.Array ? json : throw new InvalidOperationException("请输入 JSON 数组");
    }

    private static TextBox JsonEditor(string title, string json)
    {
        var editor = Field(title, json);
        editor.AcceptsReturn = true;
        editor.TextWrapping = TextWrapping.Wrap;
        editor.MinHeight = 140;
        return editor;
    }

    private Task ShowPackets()
    {
        var json = JsonEditor("包库 JSON", preferences.Packets);
        var selected = Field("按发送顺序填写包名，逗号分隔（留空发送全部启用包）");
        var interval = Field("包间隔 (ms)", "50");
        var example = new Button { Content = "示例" };
        var import = new Button { Content = "导入 JSON…" };
        var export = new Button { Content = "导出 JSON…" };
        example.Click += (_, _) => json.Text = """
            [{"name":"读状态","group":"调试","note":"","order":0,"enabled":true,"text":"AA 01","format":"hex"}]
            """;
        async Task<JsonElement> Save(string text)
        {
            var packets = ParseArray(text);
            await Workbench(new { action = "validatePackets", packets });
            preferences.Packets = text;
            preferences.Save();
            return packets;
        }
        import.Click += async (_, _) =>
        {
            try
            {
                if (PickFile is not { } pick || await pick() is not { } path) return;
                if (new FileInfo(path).Length > 1024 * 1024) throw new InvalidOperationException("包库超过 1 MiB");
                var text = await File.ReadAllTextAsync(path);
                await Save(text); json.Text = text;
            }
            catch (Exception error) { ShowError(error.Message); }
        };
        export.Click += async (_, _) =>
        {
            try
            {
                if (PickExport is { } pick && await pick("ChipDock-Packets", "json") is { } path)
                    await File.WriteAllTextAsync(path, preferences.Packets);
            }
            catch (Exception error) { ShowError(error.Message); }
        };
        return ToolDialog("常用包库", [json, selected, interval, example, import, export],
            "保存", async _ => { await Save(json.Text); return "已保存包库"; }, "批量发送", async _ =>
            {
                var packets = await Save(json.Text);
                return await StartJob(new { kind = "batch", packets,
                    selected = selected.Text.Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries),
                    intervalMs = int.Parse(interval.Text) });
            }, trackJob: true);
    }

    private Task ShowSendFile()
    {
        var path = Field("文件路径");
        var chunk = Field("块大小 (字节)", "4096");
        var interval = Field("块间隔 (ms)", "50");
        var browse = new Button { Content = "选择文件…" };
        browse.Click += async (_, _) =>
        {
            if (PickFile is { } pick && await pick() is { } file) path.Text = file;
        };
        return ToolDialog("发送文件", [path, browse, chunk, interval], "开始", _ =>
            StartJob(new { kind = "file", path = path.Text, chunkBytes = int.Parse(chunk.Text), intervalMs = int.Parse(interval.Text) }),
            "取消任务", async _ => { await CancelJob(); return "已请求取消"; }, trackJob: true);
    }

    private Task ShowMacro()
    {
        var steps = JsonEditor("宏步骤 JSON",
            """[{"send":{"text":"PING","format":"utf-8"},"wait":{"kind":"text","pattern":"OK"},"timeoutMs":1000}]""");
        var cycles = Field("循环次数", "1");
        var export = new Button { Content = "导出结果 CSV…" };
        export.Click += async (_, _) =>
        {
            try
            {
                if (PickExport is { } pick && await pick("ChipDock-Macro", "csv") is { } path)
                    await Workbench(new { action = "exportMacro", path });
            }
            catch (Exception error) { ShowError(error.Message); }
        };
        return ToolDialog("宏命令", [steps, cycles, export], "运行", _ =>
            StartJob(new { kind = "macro", steps = ParseArray(steps.Text), cycles = int.Parse(cycles.Text) }),
            "取消任务", async _ => { await CancelJob(); return "已请求取消"; }, trackJob: true);
    }

    private Task ShowReplies()
    {
        var rules = JsonEditor("自动应答 JSON", preferences.Replies);
        var example = new Button { Content = "示例" };
        example.Click += (_, _) => rules.Text =
            """[{"name":"pong","enabled":true,"trigger":{"kind":"text","pattern":"PING"},"reply":{"text":"OK","format":"utf-8"},"delayMs":20}]""";
        return ToolDialog("自动应答", [rules, example], "应用", async _ =>
        {
            await Workbench(new { action = "replies", rules = ParseArray(rules.Text) });
            preferences.Replies = rules.Text; preferences.Save();
            return "已应用应答规则";
        });
    }

    private async Task<string> StartJob(object job)
    {
        await Workbench(new { action = "startJob", job });
        return "发送任务已启动";
    }

    private async void OnCancelJob(object sender, RoutedEventArgs args) => await CancelJob();
    private async Task CancelJob()
    {
        try { await Workbench(new { action = "cancelJob" }); }
        catch (Exception error) { ShowError(error.Message); }
    }
}
