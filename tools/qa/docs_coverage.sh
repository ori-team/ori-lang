#!/usr/bin/env sh
# Validate the machine-readable documentation Atlas and its referenced paths.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
registry="$repo/docs/atlas/features.yaml"
backlog="$repo/docs/planning/BACKLOG.md"

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
require_file docs/planning/BACKLOG.md

# The Atlas uses a deliberately small YAML subset. Validate its schema and
# paths without requiring PyYAML or any other release-host dependency.
python3 "$repo/tools/qa/validate_atlas.py" "$registry"

if rg -n --glob '*.md' 'sidecar_first|docs/planning/PLANO-MATURIDADE-COMPLETO\.md|docs/planning/stdlib-gap-parity\.md|packages/FREEZE-WEB\.md' \
    "$repo/docs" "$repo/README.md" >/dev/null 2>&1; then
    echo "docs_coverage: stale canonical path or manifest spelling found" >&2
    rg -n --glob '*.md' 'sidecar_first|docs/planning/PLANO-MATURIDADE-COMPLETO\.md|docs/planning/stdlib-gap-parity\.md|packages/FREEZE-WEB\.md' \
        "$repo/docs" "$repo/README.md" >&2
    fail=1
fi

# P0 is the release-blocking priority. Keep its closure machine-enforced so a
# newly discovered blocker cannot be left hidden behind a stale roadmap claim.
open_p0=$(awk -F '|' '
    function trim(value) {
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        return value
    }
    /^\|/ {
        priority = trim($4)
        status = trim($6)
        if (priority == "0" && status != "**done**") print NR ":" $0
    }
' "$backlog")

if [ -n "$open_p0" ]; then
    echo "docs_coverage: release-blocking P0 backlog entries remain open" >&2
    echo "$open_p0" >&2
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "docs_coverage: Atlas paths, canonical references, and P0 closure are valid"
