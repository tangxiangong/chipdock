namespace ChipDock;

/// <summary>Repeats one completed send at a time and owns cancellation of the next send.</summary>
internal sealed class SerialAutoSender
{
    private readonly object gate = new();
    private CancellationTokenSource? cancellation;
    internal bool IsRunning { get { lock (gate) return cancellation is not null; } }
    internal event Action? Changed;
    internal event Action<Exception>? Failed;

    internal bool Start(int intervalMilliseconds, Func<Task<bool>> send)
    {
        if (intervalMilliseconds is < 10 or > 3_600_000) return false;
        Stop();
        var source = new CancellationTokenSource();
        lock (gate) cancellation = source;
        Changed?.Invoke();
        _ = Run(source, intervalMilliseconds, send);
        return true;
    }

    internal void Stop()
    {
        bool changed;
        lock (gate)
        {
            changed = cancellation is not null;
            cancellation?.Cancel();
            cancellation = null;
        }
        if (changed) Changed?.Invoke();
    }

    private async Task Run(CancellationTokenSource source, int intervalMilliseconds, Func<Task<bool>> send)
    {
        var token = source.Token;
        try
        {
            while (!token.IsCancellationRequested)
            {
                if (!await send()) break;
                await Task.Delay(intervalMilliseconds, token);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch (Exception error) { Failed?.Invoke(error); }
        finally
        {
            bool changed;
            lock (gate)
            {
                changed = ReferenceEquals(cancellation, source);
                if (changed) cancellation = null;
            }
            source.Dispose();
            if (changed) Changed?.Invoke();
        }
    }
}
