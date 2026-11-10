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
stage_linux "$test_dir/portable" "$test_dir/binary"
install -m 755 "$gpui_repo/linux/install.sh" "$test_dir/portable/install.sh"
install_home="$test_dir/user space \"quote\" \$dollar \`tick\` 100% \\slash"
HOME="$install_home" XDG_DATA_HOME="$test_dir/xdg data" bash "$test_dir/portable/install.sh"
cmp "$test_dir/binary" "$install_home/.local/bin/chipdock-gpui"
cmp "$gpui_repo/windows/ChipDock/Assets/ChipDock.png" "$test_dir/xdg data/icons/hicolor/256x256/apps/$gpui_app_id.png"
cmp "$gpui_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$test_dir/xdg data/icons/hicolor/scalable/apps/$gpui_app_id.svg"
desktop="$test_dir/xdg data/applications/$gpui_app_id.desktop"
grep -Fx "Icon=$gpui_app_id" "$desktop"
grep -Fx "StartupWMClass=$gpui_app_id" "$desktop"
grep -F '100%%' "$desktop"
grep -F '\\"quote\\"' "$desktop"
# shellcheck disable=SC2016
grep -F '\\$dollar' "$desktop"
if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$desktop"; fi
if HOME="$install_home" XDG_DATA_HOME=relative bash "$test_dir/portable/install.sh"; then exit 1; fi
echo 'Packaging and user installer logic passed'

if [[ "$#" == 0 ]]; then exit 0; fi
[[ "$#" == 2 && "$(uname -s)" == Linux ]]
output="$(cd "$1" && pwd)"
arch="$2"
package_id="$(cd "$gpui_repo" && cargo pkgid --locked -p chipdock-gpui)"
version="${package_id##*@}"
name="chipdock-gpui-$version-linux-$arch"
case "$arch" in x86_64) deb_arch=amd64 ;; aarch64) deb_arch=arm64 ;; *) exit 1 ;; esac
test "$(dpkg-deb -f "$output/$name.deb" Package)" == chipdock-gpui
test "$(dpkg-deb -f "$output/$name.deb" Version)" == "$version"
test "$(dpkg-deb -f "$output/$name.deb" Architecture)" == "$deb_arch"
dpkg-deb -f "$output/$name.deb" Depends | grep -F libvulkan1
test "$(rpm -qp --queryformat '%{NAME} %{VERSION} %{ARCH}' "$output/$name.rpm")" == "chipdock-gpui $version $arch"
rpm -qp --requires "$output/$name.rpm" | grep -F 'libvulkan.so.1()(64bit)'
dpkg-deb -x "$output/$name.deb" "$test_dir/deb"
mkdir "$test_dir/rpm" "$test_dir/archive"
rpm2cpio "$output/$name.rpm" | (cd "$test_dir/rpm" && cpio -id --quiet)
tar -xzf "$output/$name.tar.gz" -C "$test_dir/archive"
for prefix in "$test_dir/deb/usr" "$test_dir/rpm/usr" "$test_dir/archive/$name"; do
  cmp "$output/chipdock-gpui-linux-$arch" "$prefix/bin/chipdock-gpui"
  test -x "$prefix/bin/chipdock-gpui"
  desktop-file-validate "$prefix/share/applications/$gpui_app_id.desktop"
  cmp "$gpui_repo/windows/ChipDock/Assets/ChipDock.png" "$prefix/share/icons/hicolor/256x256/apps/$gpui_app_id.png"
  cmp "$gpui_repo/assets/app-icons/ChipDock.icon/Assets/AppIcon.svg" "$prefix/share/icons/hicolor/scalable/apps/$gpui_app_id.svg"
  test -s "$prefix/share/doc/chipdock-gpui/LICENSE-MIT"
  test -s "$prefix/share/doc/chipdock-gpui/LICENSE-APACHE"
done
test -f "$test_dir/deb/usr/share/chipdock-gpui/package-managed"
test -f "$test_dir/rpm/usr/share/chipdock-gpui/package-managed"
test ! -f "$test_dir/archive/$name/share/chipdock-gpui/package-managed"
echo "Verified Linux $arch DEB, RPM and portable payloads without installing or launching them"
