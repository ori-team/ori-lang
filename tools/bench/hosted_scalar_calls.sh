#!/usr/bin/env sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$project_root/compiler"
exec cargo run -p ori-embed --release --example hosted_scalar_calls
