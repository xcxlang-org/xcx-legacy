#!/usr/bin/env bash
set -euo pipefail

source_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)"
prefix="${HOME}/.local"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            [ "$#" -ge 2 ] && [ -n "$2" ] || { echo '--prefix requires a directory' >&2; exit 2; }
            prefix="$2"
            shift 2
            ;;
        -h|--help)
            echo 'Usage: ./install.sh [--prefix DIRECTORY]'
            echo 'Default prefix: $HOME/.local. System installation: sudo ./install.sh --prefix /usr/local'
            exit 0
            ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

for file in xcx-legacy libxcx.a xcx_launcher.o README.md LICENSE install.sh uninstall.sh; do
    [ -f "$source_dir/$file" ] || { echo "Incomplete package: missing $file" >&2; exit 1; }
done
mkdir -p -- "$prefix"
prefix="$(cd -- "$prefix" && pwd -P)"
[ "$prefix" != / ] || { echo 'The filesystem root is not an installation prefix' >&2; exit 1; }
install_dir="$prefix/lib/xcx-legacy"
bin_dir="$prefix/bin"
[ ! -L "$install_dir" ] || { echo "Installation directory cannot be a symlink: $install_dir" >&2; exit 1; }
if [ -e "$install_dir" ]; then
    [ -f "$install_dir/.xcx-legacy-install" ] && [ "$(cat "$install_dir/.xcx-legacy-install")" = xcx-legacy-v1 ] || {
        echo "Refusing to replace an unmanaged directory: $install_dir" >&2
        exit 1
    }
fi
if [ -e "$bin_dir/xcx-legacy" ] || [ -L "$bin_dir/xcx-legacy" ]; then
    [ -L "$bin_dir/xcx-legacy" ] && [ "$(readlink "$bin_dir/xcx-legacy")" = "$install_dir/xcx-legacy" ] || {
        echo "Refusing to replace another command: $bin_dir/xcx-legacy" >&2
        exit 1
    }
fi
mkdir -p -- "$install_dir" "$bin_dir"
printf '%s\n' xcx-legacy-v1 > "$install_dir/.xcx-legacy-install"
if [ "$source_dir" != "$install_dir" ]; then
    for file in xcx-legacy libxcx.a xcx_launcher.o README.md LICENSE install.sh uninstall.sh; do
        cp -- "$source_dir/$file" "$install_dir/$file"
    done
fi
chmod 755 "$install_dir/xcx-legacy" "$install_dir/install.sh" "$install_dir/uninstall.sh"
chmod 644 "$install_dir/libxcx.a" "$install_dir/xcx_launcher.o" "$install_dir/README.md" "$install_dir/LICENSE" "$install_dir/.xcx-legacy-install"
if [ ! -L "$bin_dir/xcx-legacy" ]; then
    ln -s -- "$install_dir/xcx-legacy" "$bin_dir/xcx-legacy"
fi
printf 'Installed: %s\n' "$bin_dir/xcx-legacy"
case ":${PATH}:" in
    *":${bin_dir}:"*) ;;
    *)
        echo 'Add this line to your shell startup file, then open a new terminal:'
        printf 'export PATH=%q:"$PATH"\n' "$bin_dir"
        ;;
esac
echo 'Compiling programs requires GCC on Linux or Xcode Command Line Tools on macOS.'
printf 'Uninstall: %q --prefix %q\n' "$install_dir/uninstall.sh" "$prefix"
