#!/usr/bin/env bash
set -euo pipefail
umask 022

desktop_repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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
  local app_dir
  app_dir="$(dirname "$binary")"
  mkdir -p "$prefix/lib/chipdock"
  cp -R "$app_dir/." "$prefix/lib/chipdock/"
  printf 'Update with the system package manager.\n' > "$prefix/lib/chipdock/package-managed"
  ln -s ../lib/chipdock/ChipDock "$prefix/bin/ChipDock"
  install -m 644 "$desktop_repo/linux/ChipDock.desktop" "$prefix/share/applications/$linux_app_id.desktop"
  install -m 644 "$desktop_repo/windows/ChipDock/Assets/ChipDock.png" "$prefix/share/icons/hicolor/256x256/apps/$linux_app_id.png"
  install -m 644 "$desktop_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$prefix/share/icons/hicolor/scalable/apps/$linux_app_id.svg"
  install -m 644 "$desktop_repo/LICENSE-MIT" "$desktop_repo/LICENSE-APACHE" "$desktop_repo/NOTICE" "$prefix/share/doc/chipdock/"
}

package_linux() {
  local binary="$1" version="$2" arch="$3" output="$4" work="$5"
  local name="ChipDock-$version-linux-$arch" deb_arch dependencies payload top
  payload="$work/payload"
  top="$work/rpm"
  case "$arch" in x86_64) deb_arch=amd64 ;; aarch64) deb_arch=arm64 ;; esac
  local portable="$work/ChipDock"
  mkdir -p "$portable"
  cp -R "$(dirname "$binary")/." "$portable/"
  install -m 755 "$desktop_repo/linux/install.sh" "$portable/install.sh"
  install -m 644 "$desktop_repo/linux/ChipDock.desktop" "$portable/$linux_app_id.desktop"
  install -m 644 "$desktop_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$portable/$linux_app_id.svg"
  install -m 644 "$desktop_repo/linux/README.md" "$portable/README.md"
  install -m 644 "$desktop_repo/LICENSE-MIT" "$desktop_repo/LICENSE-APACHE" "$desktop_repo/NOTICE" "$portable/"
  tar --sort=name --mtime="@${SOURCE_DATE_EPOCH:-0}" --owner=0 --group=0 --numeric-owner \
    -cf - -C "$work" ChipDock | gzip -n > "$output/$name.tar.gz"
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
  dependencies="$(cd "$work" && dpkg-shlibdeps -O "$payload/usr/lib/chipdock/ChipDock")"
  dependencies="$(sed -n 's/^shlibs:Depends=//p' <<< "$dependencies")"
  test -n "$dependencies"
  # GTK 4.14 is the NSH API baseline; media plugins are discovered at runtime.
  printf 'Depends: %s, libgtk-4-1 (>= 4.14), gstreamer1.0-plugins-base, hicolor-icon-theme\n' "$dependencies" >> "$payload/DEBIAN/control"
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
Requires: gtk4 >= 4.14
Requires: gstreamer1-plugins-base
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
/usr/lib/chipdock/
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

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  [[ $# == 4 && "$(uname -s)" == Linux ]]
  binary="$1/ChipDock"
  validate_binary "$binary" linux "$3"
  test -s "$1/nsh/bundle.nshc"
  test -s "$1/nsh/assets/nsh-updater.json"
  python3 - "$1" "$2" "$desktop_repo/linux/nsh.json" <<'PYINFO'
import json, sys
from pathlib import Path
app, version, config = sys.argv[1:]
resources = Path(app) / "nsh"
package = json.loads((resources / "app.json").read_text())
updater = json.loads((resources / "assets/nsh-updater.json").read_text())
source = json.loads(Path(config).read_text())
if any(doc["id"] != source["id"] or doc["version"] != version for doc in (package, updater)):
    raise SystemExit("NSH package ID or version mismatch")
if any(updater.get(key) != value for key, value in source["updater"].items()):
    raise SystemExit("NSH update source or signing key mismatch")
PYINFO
  mkdir -p "$4"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  package_linux "$binary" "$2" "$3" "$(cd "$4" && pwd)" "$work"
fi
