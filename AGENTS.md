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
- Client workflows filter push and pull-request changes by their source, build and release dependencies. Changes confined to `linux/` run the cross-platform NSH workflow and skip the independent native SwiftUI and WinUI workflows. NSH retains macOS/Windows compatibility checks and Linux packaging. Shared Cargo manifests, lockfile and core changes still trigger client validation. Release and manual triggers remain unfiltered.
- Each workflow's push and pull-request path filters include only its own file under `.github/workflows/`. A workflow-only change must trigger exactly the workflows edited in that commit, without unrelated CI runs. Source and shared dependency changes additionally trigger their owning workflows.

## Releases

1. Synchronize the workspace version in `Cargo.toml`, `MARKETING_VERSION` in `macos/ChipDock.xcodeproj/project.pbxproj`, and `Version` in `windows/ChipDock/ChipDock.csproj`.
2. Publishing a stable GitHub Release with a matching `v<version>` tag triggers the platform workflows. They run their validation jobs before publishing. The GitHub `release` environment supplies `SPARKLE_PRIVATE_ED_KEY`, `WINDOWS_UPDATE_PRIVATE_KEY`, `COS_SECRET_ID`, and `COS_SECRET_KEY`. The clients embed only the update public keys.
3. The workflows publish the macOS, Windows and Linux installers and signed update metadata, advance the GitHub `chipdock-updates` feed, then mirror the verified release to COS. All jobs that advance feeds share the `chipdock-updates` concurrency group. Keep `appcast.xml`, each `releases.win-<arch>.signed.json` and Linux `latest.json` with `latest.json.sig` with the versioned assets for verification and recovery.

- **macOS.** Keep `com.xiaoyu.chipdock` and `ChipDock.app` as the installation identity. `ci/release-macos.sh` packages the universal app, generates the Sparkle Ed25519 appcast, and verifies the signed archive. Local packaging reads `.build/Release/Build/Products/Release/ChipDock.app` by default and writes `.build/ReleaseAssets/`. Set `APP_BUNDLE` and `SPARKLE_BIN` to use other build output paths. Local signing reads the private key from the environment or the `com.xiaoyu.chipdock` login keychain account. CI runs the script with `--publish`.
- **Windows.** Keep Velopack package ID `ChipDock`, executable `ChipDock.exe`, and channels `win-x64` and `win-arm64`. Each Windows runner uses `windows/scripts/publish.ps1` to build EXE, MSI, and full `.nupkg` packages. `ci/release-windows.ps1` signs and verifies feed metadata using `windows/UpdateTool`, reuses verified signatures on retries, and checks package identity, size, and SHA-256. Coordinate signing key rotation with the public key in `ChipDock.csproj` and the `WINDOWS_UPDATE_PRIVATE_KEY` environment secret.
- Run `cargo test --locked -p xtask` for local release tool tests. Use `cargo run --locked -p xtask -- verify-release --tag "v<version>"` to download and verify GitHub assets. Add `--platform macos` or `--platform windows` to verify just one platform.

### COS mirror

- Both clients prefer Tencent Cloud COS and fall back to GitHub on eligible feed or download failures. End the check when a valid primary feed reports no update. macOS retries once on transport failure and preserves the manual or background mode. Windows preserves the selected version, filename, size, and SHA-256 on package fallback. Stop on user cancellation.
- Read the current endpoints and cache policies in `xtask/src/cos.rs`. Enable global acceleration for uploads and use the public HTTPS endpoint for client downloads. Scope the publisher's `cos:HeadObject`, `cos:PutObject`, and `cos:PutObjectACL` permissions to `chipdock/*`. The versioned files live under `chipdock/releases/v<version>/` and the live feeds under `chipdock/updates/`.
- `xtask` verifies GitHub digests and update signatures, then checks public object bytes before promoting feeds. The COS appcast changes only the download URL. Both Windows sources use the same signed metadata. Normal publishing rejects conflicts with immutable assets. Preserve `updates/state.json` and all assets still referenced by a feed. The state file reserves every pending channel version before its live feed moves, preventing an interrupted release or old retry from downgrading the feed.
- For a failed mirror, run [Publish existing release to COS](.github/workflows/publish-cos.yml) with an existing stable tag for the current installation identity and platform `all`, `macos`, `windows`, or `linux` (`gpui` is an alias). Ordinary recovery runs `xtask publish-cos` with the published installers and signed feeds. Enable `replace_existing` only after the maintainer explicitly replaces the corresponding GitHub assets. That path calls `xtask replace-cos-release` and requires a valid state file while retaining feed downgrade protection.
- For an explicitly requested same-tag re-release, set the `release` environment variable `CHIPDOCK_REPUBLISH_TAG` to that exact tag before publishing. The Windows release tool then permits a verified same-version replacement, and the native workflows use `replace-cos-release` for the regenerated assets. Clear the variable after all platforms finish. Downgrades remain rejected.

### Platform-specific release notes

`crates/chipdock-release-notes` selects common text and platform blocks from the GitHub Release body. Use standalone opening markers such as `<!-- platform:macos -->` and the standalone closing marker `<!-- /platform -->`. Valid selectors are `macos`, `windows`, `linux`, and `all`. Use comma-separated platform names or `all` by itself. Put platform headings inside their blocks, keep blocks separate and closed, and include text for macOS, Windows and Linux. The parser validates every block.

Preview the selected text locally. For local packaging, pass the complete source via `RELEASE_NOTES_FILE` on macOS or `publish.ps1 -ReleaseNotes` on Windows.

```sh
cargo run --locked -p chipdock-release-notes -- macos ReleaseNotes.md .build/notes-macos.md
cargo run --locked -p chipdock-release-notes -- windows ReleaseNotes.md .build/notes-windows.md
cargo run --locked -p chipdock-release-notes -- linux ReleaseNotes.md .build/notes-linux.md
```


## NSH client

- `linux/` is the cross-platform mixed Rust/TSX NSH client, published as the Linux frontend. The standard NSH runner loads `App.tsx` and the `chipdock/native` Rust module. Pages and dialog composition live in `linux/pages/` and `linux/components/`. For local work, use the CLI matching the resolved NSH crate version. CI resolves the latest stable NSH release once per run and verifies the published SHA-256 for each CLI archive.
- `src/ui/` contains the retained NSH views and app-owned controls. `workbench.rs` owns persistent preferences, sessions, result reconciliation and polling; `backend.rs` dispatches probe, per-session serial, plot, file and update work on independent workers. `model.rs`, `presentation.rs` and `serial.rs` retain shared validation, storage, filtering and request semantics. `platform/windows/` uses typed `windows` SetupAPI and WinUSB calls, without a .NET helper.
- `ui/commands.rs` derives native menu state before installing handlers; unrelated polling must not dispose active menu handlers. App-owned file tasks use the workbench owner so menu refreshes cannot cancel them. `ui/commands_macos.rs` registers NSH menus with AppKit's system roles; `ui/file_dialogs.rs` preserves the native standalone panels and document sheets through public AppKit APIs. Other platforms keep NSH file dialogs. These bridges run in the desktop process.
- `ui/tool_feedback.rs` owns each tool dialog's result/error state. Consume per-session `OperationResult` with the captured session, request action and revision; never infer dialog completion from the global activity message. Diagnostic results keep the native bounded GroupBox and workflow outcomes keep their native labels.
- Preserve the tuned GPUI layout while translating framework APIs, and use the macOS client built from the current checkout as the visual reference. Do not redesign or reorganize an existing surface during the framework migration. Keep exact control geometry, fonts, line heights, spacing, menu placement, dialog composition and keyboard semantics. Check **every** screen and dialog at the same window size, appearance and content state with paired screenshots and pixel difference images. Record unverified surfaces; builds and logic tests never establish visual acceptance. Do not add GUI automation tests.
- UI foundations live in `ui/components.rs`, with the original SVG icon family in `ui/icons.rs`. Keep body text at 13 pt, caption text at 12 pt and the original 1.2 line-height ratio; the established 24 pt push buttons, 28 pt accent capsules and 20 pt small controls are migration constraints. Feature pages compose those shared controls. Use NSH semantic components for keyboard, accessibility, focus, menus, dialogs, resizable panes, virtualized data and plots. Keep theme roles and motion policy in `theme.json` and `DESIGN.md`.
- SVG icons are compiled into the executable through NSH's inline SVG image source. The portable updater replaces the packaged application directory, including its bytecode and resources. Preserve native application icons in packaged desktop resources.
- For isolated manual acceptance, set `CHIPDOCK_PREFERENCES_PATH` to an absolute JSON file path. NSH manages its own window state. Keep `HOME` unchanged; normal launches retain the historical default preference location.
- `src/lib.rs` exports the mixed app’s native module. The workbench host initializes desktop ownership and driver handoffs. Never enumerate debug probes automatically at startup. Launching may list serial ports; device access and GUI control still require the explicit authorization in Verification.

### Native builds and NSH CLI

Run from the repository root, using the user's installed `cargo` (including the shared-cache Cargo when installed):

```sh
nsh check linux
nsh build linux -o .build/NSH
cargo clippy --locked -p chipdock-nsh -p xtask --all-targets -- -D warnings
cargo test --locked -p chipdock-nsh --all-targets
cargo test --locked -p xtask
bash linux/tests/packaging.sh
shellcheck linux/*.sh linux/tests/*.sh
```

- Use `nsh run linux` for an explicitly authorized native GUI session. Choose `nsh package` targets only on their native hosts. NSH CLI packages do not replace the release-specific DEB/RPM/feed publishing pipeline below. Do not use `nsh test` to introduce GUI automation tests; existing Rust logic tests and manual acceptance remain the project policy.
- macOS NSH Cargo builds and bundle packaging require Python 3 and the full Xcode 26+ installation with `actool`. `linux/build-macos-assets.sh` compiles the canonical `assets/app-icons/ChipDock.icon` directly, independently of the SwiftUI app, into `Assets.car`, `ChipDock.icns`, icon plist metadata and the largest legacy PNG for CLI input. `linux/build.rs` invokes this generator for macOS into Cargo's `OUT_DIR`, tracks the icon source/helper/developer-directory changes and exports `CHIPDOCK_MACOS_ICON_PNG` for compile-time inclusion in inline macOS icon views. Cargo caches the build-script output; no asset compiler or helper is needed at runtime. The asset deployment floor mirrors the native icon target (14.0); the NSH executable and bundle still require macOS 15+. `linux/build.sh` installs both compiled resources and the generated `CFBundleIconName`/`CFBundleIconFile` before signing. Do not derive macOS icons from the Windows PNG or compensate for incorrect source artwork by shrinking alert icons or clearing OS caches.
- The shared `nsh.json` PNG remains the Windows/Linux input. For NSH CLI macOS package verification, override it with the generated canonical PNG, then install the full catalog and native icon metadata and re-sign the local bundle:

  ```sh
  bash linux/build-macos-assets.sh .build/NSHMacAssets
  python3 linux/ota/entry.py desktop .build/NSHDesktopEntry
  nsh package .build/NSHDesktopEntry --target macos --icon .build/NSHMacAssets/ChipDock.png -o .build/NSHPackage
  bash linux/build-macos-assets.sh .build/NSHMacAssets .build/NSHPackage/ChipDock.app
  codesign --force --sign - .build/NSHPackage/ChipDock.app
  codesign --verify --strict .build/NSHPackage/ChipDock.app
  ```
- Run `bash linux/build.sh` on the target platform. NSH packages the runner, bundle and resources together. Linux then stages the entire app for DEB, RPM and the user-installable archive. The default output directory is `dist/gpui`. The macOS source package uses the shared NSH identity and retains its canonical icons.
- Ubuntu 24.04 native build packages: `python3 pkg-config clang cmake libudev-dev libgtk-4-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libssl-dev`. Packaging also requires `dpkg-dev rpm cpio desktop-file-utils shellcheck`. The Linux backend requires GTK 4.14 and links GStreamer, GStreamer App and GStreamer Audio even with the app's optional UI features reduced. Use a graphical Wayland or X11 session for Linux runtime acceptance. A Vulkan driver is no longer an application requirement.
- DEB uses `dpkg-shlibdeps`; RPM keeps automatic ELF dependencies. Both additionally require GTK >= 4.14, GStreamer base plugins and the system icon theme. Both install under `/usr` with a package-managed marker; updates must use the package manager. Portable archives retain their user installer.
- `.github/workflows/nsh.yml` validates the cross-platform NSH client: it builds Linux x86_64 and aarch64 packages and checks the shared source on native macOS and Windows runners. It validates extracted package metadata, GTK/GStreamer requirements, desktop resources and exact runner and bytecode bytes without installing packages, launching the GUI or accessing devices. Local host checks are not CI, installer, other-platform, GUI or hardware acceptance.
- `bash linux/tests/packaging.sh` covers architecture headers, user installation paths and packaging of the standard NSH app directory without building or launching the app. On Linux add `<asset-directory> <x86_64|aarch64>` to verify real DEB/RPM/tarball payloads.

### Migration acceptance requirements

Review each route against the current macOS native interface and preserve its behavior:

| Surface | Owning code / invariants |
| --- | --- |
| Title bar, modes, menus, keyboard commands | `ui/chrome.rs`, `ui/mod.rs`, `ui/commands.rs`, `ui/commands_macos.rs`; original toolbar geometry, native menu roles and context-sensitive modal/busy/disconnected guards. Menu construction subscribes only to the computed menu snapshot; async file work starts with stable `Store.spawn` ownership. |
| Firmware document, recent files, target setup and flash actions | `ui/flash.rs`, `ui/target.rs`, `model.rs`, `workbench.rs`; offline firmware analysis, retained BIN addresses, stale-result rejection, captured-target erase confirmation and identification invalidation. |
| Connection rail, sessions, terminal and composer | `ui/serial.rs`, `workbench.rs`, `backend.rs`; actual default session 1, per-session drafts/history, independent lanes, bounded records, paused snapshots, stable clear cursor and reconnect/IPC configuration reconciliation. |
| Protocol, workflow and diagnostic dialogs | `ui/protocol.rs`, `ui/tools.rs`, `ui/tool_feedback.rs`, `ui/colors.rs`, `ui/records.rs`; structured rule controls, validation, offline preview/layout/decode, packet/file/macro/reply flows, capture cancellation, record selection, sorting, original-sequence navigation and exports. |
| Memory, logs, settings and per-session plot windows | `ui/auxiliary.rs` and its view modules; preserve independent windows, exact native composition, file dialogs, local state/focus, per-session plot ownership, background sampling, revisions and CSV/JSON export. |
| Shared protocol and display semantics | `serial.rs`, `presentation.rs`; Modbus uses `timeout_ms`, filtering and colors agree across views, literal rules are escaped, CSV is escaped, invalid colors and regexes remain visible. |
| Update checks and installation | `App.tsx`, `src/update.rs`, `workbench.rs`; primary no-update ends the check, fallback retains signed identity/version/hash, cancellation blocks retries, active serial work blocks installation, and failed preference saves block restart. |

Re-run native-host logic checks after changes. GUI, focus, keyboard dispatch, light/dark appearance, accessibility, installer behavior and hardware acceptance are distinct checks. Do not trigger physical operations while collecting pixel comparisons.

### Release compatibility

- Linux uses NSH whole-app updates with the existing Ed25519 trust key. The version in nsh.json must match the release tag. Release CI runs nsh update-manifest over both portable archives. GitHub publishes latest.json, latest.json.sig and the packages. The COS mirror verifies the signed manifest and archive hashes before advancing the live feed.
- DEB and RPM distribute the runner alongside its bytecode and assets under /usr/lib/chipdock. Preserve the package IDs, desktop ID and historical preferences. Portable updates replace the application directory as a unit.

### NSH OTA interface packages

The mixed client checks its signed interface-package feed automatically after the main workbench opens and once per day while running. NSH stages the verified package in the user's app data, tries it on the next normal launch, and manages health confirmation and rollback. Existing installations need a shell containing the OTA trust root and checker before they can receive packages.

Use the NSH CLI version pinned to the client. Desktop and Linux share the application ID in `linux/nsh.json` and one OTA feed per ABI. Install the Python preflight dependency, then check a candidate build without signing or producing output:

```sh
python3 -m pip install -r linux/ota/requirements.txt
python3 linux/ota/build.py 6 dist/ota-build-6 --dry-run
```

For a confirmed first publication only, add `--first-release`. A missing manifest on an existing branch is an error unless a fresh ABI is explicitly requested with `--new-abi` and the shipped build exceeds previous ABI packages. After reviewing native compatibility, remove `--dry-run` and add `--abi-reviewed` to build locally. The preflight checks the signed manifest, package and reachable numbered package history. Keep immutable packages and other ABI directories; publish only in one fast-forward commit based on the reported branch head, after rechecking that head. A package-only publication leaves the application version and its release feeds intact. Run offline tests with `python3 -m unittest discover -s linux/ota -p 'test_*.py'`.

The local signing directory `.chipdock-ota-keys/` is Git-ignored. Back up the root private key offline. The build uses the certified release key for the package and its feed signature. A signing machine may pass `--key <release.secret>`. Renew the scoped certificate with `nsh certify` before it expires. Changing the feed signing key requires shipping the corresponding public key in the shell.

Keep native exports compatible within each ABI feed. For an incompatible native change, choose a fresh ABI feed path in `linux/nsh.json` and raise the shipped `build` above all packages previously published for the application ID so cached packages cannot override the new shell. `linux/shell-updater.json` owns the separate whole-app feed configuration.
