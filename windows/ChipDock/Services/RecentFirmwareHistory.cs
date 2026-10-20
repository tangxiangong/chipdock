using ChipDock.Core;

namespace ChipDock;

/// Serializes history edits against the latest successfully saved list.
internal sealed class RecentFirmwareHistory(
    Func<Task<List<RecentFirmwareRecord>>> load,
    Func<List<RecentFirmwareRecord>, Task> save)
{
    private readonly SemaphoreSlim updates = new(1, 1);
    private bool loaded;
    internal event Action? Changed;
    internal List<RecentFirmwareRecord> Files { get; private set; } = [];
    internal HashSet<string> MissingPaths { get; private set; } = new(StringComparer.OrdinalIgnoreCase);

    internal async Task Load()
    {
        await updates.WaitAsync();
        try { await EnsureLoaded(); }
        finally { updates.Release(); }
    }

    private async Task EnsureLoaded()
    {
        if (loaded) return;
        SetFiles(await load());
        loaded = true;
    }

    private void SetFiles(List<RecentFirmwareRecord> files)
    {
        Files = files;
        MissingPaths = FindMissingFiles();
        Changed?.Invoke();
    }

    internal void RefreshAvailability()
    {
        var missing = FindMissingFiles();
        if (MissingPaths.SetEquals(missing)) return;
        MissingPaths = missing;
        Changed?.Invoke();
    }

    internal Task Record(RecentFirmwareRecord file) => Update(files =>
        [file, .. files.Where(item => !string.Equals(item.path, file.path, StringComparison.OrdinalIgnoreCase)).Take(9)]);

    internal Task Remove(IEnumerable<string> paths)
    {
        var removed = paths.ToHashSet(StringComparer.OrdinalIgnoreCase);
        return Update(files => files.Where(file => !removed.Contains(file.path)).ToList());
    }

    internal Task Clear() => Update(_ => []);

    private HashSet<string> FindMissingFiles() => Files.Where(file => !File.Exists(file.path))
        .Select(file => file.path).ToHashSet(StringComparer.OrdinalIgnoreCase);

    private async Task Update(Func<List<RecentFirmwareRecord>, List<RecentFirmwareRecord>> update)
    {
        await updates.WaitAsync();
        try
        {
            await EnsureLoaded();
            var files = update(Files);
            await save(files);
            SetFiles(files);
        }
        finally { updates.Release(); }
    }
}
