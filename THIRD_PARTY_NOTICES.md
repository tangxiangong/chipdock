# Third-party Notices

FlashDesk uses Apple SwiftUI, AppKit and system-provided SF Symbols. Interface symbols are resolved by macOS at runtime.

[Sparkle 2.10.0](https://github.com/sparkle-project/Sparkle/tree/2.10.0) provides native software updates. Its MIT license and bundled third-party notices are reproduced in [Sparkle.txt](macos/FlashDesk/Resources/Licenses/Sparkle.txt), which is included in the application resources.

The Windows client uses the Microsoft Windows App SDK (WinUI 3) and Windows SDK build tools from NuGet, and system-provided Segoe Fluent Icons. On first launch it may download the Microsoft Windows App Runtime installer, which is verified against a pinned SHA-256 before installation. Its Rust bindings are generated with [Interoptopus](https://github.com/ralfbiedert/interoptopus) 0.16. Installers and updates use [Velopack](https://github.com/velopack/velopack) 1.2 (MIT); its runtime library ships with the application.

The Rust Core uses probe-rs, UniFFI, serde, serde_json, chrono, uuid, thiserror, object, ihex and tempfile, plus their transitive dependencies. Versions and sources are recorded in Cargo.lock. Each package retains its own license; metadata and license texts are available in the crates.io source packages downloaded by Cargo.

The FlashDesk application icon is supplied by the project owner in Figma and packaged using Apple Icon Composer. FlashDesk is licensed under MIT OR Apache-2.0.
