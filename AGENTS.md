# AGENTS.md

## Documentation

- Write README files for users. Use the root README for downloads and common tasks, platform READMEs for installation and platform-specific help, and directory READMEs for a short description and relevant links.
- Keep implementation notes, build and test commands, and release infrastructure in this file.
- Omit legacy upgrade instructions from future release notes. Focus each announcement on the changes in that release.

## Code layout

- `crates/chipdock-core` handles firmware analysis, probe-rs operations, serial transport, and storage. ESP target and BIN address logic lives in `src/services/esp.rs`. Serial templates and receive parsing live in `src/serial/program.rs`.
- `crates/chipdock-service` provides the shared application API, structured errors, progress events, and hardware exclusion. Probe operations run serially on a dedicated worker. Serial sessions use a shared Tokio runtime and independent connection and send locks. Each record reader owns a cursor.
- `crates/chipdock-control` defines local IPC and the shared client. `crates/chipdock-tools` builds the Rust CLI and stdio MCP server. Only the desktop process owns serial connections.
- The SwiftUI/AppKit client lives in `macos/ChipDock/Sources/ChipDock`. It uses the UniFFI exports in `crates/chipdock-ffi` through `ChipDockCore` and `ChipDockCoreFFI`.
- The WinUI client lives in `windows/ChipDock`. Its `RustCore.cs` calls Interoptopus exports from `crates/chipdock-csharp` through `ChipDock.Core` and `chipdock_csharp.dll`. Preserve typed Wire requests and responses, structured errors, and Unix-millisecond timestamps at the boundary.
- When changing Windows App Runtime, update the package reference in `windows/ChipDock/ChipDock.csproj` and the version, installer URLs, and SHA-256 values in `WindowsRuntime.cs`. `ChipDock.exe --prepare-runtime` prepares the runtime without opening the UI.
- Windows probe-driver selection lives in `windows/ChipDock/ProbeDriverRules.cs`. Installation and re-enumeration live in `ProbeDriver.cs`. Limit repair to known debug interfaces on the selected device and exclude composite parents, HID, CDC, storage, and hubs. ESP USB-JTAG (`303A:1001`) uses `MI_02`. Match FTDI by debug channel and Glasgow by the enumerated applet interface pair. Require an unambiguous physical device and recheck its identity through probe-rs after repair.

## Build and bindings

Run commands from the repository root. Xcode and MSBuild generate their own bindings on a clean checkout. Track the generator and export sources in Git and exclude generated declarations.

Declare dependency versions, sources, and default feature settings in the root `[workspace.dependencies]`. Every member inherits its dependencies with `workspace = true`, adding member-specific features or optionality where needed.

- **macOS.** Use Xcode 26+ and stable Rust from rustup. The app targets macOS 14+. Open `macos/ChipDock.xcodeproj` with the `ChipDock` scheme or run `CONFIGURATION=Debug bash macos/build.sh`. Set `ARCH=universal` for arm64 and x86_64. Set `BUILD_ONLY=0` to copy the bundle to `dist/macos/<arch>/`. `bash macos/run.sh` is the launch entry point.
- Xcode's `RustBindings` target calls `macos/build-bindings.sh` to build the Rust static library and generate Swift, the C header, and the module map under `$(BUILD_DIR)/ChipDockBindings/$(CONFIGURATION)`. To generate bindings from an existing library, run `bash bindings/generate-swift.sh target/debug/libchipdock_ffi.dylib`.
- **Windows.** Use Visual Studio 2026 with Windows application development, C++ desktop development, and the Windows SDK, plus .NET 10 and the Rust MSVC toolchain. Open `windows/ChipDock.sln` or run `dotnet build windows/ChipDock.sln -c Debug -p:Platform=x64`. Use `-p:Platform=ARM64` for ARM64.
- MSBuild's `windows/ChipDock/Build/Rust.targets` builds the DLL, runs `chipdock-csharp-bindgen`, then compiles `bindings/generated/csharp/Interop.cs`. IntelliSense uses the last generated declarations. For standalone generation, run `cargo run --locked -p chipdock-csharp-bindgen`. The DLL is at `target/<target-triple>/<profile>/chipdock_csharp.dll`.

## Verification

Default to native host builds, logic tests, and static checks. Get explicit user authorization before accessing devices, installing drivers, or controlling a running app. Use manual GUI acceptance. Report host, CI, installer, GUI, and hardware validation separately.

- Shared Rust formatting, linting, and tests

  ```sh
  cargo fmt --all -- --check
  cargo clippy --locked -p chipdock-core -p chipdock-service -p chipdock-control -p chipdock-tools -p chipdock-release-notes --all-targets -- -D warnings
  cargo test --locked -p chipdock-core -p chipdock-service -p chipdock-control -p chipdock-tools -p chipdock-release-notes --all-targets
  ```

- macOS bridge, serial, and update checks. Resolve Xcode's Sparkle dependency before the mirror policy check.

  ```sh
  bash macos/ChipDock/Tests/run-interop-tests.sh
  bash macos/ChipDock/Tests/run-serial-tests.sh
  bash macos/ChipDock/Tests/run-update-tests.sh
  bash macos/ChipDock/Tests/run-update-mirror-tests.sh
  ```

- Run the app-hosted XCTest suite with the shared scheme. Its Test action uses build-local preferences and disables automatic serial polling, local IPC, and update startup through `CHIPDOCK_TEST_HOST=1`. Keep XCTest focused on logic and bridge behavior; use manual GUI acceptance for window layout.

  ```sh
  xcodebuild -project macos/ChipDock.xcodeproj -scheme ChipDock \
    -configuration Debug -destination 'platform=macOS' \
    -derivedDataPath .build/XCTest -clonedSourcePackagesDirPath .build/XCTest/SourcePackages \
    SWIFT_TREAT_WARNINGS_AS_ERRORS=YES test
  ```

- Portable Windows logic checks. On Windows, pass the built `chipdock_csharp.dll` path to the test project to exercise the Rust/C# bridge. The full Windows test run includes read-only SetupAPI enumeration and WinUSB preflight checks. Driver installation scenarios use simulations.

  ```powershell
  dotnet run --project windows/Tests/ChipDock.Tests.csproj -- --workbench-only
  dotnet run --project windows/Tests/ChipDock.Tests.csproj -- --updates-only
  dotnet run --project windows/Tests/ChipDock.Tests.csproj -- --driver-rules-only
  ```

- The [Rust workflow](.github/workflows/rust.yml) runs shared checks on Linux. The [macOS workflow](.github/workflows/macos.yml) and [Windows workflow](.github/workflows/windows.yml) build and validate each native client and bridge on both of its supported architectures.

## Releases

1. Synchronize the workspace version in `Cargo.toml`, `MARKETING_VERSION` in `macos/ChipDock.xcodeproj/project.pbxproj`, and `Version` in `windows/ChipDock/ChipDock.csproj`.
2. Publishing a stable GitHub Release with a matching `v<version>` tag triggers the platform workflows. They run their validation jobs before publishing. The GitHub `release` environment supplies `SPARKLE_PRIVATE_ED_KEY`, `WINDOWS_UPDATE_PRIVATE_KEY`, `COS_SECRET_ID`, and `COS_SECRET_KEY`. The clients embed only the update public keys.
3. The workflows publish the macOS, Windows and Linux installers and signed update metadata, advance the GitHub `chipdock-updates` feed, then mirror the verified release to COS. All jobs that advance feeds share the `chipdock-updates` concurrency group. Keep `appcast.xml`, each `releases.win-<arch>.signed.json` and `linux-stable.signed.json` with the versioned assets for verification and recovery.

- **macOS.** Keep `com.xiaoyu.chipdock` and `ChipDock.app` as the installation identity. `ci/release-macos.sh` packages the universal app, generates the Sparkle Ed25519 appcast, and verifies the signed archive. Local packaging reads `.build/Release/Build/Products/Release/ChipDock.app` by default and writes `.build/ReleaseAssets/`. Set `APP_BUNDLE` and `SPARKLE_BIN` to use other build output paths. Local signing reads the private key from the environment or the `com.xiaoyu.chipdock` login keychain account. CI runs the script with `--publish`.
- **Windows.** Keep Velopack package ID `ChipDock`, executable `ChipDock.exe`, and channels `win-x64` and `win-arm64`. Each Windows runner uses `windows/scripts/publish.ps1` to build EXE, MSI, and full `.nupkg` packages. `ci/release-windows.ps1` signs and verifies feed metadata using `windows/UpdateTool`, reuses verified signatures on retries, and checks package identity, size, and SHA-256. Coordinate signing key rotation with the public key in `ChipDock.csproj` and the `WINDOWS_UPDATE_PRIVATE_KEY` environment secret.
- Run `cargo test --locked -p xtask` for local release tool tests. Use `cargo run --locked -p xtask -- verify-release --tag "v<version>"` to download and verify GitHub assets. Add `--platform macos` or `--platform windows` to verify just one platform.

### COS mirror

- Both clients prefer Tencent Cloud COS and fall back to GitHub on eligible feed or download failures. End the check when a valid primary feed reports no update. macOS retries once on transport failure and preserves the manual or background mode. Windows preserves the selected version, filename, size, and SHA-256 on package fallback. Stop on user cancellation.
- Read the current endpoints and cache policies in `xtask/src/cos.rs`. Enable global acceleration for uploads and use the public HTTPS endpoint for client downloads. Scope the publisher's `cos:HeadObject`, `cos:PutObject`, and `cos:PutObjectACL` permissions to `chipdock/*`. The versioned files live under `chipdock/releases/v<version>/` and the live feeds under `chipdock/updates/`.
- `xtask` verifies GitHub digests and update signatures, then checks public object bytes before promoting feeds. The COS appcast changes only the download URL. Both Windows sources use the same signed metadata. Normal publishing rejects conflicts with immutable assets. Preserve `updates/state.json` and all assets still referenced by a feed. The state file reserves every pending channel version before its live feed moves, preventing an interrupted release or old retry from downgrading the feed.
- For a failed mirror, run [Publish existing release to COS](.github/workflows/publish-cos.yml) with an existing stable tag for the current installation identity and platform `all`, `macos`, `windows`, or `linux` (`gpui` is an alias). Ordinary recovery runs `xtask publish-cos` with the published installers and signed feeds. Enable `replace_existing` only after the maintainer explicitly replaces the corresponding GitHub assets. That path calls `xtask replace-cos-release` and requires a valid state file while retaining feed downgrade protection.

### Platform-specific release notes

`crates/chipdock-release-notes` selects common text and platform blocks from the GitHub Release body. Use standalone opening markers such as `<!-- platform:macos -->` and the standalone closing marker `<!-- /platform -->`. Valid selectors are `macos`, `windows`, `linux`, and `all`. Use comma-separated platform names or `all` by itself. Put platform headings inside their blocks, keep blocks separate and closed, and include text for macOS, Windows and Linux. The parser validates every block.

Preview the selected text locally. For local packaging, pass the complete source via `RELEASE_NOTES_FILE` on macOS or `publish.ps1 -ReleaseNotes` on Windows.

```sh
cargo run --locked -p chipdock-release-notes -- macos ReleaseNotes.md .build/notes-macos.md
cargo run --locked -p chipdock-release-notes -- windows ReleaseNotes.md .build/notes-windows.md
cargo run --locked -p chipdock-release-notes -- linux ReleaseNotes.md .build/notes-linux.md
```


## GPUI client

- `linux/` is the cross-platform Rust / GPUI client, published as the Linux frontend. `src/ui/` splits state, events, common controls and individual pages. `backend.rs` dispatches probe, serial and update work on independent workers. `platform/windows/` uses the typed `windows` crate for SetupAPI and WinUSB; there is no .NET helper. All direct Windows API dependencies in this workspace use `windows`; transitive dependencies may still contain `windows-sys`.
- `linux/update/` is a UI-independent update crate. It owns the signed GPUI feed, downloads, portable installation, signing and publishing commands. Keep its public key synchronized with `SPARKLE_PUBLIC_ED_KEY`; private keys remain in the release environment.
- Build and package natively with `bash linux/build.sh` (release) or `PROFILE=debug bash linux/build.sh`. `OUTPUT_DIR` defaults to `dist/gpui`. The executable is `ChipDock` (`ChipDock.exe` on Windows). Linux produces `ChipDock-<version>-linux-<arch>` with `.deb`, `.rpm` or `.tar.gz` and uses desktop ID `com.xiaoyu.chipdock`. The macOS source build creates an ad-hoc-signed `ChipDock.app` in that output directory, requires macOS 15+ and keeps the distinct bundle ID `com.xiaoyu.chipdock.gpui`. Windows source builds use `cargo build --locked -p chipdock-gpui --bin ChipDock`. `build.rs` embeds the existing ICO and suppresses the console window.
- Ubuntu build packages: `pkg-config clang cmake libudev-dev libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libvulkan1`. Packaging additionally uses `dpkg-dev rpm cpio desktop-file-utils shellcheck`. Build on the native Ubuntu 24.04 architecture; DEB uses `dpkg-shlibdeps` and RPM uses automatic ELF dependencies plus explicit dynamically loaded libraries. Both formats install to `/usr` and include a package-managed marker; their updates must go through the package manager. Linux runtime acceptance requires a graphical session and working Vulkan driver.
- Host verification: `cargo clippy --locked -p chipdock-gpui -p chipdock-gpui-update -p xtask --all-targets -- -D warnings`; `cargo test --locked -p chipdock-gpui --all-targets`; `cargo test --locked -p chipdock-gpui-update -p xtask`. These do not launch the GUI or use physical devices. Use manual GUI acceptance; do not add GUI automation tests.
- `.github/workflows/linux.yml` builds Linux x86_64 and aarch64, validates extracted DEB/RPM/tarball metadata, icons and exact binary bytes without installation or GUI access, then signs `linux-stable.signed.json`. The signature covers both raw update binaries and versioned DEB/RPM/tarball hashes. Versioned assets publish before the `chipdock-updates` feed advances under the shared concurrency group. `xtask --platform linux` (`gpui` alias) verifies all signed payloads before mirroring. `all` includes Linux when its feed is present and supports the historical `gpui-stable.signed.json` format.
- Run `bash linux/tests/packaging.sh` for host installer/path and architecture logic checks; on Linux, add `<asset-directory> <x86_64|aarch64>` to extract and verify the real packages. Run `shellcheck linux/*.sh linux/tests/*.sh` for shell linting. These checks never launch the app or install packages/drivers.
- Release tools: `cargo run --locked -p chipdock-gpui-update --bin sign-gpui-update -- v<VERSION> <DIRECTORY> <NOTES_FILE>` and `cargo run --locked -p chipdock-gpui-update --bin publish-gpui-update -- v<VERSION> <DIRECTORY>`. Signing requires the matching workspace tag, both Linux binaries, all six DEB/RPM/tarball packages and `SPARKLE_PRIVATE_ED_KEY`; publishing requires the standard GitHub environment. Neither command runs during local builds.
- The Linux feed retains the signature identity `com.xiaoyu.chipdock.gpui` and names portable binaries `ChipDock-<version>-linux-<arch>`. The DEB package ID is `chipdock` and the RPM package ID is `ChipDock`. Package metadata replaces the original `chipdock-gpui` installation. The first Linux v0.3.0 portable build uses the historical feed and needs a reinstall to switch feeds. Keep preference paths intact. Do not apply single-binary updates inside signed `.app` bundles or package-manager-owned installations.

### GPUI parity audit (2026-11-07)

This is a source/API and native-host logic audit, not a GUI, hardware, installer or three-platform acceptance certificate. Compare both native clients when changing these paths. GPUI keeps the main sidebar and also exposes independent memory, log and per-session plot windows.

| Capability checked in native clients | GPUI route and audit result |
| --- | --- |
| `FlashView.swift`, `MainWindow.cs`: open, drag/drop, launch handoff, close and reveal firmware | `ui/actions.rs`, `ui/flash.rs`, `ui/events.rs`, `instance.rs`; automatic analysis rejects stale results after changing the path or BIN base. |
| `WorkbenchOperations.swift`, `RecentFirmwareHistory.cs`: recent files and BIN addresses | `model.rs`, `ui/profiles.rs`; preserve each file's base, reopen, reveal, remove, prune missing files and clear history. |
| `TargetSetupSheet.swift`, `TargetSetupDialog.cs`: scan, chip search, autodetection, SWD/JTAG, speed and reset | Shared typed service requests; reject zero speed before device access and retain structured errors, recovery advice and target candidates. |
| `FlashView.swift`, `FlashView.cs`: preflight, erase policies, verify and reset | Shared flash engine; explicit erase confirmation names the captured target and locks conflicting form changes. |
| `MemoryView.swift`, `MemoryView.cs`: regions, address/length, HEX/ASCII, export and erase | `ui/memory.rs`, `ui/auxiliary.rs`; region selection, BIN save dialog and confirmed erase route. |
| `SaveProfileSheet.swift`, `SaveProfileDialog.cs`: save/apply/delete profiles | `ui/profiles.rs`; blank names fail; a repeated name creates a numbered copy instead of silently overwriting a saved configuration. |
| `LogsView.swift`, `LogsView.cs`: search, error filter, follow, copy/export, clear and unseen errors | `ui/logs.rs`, `ui/state.rs`; tagged diagnostics, a tracked scroll container and an independent log window. |
| `SerialStore.swift`, `SerialWorkbench.cs`: sessions and local CLI/MCP | `backend.rs`, `ui/state.rs`; initialize the real default session 1, isolate operation queues by session, restore protocol/framing/replies and prohibit closing the last session. |
| Native serial connection views: baud, data/stop bits, parity, flow, reconnect and RTS/DTR | Shared serial engine and `ui/polling.rs`; poll every session independently, synchronize port/config changes made by recovery or IPC, and refresh the port list. |
| Native serial composers: encoding, line endings, checksum range, repeat and history | `serial.rs`, `ui/terminal.rs`; separate normal/command histories and session-owned drafts. |
| Native terminals: pause, clear, timestamps, mixed HEX/text, autoscroll, search and selection | `presentation.rs`, `ui/terminal_state.rs`; one filter policy for records/terminal, stable clear cursors, selectable monospace editor and byte-range highlights. |
| Native color rules: literal/regex, priority, named colors and independent TX color | `presentation::Colors`; literal patterns are escaped and case-insensitive, invalid rules are visible, and invalid color saves are rejected. |
| Native record tools: inspect, copy, locate, JSON/TXT/CSV/BIN exports | `ui/terminal.rs`; direction-aware exports and original-sequence navigation. Workbench exports carry the clear cursor so cleared data cannot reappear. |
| Native protocol tools: validation, encoding preview, frame layout, RX decode and feedback | `ui/serial.rs`; use the shared protocol compiler and retain command history and per-session results. |
| `SerialWorkflowView.swift`, `SerialView.Workflow.cs`: packet library, file jobs, macros, replies and cancellation | Shared workbench requests; JSON import/export, packet validation, chunk/interval/cycle controls and macro CSV reports. |
| Native Modbus tools: encode-only versus send/wait | `serial.rs` must send **`timeout_ms`**, not `timeoutMs`. Regression tests check a disconnected send fails, a virtual transport emits the exact RTU request and another session remains responsive during the timeout. |
| Native capture tools: log/raw RX, rotation, paths and stop | Shared capture engine with a directory picker and explicit stop actions; UI shutdown/update checks include live captures. |
| `SerialWorkflowView.swift`, `SerialPlotWindow.cs`: numerical/KV/JSON/binary/template plots | `ui/plot.rs`, `ui/auxiliary.rs`; preview, field filtering, pause/clear, CSV/JSON export, gap count and continued background sampling in inactive sessions. Plot queues never occupy the serial send queue; revisions discard obsolete results. |
| Native settings and commands: theme, defaults and context-sensitive shortcuts | `ui/settings.rs`, `ui/mod.rs`, `main.rs`; separate new-session verify/reset defaults; refresh/connect/disconnect, save profile, execute, read memory, clear, find and settings shortcuts. |
| Native updates: source fallback, identity/signature/hash, cancellation and safe installation | `linux/update`; valid primary no-update ends the check; corrupted mirror bytes retry the same signed asset on GitHub; cancellation prevents retry; save failure blocks installation; portable Windows release names are accepted. |
| Native release verification and COS reservation | Existing `xtask --platform gpui` verification and downgrade protection remain shared; no release or mirror mutation is performed by these host checks. |

The audit adds only logic tests, including virtual serial transport, CSV escaping, rule semantics, configuration preservation, default-session routing and update fallback. `ui/highlights.rs` uses the exact `ropey` version already used by GPUI Kit's public highlighter interface. Do not add GUI automation tests. Actual window layout, keyboard dispatch, dialogs, native Windows driver execution and physical flashing remain manual/platform acceptance tasks. Launching the application lists serial ports but does not scan debug probes until the user requests it, matching the macOS client; obtain the explicit permissions in Verification before GUI acceptance.


### GPUI visual correction (2026-11-10)

- Use the macOS workbench layout: a unified title bar with two main modes; a firmware document above target/options/actions; a serial connection sidebar beside the terminal/composer split. Secondary workflows open in dialogs; memory, logs, settings and per-session plots use auxiliary windows.
- `ui/chrome.rs`, `target.rs`, `connection.rs`, `composer.rs` and `terminal_table.rs` own those surfaces. The terminal table shares the filtered, paused snapshot with the selectable text view. Keep multiline drafts and command mode per session. A change to target/probe/protocol/speed/reset settings invalidates the identified-target state; disconnecting does not discard firmware.
- Compare against the macOS app built from the current checkout, not an older installed release. Manual comparison resumed on 2026-11-10 and remains in progress; builds and logic checks do not establish full visual acceptance. Do not introduce GUI automation tests.
- Auxiliary window creation must be deferred until the current `Workbench` update has ended: GPUI draws a new window synchronously, and its owner access otherwise panics with `cannot update Workbench while it is already being updated`. Manual keyboard regression covered settings (including close/reopen), logs/search, memory, configuration dialog cancellation and the file chooser. Hardware shortcuts require disconnected/busy/modal guards; do not trigger physical operations as part of visual acceptance.

- macOS-parity primitives live in `ui/components.rs`: `glass` (toolbar capsule), `toolbar_icon`, `bordered` (24 pt regular push button), `prominent` (28 pt accent capsule), `borderless`, `mac_checkbox`, `segmented`, `form_popup` (grouped-form trailing picker), `popup_sized(.., small)` (20 pt small control) and `dashed_outline`. Reuse them instead of styling kit buttons ad hoc. Kit button labels inherit `text_sm` (14 px); pass the label as a 13 pt child as these helpers do. Icon-only `ghost().size(..)` buttons need `.p_0()` and a `flex_shrink_0()` icon, otherwise the icon is squeezed to a sliver.
- Dialogs are centered like macOS 26 sheets (`.centered()`). Do not recolor `theme.accent`: kit ghost-button hovers use `accent_foreground`.
- For visual comparison, run both apps at the same window size and capture with `screencapture -l <window id>`. GPUI dropdowns ignore instantaneous synthetic clicks; post CGEvent mouse down/up with a short delay.

- Before opening a top-level dialog from a popup menu, focus the persistent workbench handle. Otherwise the dialog can retain the popup focus handle, which disappears on dismissal and disables subsequent keyboard/menu actions. Preserve the parent dialog focus for nested dialogs. Manual regression also covered tool-menu dialog dismissal followed by Command-J, and cross-window Settings/Logs/Memory/mode switching.
