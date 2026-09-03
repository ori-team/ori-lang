#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/ori-archive-repro.XXXXXX")
cleanup() {
    find "$tmp" -type f -delete 2>/dev/null || true
    find "$tmp" -depth -type d -empty -delete 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$tmp/one/pkg/src" "$tmp/two/pkg/src" \
    "$tmp/one/pkg/.ori" "$tmp/two/pkg/.ori"
printf 'stable payload\n' > "$tmp/one/pkg/src/main.orl"
cp "$tmp/one/pkg/src/main.orl" "$tmp/two/pkg/src/main.orl"
# Cache contents are intentionally different and must not affect the archive.
printf 'cache-one\n' > "$tmp/one/pkg/.ori/object"
printf 'cache-two\n' > "$tmp/two/pkg/.ori/object"

epoch=1700000000
python3 "$repo/tools/release/create_archive.py" \
    --root "$tmp/one/pkg" --archive "$tmp/one.tar.gz" --epoch "$epoch"
python3 "$repo/tools/release/create_archive.py" \
    --root "$tmp/two/pkg" --archive "$tmp/two.tar.gz" --epoch "$epoch"

if ! cmp -s "$tmp/one.tar.gz" "$tmp/two.tar.gz"; then
    echo "archive reproducibility check failed" >&2
    exit 1
fi
echo "archive reproducibility: OK"
