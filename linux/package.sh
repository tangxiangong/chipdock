#!/usr/bin/env bash
set -euo pipefail
umask 022

gpui_repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
linux_app_id=com.xiaoyu.chipdock

validate_binary() {
  local header
  header="$(od -An -tx1 -N20 "$1" | tr -d ' \n')"
  case "$2-$3" in
    linux-x86_64) [[ "${header:0:12}" == 7f454c460201 && "${header:36:4}" == 3e00 ]] ;;
    linux-aarch64) [[ "${header:0:12}" == 7f454c460201 && "${header:36:4}" == b700 ]] ;;
    macos-x86_64) [[ "${header:0:16}" == cffaedfe07000001 ]] ;;
    macos-aarch64) [[ "${header:0:16}" == cffaedfe0c000001 ]] ;;
    *) return 1 ;;
  esac
}

stage_linux() {
  local prefix="$1" binary="$2"
  mkdir -p "$prefix/bin" "$prefix/share/applications" \
    "$prefix/share/icons/hicolor/256x256/apps" "$prefix/share/icons/hicolor/scalable/apps" \
    "$prefix/share/doc/chipdock"
  install -m 755 "$binary" "$prefix/bin/ChipDock"
  install -m 644 "$gpui_repo/linux/ChipDock.desktop" "$prefix/share/applications/$linux_app_id.desktop"
  install -m 644 "$gpui_repo/windows/ChipDock/Assets/ChipDock.png" "$prefix/share/icons/hicolor/256x256/apps/$linux_app_id.png"
  install -m 644 "$gpui_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$prefix/share/icons/hicolor/scalable/apps/$linux_app_id.svg"
  install -m 644 "$gpui_repo/LICENSE-MIT" "$gpui_repo/LICENSE-APACHE" "$gpui_repo/NOTICE" "$prefix/share/doc/chipdock/"
}

package_linux() {
  local binary="$1" version="$2" arch="$3" output="$4" work="$5"
  local name="ChipDock-$version-linux-$arch" deb_arch dependencies payload top
  payload="$work/payload"
  top="$work/rpm"
  case "$arch" in x86_64) deb_arch=amd64 ;; aarch64) deb_arch=arm64 ;; esac
  stage_linux "$work/$name" "$binary"
  install -m 755 "$gpui_repo/linux/install.sh" "$work/$name/install.sh"
  install -m 644 "$gpui_repo/linux/README.md" "$work/$name/README.md"
  tar --sort=name --mtime="@${SOURCE_DATE_EPOCH:-0}" --owner=0 --group=0 --numeric-owner \
    -cf - -C "$work" "$name" | gzip -n > "$output/$name.tar.gz"
  install -m 755 "$binary" "$output/$name"

  stage_linux "$payload/usr" "$binary"
  mkdir -p "$payload/usr/share/chipdock" "$payload/DEBIAN" "$work/debian" "$top"
  printf 'Update this installation with the system package manager.\n' > "$payload/usr/share/chipdock/package-managed"
  cat > "$payload/DEBIAN/control" <<EOF
Package: chipdock
Version: $version
Architecture: $deb_arch
Maintainer: tangxiangong <tangxiangong@gmail.com>
Section: devel
Priority: optional
Homepage: https://github.com/tangxiangong/chipdock
Provides: chipdock-gpui
Conflicts: chipdock-gpui
Replaces: chipdock-gpui
Description: ChipDock firmware programming and serial workbench
EOF
  cat > "$work/debian/control" <<EOF
Source: chipdock
Maintainer: tangxiangong <tangxiangong@gmail.com>

Package: chipdock
Architecture: any
Description: Firmware programming and serial workbench
EOF
  dependencies="$(cd "$work" && dpkg-shlibdeps -O "$payload/usr/bin/ChipDock")"
  dependencies="$(sed -n 's/^shlibs:Depends=//p' <<< "$dependencies")"
  test -n "$dependencies"
  # These libraries are also loaded dynamically and may be absent from shlibdeps output.
  printf 'Depends: %s, libvulkan1, libfontconfig1, libwayland-client0, libxkbcommon-x11-0, libx11-xcb1, hicolor-icon-theme\n' "$dependencies" >> "$payload/DEBIAN/control"
  (cd "$payload" && find usr -type f -print0 | LC_ALL=C sort -z | xargs -0 md5sum) > "$payload/DEBIAN/md5sums"
  dpkg-deb --root-owner-group --build "$payload" "$output/$name.deb"
  rm -r "$payload/DEBIAN"
  mkdir -p "$top/BUILD" "$top/BUILDROOT" "$top/RPMS" "$top/SOURCES" "$top/SPECS" "$top/SRPMS"

  cat > "$top/chipdock.spec" <<EOF
%global debug_package %{nil}
%global __os_install_post %{nil}
%global use_source_date_epoch_as_buildtime 1
%global clamp_mtime_to_source_date_epoch 1
Name: ChipDock
Version: $version
Release: 1
Summary: Firmware programming and serial workbench
License: MIT OR Apache-2.0
URL: https://github.com/tangxiangong/chipdock
BuildArch: $arch
Requires: libvulkan.so.1()(64bit), libfontconfig.so.1()(64bit)
Requires: libwayland-client.so.0()(64bit), libxkbcommon-x11.so.0()(64bit), libX11-xcb.so.1()(64bit)
Requires: hicolor-icon-theme
Provides: chipdock-gpui = %{version}-%{release}
Obsoletes: chipdock-gpui <= %{version}-%{release}

%description
ChipDock firmware programming and serial workbench.

%install
mkdir -p "%{buildroot}"
cp -a "\$RPM_PAYLOAD/usr" "%{buildroot}/usr"

%files
%attr(0755,root,root) /usr/bin/ChipDock
/usr/share/applications/$linux_app_id.desktop
/usr/share/icons/hicolor/256x256/apps/$linux_app_id.png
/usr/share/icons/hicolor/scalable/apps/$linux_app_id.svg
/usr/share/chipdock/
%dir /usr/share/doc/chipdock
%license /usr/share/doc/chipdock/LICENSE-MIT
%license /usr/share/doc/chipdock/LICENSE-APACHE
%doc /usr/share/doc/chipdock/NOTICE
EOF
  RPM_PAYLOAD="$payload" rpmbuild -bb --define "_topdir $top" --target "$arch" "$top/chipdock.spec"
  install -m 644 "$top/RPMS/$arch/ChipDock-$version-1.$arch.rpm" "$output/$name.rpm"
}

package_macos() {
  local binary="$1" version="$2" output="$3" work="$4"
  local bundle="$work/ChipDock.app" bundle_id=com.xiaoyu.chipdock.gpui size scale suffix pixels
  mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources" "$work/ChipDock.iconset"
  install -m 755 "$binary" "$bundle/Contents/MacOS/ChipDock"
  for size in 16 32 128 256 512; do
    for scale in 1 2; do
      suffix=""; [[ "$scale" == 1 ]] || suffix=@2x
      pixels=$((size * scale))
      sips -z "$pixels" "$pixels" "$gpui_repo/windows/ChipDock/Assets/ChipDock.png" \
        --out "$work/ChipDock.iconset/icon_${size}x$size$suffix.png" >/dev/null
    done
  done
  iconutil -c icns "$work/ChipDock.iconset" -o "$bundle/Contents/Resources/ChipDock.icns"
  cat > "$bundle/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>$bundle_id</string>
<key>CFBundleName</key><string>ChipDock</string>
<key>CFBundleDisplayName</key><string>ChipDock</string>
<key>CFBundleExecutable</key><string>ChipDock</string>
<key>CFBundleIconFile</key><string>ChipDock.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
  plutil -lint "$bundle/Contents/Info.plist"
  codesign --force --sign - "$bundle"
  codesign --verify --strict "$bundle"
  if [[ -e "$output/ChipDock.app" ]]; then
    [[ "$(plutil -extract CFBundleIdentifier raw "$output/ChipDock.app/Contents/Info.plist")" == "$bundle_id" ]] || {
      echo "Choose a separate output directory for this ChipDock build: $output" >&2
      return 1
    }
    rm -r "$output/ChipDock.app"
  fi
  cp -R "$bundle" "$output/"
}

package_gpui() {
  if [[ "$#" != 4 ]]; then
    echo 'Usage: package.sh <binary> <version> <target-triple> <output-directory>' >&2
    return 1
  fi
  local binary version="$2" platform arch output work
  [[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || {
    echo "Expected a canonical stable version: $version" >&2; return 1
  }
  binary="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
  case "$3" in
    x86_64-unknown-linux-gnu) platform=linux; arch=x86_64 ;;
    aarch64-unknown-linux-gnu) platform=linux; arch=aarch64 ;;
    x86_64-apple-darwin) platform=macos; arch=x86_64 ;;
    aarch64-apple-darwin) platform=macos; arch=aarch64 ;;
    *) echo "Unsupported package target: $3" >&2; return 1 ;;
  esac
  validate_binary "$binary" "$platform" "$arch" || { echo "Binary does not match $3" >&2; return 1; }
  mkdir -p "$4"
  output="$(cd "$4" && pwd)"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  if [[ "$platform" == linux ]]; then
    [[ "$(uname -s)" == Linux ]]
    package_linux "$binary" "$version" "$arch" "$output" "$work"
  else
    [[ "$(uname -s)" == Darwin ]]
    package_macos "$binary" "$version" "$output" "$work"
  fi
  rm -rf "$work"
  trap - EXIT
  echo "Packaged ChipDock $version for $platform-$arch: $output"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then package_gpui "$@"; fi
