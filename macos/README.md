# ChipDock for macOS

Open `ChipDock.xcodeproj`, select the ChipDock scheme and build with Xcode 26 or
later on macOS 14 or later. Install stable Rust using rustup first.

```text
macos/
  ChipDock.xcodeproj/       Native project and RustBindings dependency
  ChipDock/
    Sources/ChipDock/       SwiftUI/AppKit app and bundled resources
    Tests/                 Existing integration and standalone logic checks
  Info.plist               Application metadata
  build.sh                 CLI build through the Xcode project
  build-bindings.sh        Xcode-owned Rust/UniFFI build step
```

`RustBindings` builds `chipdock-ffi` and generates the Swift source, C header and
module map before Swift dependency scanning starts. Outputs are inside DerivedData
and are not source files in Git. A clean checkout builds directly in Xcode.

The CLI uses the same project and does not launch the application:

```sh
CONFIGURATION=Debug bash macos/build.sh
just build arm64 debug
bash macos/ChipDock/Tests/run-interop-tests.sh
```

Set `ARCH=universal` to build both macOS architectures. `BUILD_ONLY=0` copies the
built app into `dist/macos/<architecture>`; release signing and publication belong
to `ci/release-macos.sh` and the release workflow.

`bash macos/run.sh` preserves the Run/debug/log entry point for an explicit app
launch. Builds and CI use `build.sh` directly.
