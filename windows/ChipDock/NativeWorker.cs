using System.Collections.Concurrent;

namespace ChipDock;

/// Each hardware feature owns a serial queue and a dedicated background thread.
internal sealed class NativeWorker
{
    internal static NativeWorker Probe { get; } = new("ChipDock.Probe");
    internal static NativeWorker Serial { get; } = new("ChipDock.Serial");
    private readonly BlockingCollection<Action> jobs = new();
    private NativeWorker(string name)
    {
        var thread = new Thread(() => { foreach (var job in jobs.GetConsumingEnumerable()) job(); })
        { Name = name, IsBackground = true };
        thread.Start();
    }
    internal Task<T> Run<T>(Func<T> work)
    {
        var completion = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        jobs.Add(() =>
        {
            try { completion.SetResult(work()); }
            catch (Exception error) { completion.SetException(error); }
        });
        return completion.Task;
    }
    internal Task Run(Action work) => Run(() => { work(); return true; });
}
