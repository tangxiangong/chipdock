using System.Collections.Concurrent;

namespace ChipDock;

/// Probe operations run in order on a dedicated background thread.
internal sealed class NativeWorker
{
    internal static NativeWorker Probe { get; } = new("ChipDock.Probe");
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
