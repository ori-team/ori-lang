#!/usr/bin/env sh
set -eu

package_root=""
archive_path=""
skip_build=0
force=0

skip_deb=0
deb_path=""

usage() {
    cat <<'USAGE'
Usage: tools/package_native_release.sh [--package-root DIR] [--archive PATH] [--deb PATH] [--skip-build] [--skip-deb] [--force]

Builds and validates a release-style Ori package, then writes a .tar.gz archive
and (on Linux with dpkg-deb) a .deb package.
The package is created through tools/smoke_native_release.sh, so compile/test/JIT
smoke checks must pass before the archive is produced.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --package-root)
            package_root="${2:-}"
            shift 2
            ;;
        --archive)
            archive_path="${2:-}"
            shift 2
            ;;
        --deb)
            deb_path="${2:-}"
            shift 2
            ;;
        --skip-build)
            skip_build=1
            shift
            ;;
        --skip-deb)
            skip_deb=1
            shift
            ;;
        --force)
            force=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
cargo_toml="$repo_root/compiler/Cargo.toml"
if [ ! -f "$cargo_toml" ]; then
    cargo_toml="$repo_root/Cargo.toml"
fi
version=$(awk '
    /^\[workspace\.package\]/ { in_section=1; next }
    in_section && /^\[/ { exit }
    in_section && /^[[:space:]]*version[[:space:]]*=/ {
        gsub(/"/, "", $3)
        print $3
        exit
    }
' "$cargo_toml")
host=$(rustc -Vv | awk -F': ' '/^host:/ { print $2; exit }')

if [ -z "$version" ]; then
    echo "could not find workspace version in $cargo_toml" >&2
    exit 2
fi
if [ -z "$host" ]; then
    echo "could not detect Rust host target from rustc -Vv" >&2
    exit 2
fi

dist_root="${CARGO_TARGET_DIR:-$repo_root/compiler/target}/dist"
if [ -z "$package_root" ]; then
    package_root="$dist_root/ori-$version-$host"
fi
if [ -z "$archive_path" ]; then
    archive_path="$dist_root/ori-$version-$host.tar.gz"
fi

mkdir -p "$(dirname -- "$archive_path")"

if [ "$skip_build" -eq 1 ]; then
    "$script_dir/smoke_native_release.sh" --package-root "$package_root" --keep-package --skip-build
else
    "$script_dir/smoke_native_release.sh" --package-root "$package_root" --keep-package
fi

if [ -e "$archive_path" ] && [ "$force" -eq 0 ]; then
    echo "archive already exists at $archive_path; pass --force to replace it" >&2
    exit 2
fi
archive_temp=$(mktemp "${archive_path}.tmp.XXXXXX")
cleanup_package_temps() {
    if [ -n "${archive_temp:-}" ] && [ -e "$archive_temp" ]; then
        rm -f -- "$archive_temp"
    fi
    if [ -n "${deb_temp_dir:-}" ] && [ -d "$deb_temp_dir" ]; then
        rm -rf -- "$deb_temp_dir"
    fi
}
trap cleanup_package_temps EXIT HUP INT TERM
archive_epoch="${SOURCE_DATE_EPOCH:-0}"
case "$archive_epoch" in
    ''|*[!0-9]*)
        echo "SOURCE_DATE_EPOCH must be a non-negative integer" >&2
        exit 2
        ;;
esac
python3 "$repo_root/tools/release/create_archive.py" \
    --root "$package_root" \
    --archive "$archive_temp" \
    --epoch "$archive_epoch"
mv -f -- "$archive_temp" "$archive_path"
archive_temp=""

printf 'native release package: %s\n' "$package_root"
printf 'native release archive: %s\n' "$archive_path"

# Debian package (Linux only). Optional; skip when dpkg-deb missing or --skip-deb.
case "$(uname -s)" in
    Linux)
        if [ "$skip_deb" -eq 0 ] && command -v dpkg-deb >/dev/null 2>&1; then
            if [ -z "$deb_path" ]; then
                deb_path="$dist_root/ori_${version}_amd64.deb"
            fi
            if [ -e "$deb_path" ] && [ "$force" -eq 0 ]; then
                echo "deb already exists at $deb_path; pass --force to replace it" >&2
                exit 2
            fi
            deb_parent=$(dirname -- "$deb_path")
            mkdir -p "$deb_parent"
            deb_temp_dir=$(mktemp -d "$deb_parent/.ori-deb.XXXXXX")
            deb_temp="$deb_temp_dir/package.deb"
            "$script_dir/package_deb.sh" \
                --package-root "$package_root" \
                --output "$deb_temp" \
                --version "$version" \
                --arch amd64
            mv -f -- "$deb_temp" "$deb_path"
            rmdir "$deb_temp_dir"
            deb_temp_dir=""
            printf 'native release deb: %s\n' "$deb_path"
        elif [ "$skip_deb" -eq 0 ]; then
            echo "warning: dpkg-deb not found; skipping .deb (install dpkg-dev to enable)" >&2
        fi
        ;;
esac

trap - EXIT HUP INT TERM
