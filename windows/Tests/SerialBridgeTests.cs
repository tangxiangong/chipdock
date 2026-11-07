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
            var batch = await SerialBackend.Poll(1).WaitAsync(TimeSpan.FromSeconds(3));
            Check.That(!batch.status.connected && batch.status.rx_bytes == 0 && batch.status.tx_bytes == 0,
                "serial bridge polls while the probe worker is blocked");
        }
        finally { release.Set(); }
        Check.That(await probe, "probe worker stays independent until explicitly released");
        var config = new SerialConfig { baud_rate = 115200, data_bits = 8, stop_bits = 1, parity = Parity.None, flow_control = FlowControl.None };
        try { await SerialBackend.Connect(1, "", config, false); throw new Exception("Expected empty-port validation"); }
        catch (InvalidOperationException error) { Check.That(error.Message.Contains("请选择串口"), "serial configuration validation crosses the bridge"); }
        try { await SerialBackend.Send(1, "中文", SendFormat.Ascii, LineEnding.Crlf, ChecksumAlgorithm.None); throw new Exception("Expected ASCII validation"); }
        catch (InvalidOperationException error) { Check.That(error.Message.Contains("ASCII"), "serial encoding validation crosses the bridge"); }
        Check.That(!(await SerialBackend.Disconnect(1)).connected, "serial disconnect is safe without a device");
        await ClosedSessionCannotBeReopened(config);
    }

    private static async Task ClosedSessionCannotBeReopened(SerialConfig config)
    {
        var id = await SerialBackend.Create();
        await SerialBackend.Close(id);
        try
        {
            await SerialBackend.Connect(id, "VIRTUAL-A", config, false);
            throw new Exception("Expected closed session validation");
        }
        catch (InvalidOperationException error)
        {
            Check.That(error.Message.Contains("会话不存在"), "a closed session cannot be reopened by a late connect");
        }
    }
}
