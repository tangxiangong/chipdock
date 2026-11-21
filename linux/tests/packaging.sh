#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=linux/package.sh
source "$(dirname "$0")/../package.sh"
test_dir="$(mktemp -d)"
trap 'rm -rf "$test_dir"' EXIT
printf '\177ELF\002\001\001\000\000\000\000\000\000\000\000\000\002\000\076\000' > "$test_dir/binary"
validate_binary "$test_dir/binary" linux x86_64
if validate_binary "$test_dir/binary" linux aarch64; then exit 1; fi
if validate_binary "$test_dir/binary" macos x86_64; then exit 1; fi
app="$test_dir/ChipDock"
mkdir -p "$app/nsh/assets"
install -m 755 "$test_dir/binary" "$app/ChipDock"
printf 'bundle' > "$app/nsh/bundle.nshc"
printf '{"id":"com.xiaoyu.chipdock","version":"0.3.0"}' > "$app/nsh/app.json"
cp "$desktop_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$app/$linux_app_id.svg"
cp "$desktop_repo/linux/ChipDock.desktop" "$app/$linux_app_id.desktop"
cp "$desktop_repo/windows/ChipDock/Assets/ChipDock.png" "$app/$linux_app_id.png"
stage_linux "$test_dir/system" "$app/ChipDock"
cmp "$app/nsh/bundle.nshc" "$test_dir/system/lib/chipdock/nsh/bundle.nshc"
cmp "$app/ChipDock" "$test_dir/system/bin/ChipDock"
cp "$desktop_repo/linux/install.sh" "$app/install.sh"
install_home="$test_dir/user space"
HOME="$install_home" XDG_DATA_HOME="$test_dir/data" bash "$app/install.sh"
cmp "$app/ChipDock" "$install_home/.local/bin/ChipDock"
cmp "$app/nsh/bundle.nshc" "$install_home/.local/lib/chipdock/nsh/bundle.nshc"
grep -F "Exec=\"$install_home/.local/lib/chipdock/ChipDock\" %f" "$test_dir/data/applications/$linux_app_id.desktop"
if [[ "$#" == 0 ]]; then exit 0; fi
[[ "$#" == 2 && "$(uname -s)" == Linux ]]
output="$(cd "$1" && pwd)"
arch="$2"
package_id="$(cd "$desktop_repo" && cargo pkgid --locked -p chipdock-nsh)"
version="${package_id##*@}"
name="ChipDock-$version-linux-$arch"
case "$arch" in x86_64) deb_arch=amd64 ;; aarch64) deb_arch=arm64 ;; *) exit 1 ;; esac
test "$(dpkg-deb -f "$output/$name.deb" Package)" == chipdock
test "$(dpkg-deb -f "$output/$name.deb" Version)" == "$version"
test "$(dpkg-deb -f "$output/$name.deb" Architecture)" == "$deb_arch"
dpkg-deb -f "$output/$name.deb" Depends | grep -F 'libgtk-4-1 (>= 4.14)'
dpkg-deb -f "$output/$name.deb" Depends | grep -F gstreamer1.0-plugins-base
for field in Provides Conflicts Replaces; do
  test "$(dpkg-deb -f "$output/$name.deb" "$field")" == chipdock-gpui
done
test "$(rpm -qp --queryformat '%{NAME} %{VERSION} %{ARCH}' "$output/$name.rpm")" == "ChipDock $version $arch"
rpm -qp --requires "$output/$name.rpm" | grep -Fx 'gtk4 >= 4.14'
rpm -qp --requires "$output/$name.rpm" | grep -Fx gstreamer1-plugins-base
rpm -qp --obsoletes "$output/$name.rpm" | grep -Fx "chipdock-gpui <= $version-1"
dpkg-deb -x "$output/$name.deb" "$test_dir/deb"
mkdir "$test_dir/rpm" "$test_dir/archive"
rpm2cpio "$output/$name.rpm" | (cd "$test_dir/rpm" && cpio -id --quiet)
tar -xzf "$output/$name.tar.gz" -C "$test_dir/archive"
for prefix in "$test_dir/deb/usr" "$test_dir/rpm/usr"; do
  validate_binary "$prefix/lib/chipdock/ChipDock" linux "$arch"
  test -s "$prefix/lib/chipdock/package-managed"
  test -s "$prefix/lib/chipdock/nsh/assets/nsh-updater.json"
  test -s "$prefix/lib/chipdock/nsh/bundle.nshc"
  cmp "$test_dir/archive/ChipDock/ChipDock" "$prefix/bin/ChipDock"
  diff -r "$test_dir/archive/ChipDock/nsh" "$prefix/lib/chipdock/nsh"
  test -x "$prefix/bin/ChipDock"
  test ! -e "$prefix/bin/chipdock-gpui"
  desktop-file-validate "$prefix/share/applications/$linux_app_id.desktop"
  grep -Fx 'Exec=ChipDock %f' "$prefix/share/applications/$linux_app_id.desktop"
  cmp "$desktop_repo/windows/ChipDock/Assets/ChipDock.png" "$prefix/share/icons/hicolor/256x256/apps/$linux_app_id.png"
  cmp "$desktop_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$prefix/share/icons/hicolor/scalable/apps/$linux_app_id.svg"
  test -s "$prefix/share/doc/chipdock/LICENSE-MIT"
  test -s "$prefix/share/doc/chipdock/LICENSE-APACHE"
done
test -f "$test_dir/deb/usr/share/chipdock/package-managed"
test -f "$test_dir/rpm/usr/share/chipdock/package-managed"
test -x "$test_dir/archive/ChipDock/install.sh"
test ! -e "$test_dir/archive/ChipDock/package-managed"
cmp "$desktop_repo/linux/ChipDock.desktop" "$test_dir/archive/ChipDock/$linux_app_id.desktop"
echo "Verified Linux $arch DEB, RPM and portable payloads without installing or launching them"
