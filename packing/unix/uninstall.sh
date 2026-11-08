#!/usr/bin/env bash
set -euo pipefail

source_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)"
prefix="${HOME}/.local"
if [ -f "$source_dir/.xcx-legacy-install" ]; then
    prefix="$(cd -- "$source_dir/../.." && pwd -P)"
fi
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            [ "$#" -ge 2 ] && [ -n "$2" ] || { echo '--prefix requires a directory' >&2; exit 2; }
            prefix="$2"
            shift 2
            ;;
        -h|--help) echo 'Usage: ./uninstall.sh [--prefix DIRECTORY]'; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -d "$prefix" ] || { echo 'xcx-legacy is not installed at this prefix'; exit 0; }
prefix="$(cd -- "$prefix" && pwd -P)"
install_dir="$prefix/lib/xcx-legacy"
marker="$install_dir/.xcx-legacy-install"
[ ! -L "$install_dir" ] || { echo "Installation directory cannot be a symlink: $install_dir" >&2; exit 1; }
if [ ! -e "$install_dir" ]; then
    echo 'xcx-legacy is not installed at this prefix'
    exit 0
fi
[ -f "$marker" ] && [ "$(cat "$marker")" = xcx-legacy-v1 ] || {
    echo "Refusing to remove an unmanaged directory: $install_dir" >&2
    exit 1
}
command_path="$prefix/bin/xcx-legacy"
if [ -L "$command_path" ] && [ "$(readlink "$command_path")" = "$install_dir/xcx-legacy" ]; then
    rm -- "$command_path"
fi
for file in xcx-legacy libxcx.a xcx_launcher.o README.md LICENSE install.sh uninstall.sh .xcx-legacy-install; do
    rm -f -- "$install_dir/$file"
done
if ! rmdir -- "$install_dir" 2>/dev/null; then
    echo "Additional user files were preserved in $install_dir"
fi
echo 'xcx-legacy uninstalled. Shared prefix and shell configuration were preserved.'
