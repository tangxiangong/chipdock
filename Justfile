[unix]
set shell := ["bash", "-cu"]
[windows]
set shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

host_arch := if os() == "windows" { if arch() == "aarch64" { "ARM64" } else { "x64" } } else if arch() == "aarch64" { "arm64" } else { "x86_64" }

default:
    @just --list

# The IDE and CLI use the same native build graph.
[macos]
build architecture=host_arch profile="release":
    ARCH={{ quote(architecture) }} CONFIGURATION={{ quote(profile) }} BUILD_ONLY=1 bash macos/build.sh

[windows]
build architecture=host_arch profile="Release":
    & ./windows/scripts/build.ps1 -Architecture '{{ replace(architecture, "'", "''") }}' -Configuration '{{ replace(profile, "'", "''") }}'

[macos]
bundle architecture=host_arch profile="release":
    ARCH={{ quote(architecture) }} CONFIGURATION={{ quote(profile) }} BUILD_ONLY=0 bash macos/build.sh

fmt:
    cargo fmt --all -- --check

test-rust-shared:
    cargo test --locked -p chipdock-core -p chipdock-service -p chipdock-release-notes --all-targets

[macos]
test-macos:
    bash macos/ChipDock/Tests/run-interop-tests.sh

[macos]
test-updates:
    bash macos/ChipDock/Tests/run-update-tests.sh

[macos]
test-serial:
    bash macos/ChipDock/Tests/run-serial-tests.sh

[windows]
test-windows library:
    dotnet run --project windows/Tests/ChipDock.Tests.csproj -- '{{ replace(library, "'", "''") }}'
