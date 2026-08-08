#!/usr/bin/env sh
# Validate the machine-readable documentation Atlas and its referenced paths.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
registry="$repo/docs/atlas/features.yaml"

fail=0

require_file() {
    path=$1
    if [ ! -e "$repo/$path" ]; then
        echo "docs_coverage: missing $path" >&2
        fail=1
    fi
}

require_file docs/ATLAS.md
require_file docs/atlas/features.yaml
require_file docs/spec/03-grammar.ebnf
require_file docs/spec/19-abi.md
require_file docs/guides/cli-reference.md
require_file docs/guides/cli-reference.pt-BR.md

# The registry uses one-line YAML arrays for source paths. Keep this check
# deliberately dependency-free so it works on release packages and CI hosts
# without Python/Ruby YAML libraries.
while IFS= read -r path; do
    [ -n "$path" ] || continue
    require_file "$path"
done <<EOF
$(awk '
    /^(    )(implementation|tests|user_docs|reference|examples):/ {
        line = $0
        sub(/^[^[]*\[/, "", line)
        sub(/\].*$/, "", line)
        count = split(line, values, ",")
        for (i = 1; i <= count; i++) {
            value = values[i]
            gsub(/[[:space:]]/, "", value)
            gsub(/"/, "", value)
            if (value != "") print value
        }
    }
' "$registry" | sort -u)
EOF

if rg -n --glob '*.md' 'sidecar_first|docs/planning/PLANO-MATURIDADE-COMPLETO\.md|docs/planning/stdlib-gap-parity\.md|packages/FREEZE-WEB\.md' \
    "$repo/docs" "$repo/README.md" >/dev/null 2>&1; then
    echo "docs_coverage: stale canonical path or manifest spelling found" >&2
    rg -n --glob '*.md' 'sidecar_first|docs/planning/PLANO-MATURIDADE-COMPLETO\.md|docs/planning/stdlib-gap-parity\.md|packages/FREEZE-WEB\.md' \
        "$repo/docs" "$repo/README.md" >&2 || true
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "docs_coverage: Atlas paths and canonical references are valid"
