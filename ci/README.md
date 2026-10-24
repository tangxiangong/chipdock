# Build and release automation

Native projects own compilation and binding generation. CI checks the same build
graphs on macOS arm64/x86_64 and Windows x64/ARM64 from checkouts without generated
bindings. Rust caches do not contain committed Swift or C# declarations.

- `.github/workflows/rust.yml` checks the shared core, application service and
  release-note logic without hardware.
- `.github/workflows/macos.yml` builds the Xcode project and checks Swift/UniFFI
  integration, update verification and serial logic on macOS.
- `.github/workflows/windows.yml` builds the WinUI solution and checks the
  Interoptopus exports, C# bridge and update verification on Windows.
- `release-macos.sh` packages, verifies and publishes the macOS release.
- `release-windows.ps1` signs, verifies and publishes the Windows packages built by
  `windows/scripts/publish.ps1`.
- `verify-macos-update.swift` verifies the Sparkle archive and signed metadata.
- `xtask` verifies immutable release assets and publishes the COS mirror.

Publishing uses the existing release environment secrets and is triggered by a
published stable release. Ordinary source pushes perform validation only.
