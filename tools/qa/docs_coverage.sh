#!/usr/bin/env sh
# Validate the canonical documentation catalog and its repository paths.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
catalog="$repo/docs/catalog.yaml"

fail=0

require_file() {
    path=$1
    if [ ! -e "$repo/$path" ]; then
        echo "docs_coverage: missing $path" >&2
        fail=1
    fi
}

require_file docs/ATLAS.md
require_file docs/catalog.yaml
require_file docs/quality/documentation-quality.md
require_file docs/quality/language-conformance.md
require_file docs/spec/13-error-catalog.md
require_file docs/spec/19-abi.md
require_file docs/guides/cli-reference.md
require_file docs/guides/cli-reference.pt-BR.md
require_file docs/language/advanced.md
require_file docs/language/concurrency.md
require_file docs/language/interop.md

# Catalog paths are simple repository-relative scalars. Keep this check
# dependency-free so release packages and CI hosts need no YAML library.
while IFS= read -r path; do
    [ -n "$path" ] || continue
    require_file "$path"
done <<EOF
$(awk '
    /^[[:space:]]+path:[[:space:]]+/ {
        line = $0
        sub(/^[^:]*:[[:space:]]*/, "", line)
        if (line !~ /^https?:\/\// && line !~ /^\//) print line
    }
' "$catalog" | sort -u)
EOF

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "docs_coverage: catalog paths and canonical files are valid"
