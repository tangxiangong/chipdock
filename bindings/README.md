# Rust bindings

ChipDock follows the same native project layout as BibCiTeX. Application operations
live in `crates/chipdock-service`, domain and transport primitives in
`crates/chipdock-core`, Swift exports in `crates/chipdock-ffi`, and Windows exports
in `crates/chipdock-csharp`.

```text
bindings/
  generator/          UniFFI CLI (`chipdock-bindgen`)
  csharp-generator/   Interoptopus generator (`chipdock-csharp-bindgen`)
  generate-swift.sh   Generate Swift/C bindings from a built Rust library
  generated/         Local outputs, ignored by Git
    swift/
    csharp/
```

## Build integration

Open the platform project and build directly. A clean checkout does not contain
generated declarations and needs no preparatory code-generation command.

Xcode's `RustBindings` dependency invokes `macos/build-bindings.sh` before the app
compiles. It builds `chipdock-ffi`, runs the official UniFFI generator, and writes
Swift, headers, the module map and the static library under
`$(BUILD_DIR)/ChipDockBindings/$(CONFIGURATION)`. Both Debug and Release use the
same checked-in Xcode project; no XcodeGen step is required.

MSBuild's `windows/ChipDock/Build/Rust.targets` builds `chipdock-csharp`, runs
`chipdock-csharp-bindgen`, then adds `bindings/generated/csharp/*.cs` to compilation.
It evaluates generated Compile items after generation so files missing when the
project was loaded are included in the first build. IntelliSense uses the last
generated files without launching Cargo during design-time builds.

Both generators and runtimes use the versions fixed in `Cargo.lock`. Generated
files are consumed unchanged and are never committed. CI verifies clean-checkout
generation on each platform and requires `bindings/generated` to remain untracked.

## Standalone generation

For non-GUI bridge checks, `macos/ChipDock/Tests/run-interop-tests.sh` builds the
Swift library and generates `bindings/generated/swift` automatically. To generate
only declarations from an existing library:

```sh
bash bindings/generate-swift.sh target/debug/libchipdock_ffi.dylib
cargo run --locked -p chipdock-csharp-bindgen
```

Swift imports `ChipDockCore` / `ChipDockCoreFFI`; C# uses namespace `ChipDock.Core`
and loads `chipdock_csharp.dll`. The shared service owns process-wide hardware
exclusion and the separate serial runtime. Splitting the binding crates does not
create another hardware or serial implementation.
