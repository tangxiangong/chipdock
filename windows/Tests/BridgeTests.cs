using ChipDock.Core;

namespace ChipDock.Tests;

/// Exercises the real Rust/C# boundary without debug hardware.
internal static class BridgeTests
{
    internal static async Task Run()
    {
        Check.That(System.Text.RegularExpressions.Regex.IsMatch(RustCore.Version(), @"^\d+\.\d+\.\d+$"), "core version crosses the bridge");

        await SerialBridgeTests.Run();

        var chips = await RustCore.SearchChips("STM32F103C8", 10);
        Check.That(chips.Count is > 0 and <= 10 && chips.All(chip => chip.StartsWith("STM32F103C8", StringComparison.OrdinalIgnoreCase)),
            "chip search honours query and limit");
        var map = await RustCore.TargetMemoryMap(chips[0]);
        Check.That(map.Any(region => region.kind.IsNvm) && map.Any(region => region.kind.IsRam), "memory map lists flash and RAM");

        var directory = Path.Combine(Path.GetTempPath(), $"ChipDock-bridge-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        try
        {
            var firmware = Path.Combine(directory, "固件 β.bin");
            await File.WriteAllBytesAsync(firmware, new byte[300]);
            var usage = await RustCore.FirmwareUsage(new FirmwareInput { path = firmware, format = null, base_address = 0x0800_0000 });
            Check.That(usage.format.IsBin && usage.used_bytes == 300 && usage.start_address == 0x0800_0000 && usage.end_address == 0x0800_012C,
                "BIN usage with a Unicode path");
            var missingBase = await Failure(() => RustCore.FirmwareUsage(new FirmwareInput { path = firmware, format = null, base_address = null }));
            Check.That(missingBase.code.IsMissingBinBaseAddress && missingBase.message.Length > 0 && missingBase.recovery.Length > 0,
                "structured error with recovery advice");
            var unsupported = await Failure(() => RustCore.FirmwareUsage(new FirmwareInput { path = Path.Combine(directory, "notes.txt"), format = null, base_address = null }));
            Check.That(unsupported.code.IsUnsupportedFirmwareFormat, "unsupported firmware format");

            var target = new TargetSelection { chip = chips[0], protocol = WireProtocol.Swd, speed_khz = 4000, connect_under_reset = true };
            var profile = new ProfileRecord
            {
                id = Guid.NewGuid().ToString(),
                name = "测试配置 β",
                probe = "1366:0101:000123456789",
                target = target,
                flash_options = new FlashOptions { verify = true, dry_run = false, skip_erase = false, allow_erase_all = false, reset_after = true },
                bin_base_address = 0x0800_4000,
                updated_at_ms = 1_760_000_000_123,
            };
            await RustCore.SaveProfiles(directory, [profile]);
            var loaded = (await RustCore.LoadProfiles(directory)).Single();
            Check.That(loaded.id == profile.id && loaded.name == profile.name && loaded.probe == profile.probe
                && loaded.target.chip == chips[0] && loaded.target.protocol.IsSwd && loaded.target.speed_khz == 4000 && loaded.target.connect_under_reset
                && loaded.flash_options.verify && !loaded.flash_options.dry_run && loaded.flash_options.reset_after
                && loaded.bin_base_address == 0x0800_4000 && loaded.updated_at_ms == profile.updated_at_ms,
                "profile round-trip keeps every field and the timestamp");

            var recent = new RecentFirmwareRecord { path = firmware, format = FirmwareFormat.Bin, base_address = 0x0800_0000, used_at_ms = 1_760_000_000_456 };
            await RustCore.SaveRecentFiles(directory, [recent]);
            var recents = await RustCore.LoadRecentFiles(directory);
            Check.That(recents.Single().path == firmware && recents[0].format.IsBin && recents[0].used_at_ms == recent.used_at_ms, "recent firmware round-trip");

            var read = await Failure(() => RustCore.ReadMemory(new MemoryRequest
            {
                probe = "ffff:ffff:chipdock-missing-probe",
                target = target,
                address = 0x0800_0000,
                length = 64,
            }));
            Check.That(read.message.Length > 0, "hardware errors cross the bridge without a probe");
            Check.That(RustCore.TakeEvents() is not null, "event queue drains");
        }
        finally { Directory.Delete(directory, recursive: true); }
    }

    private static async Task<ErrorResponse> Failure<T>(Func<Task<T>> call)
    {
        try { await call(); }
        catch (CoreException error) { return error.Response; }
        throw new InvalidOperationException("FAIL: expected a Rust Core error");
    }
}
