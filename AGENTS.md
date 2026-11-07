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
3. The workflows publish the versioned installers and signed update metadata, advance the GitHub `chipdock-updates` feed, then mirror the verified release to COS. All jobs that advance feeds share the `chipdock-updates` concurrency group. Keep `appcast.xml` and each `releases.win-<arch>.signed.json` with the versioned assets for verification and recovery.

- **macOS.** Keep `com.xiaoyu.chipdock` and `ChipDock.app` as the installation identity. `ci/release-macos.sh` packages the universal app, generates the Sparkle Ed25519 appcast, and verifies the signed archive. Local packaging reads `.build/Release/Build/Products/Release/ChipDock.app` by default and writes `.build/ReleaseAssets/`. Set `APP_BUNDLE` and `SPARKLE_BIN` to use other build output paths. Local signing reads the private key from the environment or the `com.xiaoyu.chipdock` login keychain account. CI runs the script with `--publish`.
- **Windows.** Keep Velopack package ID `ChipDock`, executable `ChipDock.exe`, and channels `win-x64` and `win-arm64`. Each Windows runner uses `windows/scripts/publish.ps1` to build EXE, MSI, and full `.nupkg` packages. `ci/release-windows.ps1` signs and verifies feed metadata using `windows/UpdateTool`, reuses verified signatures on retries, and checks package identity, size, and SHA-256. Coordinate signing key rotation with the public key in `ChipDock.csproj` and the `WINDOWS_UPDATE_PRIVATE_KEY` environment secret.
- Run `cargo test --locked -p xtask` for local release tool tests. Use `cargo run --locked -p xtask -- verify-release --tag "v<version>"` to download and verify GitHub assets. Add `--platform macos` or `--platform windows` to verify just one platform.

### COS mirror

- Both clients prefer Tencent Cloud COS and fall back to GitHub on eligible feed or download failures. End the check when a valid primary feed reports no update. macOS retries once on transport failure and preserves the manual or background mode. Windows preserves the selected version, filename, size, and SHA-256 on package fallback. Stop on user cancellation.
- Read the current endpoints and cache policies in `xtask/src/cos.rs`. Enable global acceleration for uploads and use the public HTTPS endpoint for client downloads. Scope the publisher's `cos:HeadObject`, `cos:PutObject`, and `cos:PutObjectACL` permissions to `chipdock/*`. The versioned files live under `chipdock/releases/v<version>/` and the live feeds under `chipdock/updates/`.
- `xtask` verifies GitHub digests and update signatures, then checks public object bytes before promoting feeds. The COS appcast changes only the download URL. Both Windows sources use the same signed metadata. Normal publishing rejects conflicts with immutable assets. Preserve `updates/state.json` and all assets still referenced by a feed. The state file reserves every pending channel version before its live feed moves, preventing an interrupted release or old retry from downgrading the feed.
- For a failed mirror, run [Publish existing release to COS](.github/workflows/publish-cos.yml) with an existing stable tag for the current installation identity and platform `all`, `macos`, or `windows`. Ordinary recovery runs `xtask publish-cos` with the published installers and signed feeds. Enable `replace_existing` only after the maintainer explicitly replaces the corresponding GitHub assets. That path calls `xtask replace-cos-release` and requires a valid state file while retaining feed downgrade protection.

### Platform-specific release notes

`crates/chipdock-release-notes` selects common text and platform blocks from the GitHub Release body. Use standalone opening markers such as `<!-- platform:macos -->` and the standalone closing marker `<!-- /platform -->`. Valid selectors are `macos`, `windows`, `linux`, and `all`. Use comma-separated platform names or `all` by itself. Put platform headings inside their blocks, keep blocks separate and closed, and include text for both shipped platforms. The parser validates every block.

Preview the selected text locally. For local packaging, pass the complete source via `RELEASE_NOTES_FILE` on macOS or `publish.ps1 -ReleaseNotes` on Windows.

```sh
cargo run --locked -p chipdock-release-notes -- macos ReleaseNotes.md .build/notes-macos.md
cargo run --locked -p chipdock-release-notes -- windows ReleaseNotes.md .build/notes-windows.md
```
