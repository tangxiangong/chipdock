using ChipDock.Core;

namespace ChipDock.Tests;

internal static class SerialBridgeTests
{
    internal static async Task Run()
    {
        var entered = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        using var release = new ManualResetEventSlim();
        var probe = NativeWorker.Probe.Run(() =>
        {
            entered.SetResult(true);
            return release.Wait(TimeSpan.FromSeconds(5));
        });
        try
        {
            await entered.Task.WaitAsync(TimeSpan.FromSeconds(3));
            var batch = await SerialBackend.Poll().WaitAsync(TimeSpan.FromSeconds(3));
            Check.That(!batch.status.connected && batch.status.rx_bytes == 0 && batch.status.tx_bytes == 0,
                "serial bridge polls while the probe worker is blocked");
        }
        finally { release.Set(); }
        Check.That(await probe, "probe worker stays independent until explicitly released");
        var config = new SerialConfig { baud_rate = 115200, data_bits = 8, stop_bits = 1, parity = Parity.None, flow_control = FlowControl.None };
        try { await SerialBackend.Connect("", config); throw new Exception("Expected empty-port validation"); }
        catch (InvalidOperationException error) { Check.That(error.Message.Contains("请选择串口"), "serial configuration validation crosses the bridge"); }
        try { await SerialBackend.Send("中文", SendFormat.Ascii, LineEnding.Crlf); throw new Exception("Expected ASCII validation"); }
        catch (InvalidOperationException error) { Check.That(error.Message.Contains("ASCII"), "serial encoding validation crosses the bridge"); }
        Check.That(!(await SerialBackend.Disconnect()).connected, "serial disconnect is safe without a device");
        await ShutdownFollowsQueuedCalls();
    }
    private static async Task ShutdownFollowsQueuedCalls()
    {
        var entered = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        var shutdownStarted = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        using var release = new ManualResetEventSlim();
        var order = new System.Collections.Concurrent.ConcurrentQueue<string>();
        var blocked = NativeWorker.Serial.Run(() =>
        {
            entered.SetResult(true);
            return release.Wait(TimeSpan.FromSeconds(5));
        });
        await entered.Task.WaitAsync(TimeSpan.FromSeconds(3));
        var queued = NativeWorker.Serial.Run(() => order.Enqueue("pending serial call"));
        var shutdown = Task.Run(() =>
        {
            shutdownStarted.SetResult(true);
            SerialBackend.Shutdown();
            order.Enqueue("shutdown");
        });
        bool completedEarly;
        try
        {
            await shutdownStarted.Task.WaitAsync(TimeSpan.FromSeconds(3));
            completedEarly = await Task.WhenAny(shutdown, Task.Delay(150)) == shutdown;
        }
        finally { release.Set(); }
        await Task.WhenAll(blocked, queued, shutdown);
        Check.That(await blocked, "serial worker is released before its timeout");
        Check.That(!completedEarly && order.SequenceEqual(new[] { "pending serial call", "shutdown" }),
            "shutdown follows pending serial calls instead of allowing them to reopen a closed connection");
    }

}
