using ChipDock.Core;

namespace ChipDock.Tests;

/// Portable history and display logic; no WinUI or native device APIs are used.
internal static class WorkbenchTests
{
    internal static async Task Run()
    {
        await MissingFilesCanBeRefreshedAndRemoved();
        await QueuedHistoryEditsKeepTheLatestList();
        await StartupHistoryLoadKeepsConcurrentEdits();
        await FailedSavesDoNotBlockLaterEdits();
        await HistoryDeduplicatesWindowsPathsAndKeepsTenFiles();
        Check.That(ActivityText.Message("正在写入（50%）", true) == "正在写入", "full-width trailing progress is removed");
        Check.That(ActivityText.Message("正在写入 (50%) ", true) == "正在写入", "ASCII trailing progress is removed");
        Check.That(ActivityText.Message("正在写入（50%）", false) == "正在写入（50%）", "messages without a separate progress value stay intact");
        Check.That(ActivityText.Message("校验 50% 数据", true) == "校验 50% 数据", "percentages inside a message stay intact");
        Check.That(ActivityText.Percentage(0.285f) == "29%", "progress rounds halfway away from zero");
        Check.That(ActivityText.Percentage(0.9995f) == "100%", "near-complete progress rounds to 100 percent");
    }

    private static async Task MissingFilesCanBeRefreshedAndRemoved()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"ChipDock-history-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        try
        {
            var existing = Path.Combine(directory, "existing.bin");
            var missing = Path.Combine(directory, "missing.bin");
            await File.WriteAllBytesAsync(existing, [0x41]);
            var history = new RecentFirmwareHistory(
                () => Task.FromResult<List<RecentFirmwareRecord>>([FileRecord(existing), FileRecord(missing)]),
                _ => Task.CompletedTask);
            await history.Load();
            Check.That(history.MissingPaths.SetEquals([missing]), "missing recent files are identified at load");
            File.Delete(existing);
            history.RefreshAvailability();
            Check.That(history.MissingPaths.SetEquals([existing, missing]), "removed files are detected on refresh");
            await File.WriteAllBytesAsync(existing, [0x42]);
            history.RefreshAvailability();
            Check.That(!history.MissingPaths.Contains(existing), "restored files become available again");
            await history.Remove(history.MissingPaths);
            Check.That(history.Files.Single().path == existing, "clearing missing records keeps valid files");
            await history.Clear();
            Check.That(history.Files.Count == 0 && File.Exists(existing), "clearing history does not delete firmware files");
        }
        finally { Directory.Delete(directory, recursive: true); }
    }

    private static async Task QueuedHistoryEditsKeepTheLatestList()
    {
        var started = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var writes = 0;
        var history = new RecentFirmwareHistory(
            () => Task.FromResult<List<RecentFirmwareRecord>>([FileRecord("a.bin"), FileRecord("b.bin")]),
            async _ =>
            {
                if (++writes != 1) return;
                started.SetResult();
                await release.Task;
            });
        await history.Load();
        var removeA = history.Remove(["a.bin"]);
        await started.Task.WaitAsync(TimeSpan.FromSeconds(5));
        var recordC = history.Record(FileRecord("c.bin"));
        var removeB = history.Remove(["b.bin"]);
        Check.That(writes == 1, "later history saves wait for the pending save");
        release.SetResult();
        await Task.WhenAll(removeA, recordC, removeB).WaitAsync(TimeSpan.FromSeconds(5));
        Check.That(writes == 3 && history.Files.Select(file => file.path).SequenceEqual(["c.bin"]),
            "opening and removing files concurrently does not restore deleted records");
    }

    private static async Task StartupHistoryLoadKeepsConcurrentEdits()
    {
        var snapshot = new TaskCompletionSource<List<RecentFirmwareRecord>>(TaskCreationOptions.RunContinuationsAsynchronously);
        var reads = 0;
        List<RecentFirmwareRecord>? saved = null;
        var history = new RecentFirmwareHistory(
            () => { reads++; return snapshot.Task; },
            files => { saved = files; return Task.CompletedTask; });
        var record = history.Record(FileRecord("c.bin"));
        var startup = history.Load();
        snapshot.SetResult([FileRecord("a.bin"), FileRecord("b.bin")]);
        await Task.WhenAll(startup, record).WaitAsync(TimeSpan.FromSeconds(5));
        Check.That(reads == 1 && history.Files.Select(file => file.path).SequenceEqual(["c.bin", "a.bin", "b.bin"]),
            "startup loading and concurrent edits preserve both saved and new records");
        Check.That(saved is not null && saved.Select(file => file.path).SequenceEqual(["c.bin", "a.bin", "b.bin"]),
            "the first edit preserves records already on disk before saving");
    }

    private static async Task FailedSavesDoNotBlockLaterEdits()
    {
        var writes = 0;
        var history = new RecentFirmwareHistory(
            () => Task.FromResult<List<RecentFirmwareRecord>>([FileRecord("a.bin")]),
            _ => ++writes == 1
                ? Task.FromException(new IOException("save failed"))
                : Task.CompletedTask);
        await history.Load();
        await Check.ThrowsAsync<IOException>(() => history.Remove(["a.bin"]), "failed history saves are reported");
        Check.That(history.Files.Single().path == "a.bin", "a failed save keeps the last saved list");
        await history.Record(FileRecord("b.bin")).WaitAsync(TimeSpan.FromSeconds(5));
        Check.That(history.Files.Select(file => file.path).SequenceEqual(["b.bin", "a.bin"]), "later edits succeed after a failed save");
    }

    private static async Task HistoryDeduplicatesWindowsPathsAndKeepsTenFiles()
    {
        var history = new RecentFirmwareHistory(
            () => Task.FromResult(Enumerable.Range(0, 12).Select(index => FileRecord($"{index}.bin")).ToList()),
            _ => Task.CompletedTask);
        await history.Load();
        await history.Record(FileRecord("0.BIN"));
        Check.That(history.Files.Count == 10 && history.Files.Count(file => file.path.Equals("0.bin", StringComparison.OrdinalIgnoreCase)) == 1,
            "Windows paths are deduplicated ignoring case and history is bounded");
        await history.Remove(["0.bin"]);
        Check.That(history.Files.All(file => !file.path.Equals("0.BIN", StringComparison.OrdinalIgnoreCase)), "record removal ignores path case");
    }

    private static RecentFirmwareRecord FileRecord(string path) => new()
    {
        path = path,
        format = FirmwareFormat.Bin,
        base_address = 0x0800_0000,
        used_at_ms = 0,
    };
}
