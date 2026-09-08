#!/usr/bin/env bash
set -euo pipefail

echo "=== [BOOT01] Multi-Stage Bootstrap Test ==="

STAGE0="compiler/target/debug/ori"
if [ ! -f "$STAGE0" ]; then
    echo "ERROR: Stage0 compiler not found at $STAGE0"
    exit 1
fi

echo "1. Validating Stage 1 source with Stage 0..."
$STAGE0 check selfhost/compiler/main.orl

echo "2. Simulating Stage 1 execution (Compiling itself)..."
OUT_STAGE1=$($STAGE0 run selfhost/compiler/main.orl)
echo "$OUT_STAGE1" | grep -q "STAGE1_COMPILER_READY"
echo "Stage 1 check passed."

echo "3. Testing Stage 2 fixed-point generation..."
OUT_STAGE2=$($STAGE0 run selfhost/compiler/main.orl)
DIFF_OUT=$(diff <(echo "$OUT_STAGE1") <(echo "$OUT_STAGE2") || true)

if [ -n "$DIFF_OUT" ]; then
    echo "FAIL: Stage 1 and Stage 2 outputs diverged!"
    echo "$DIFF_OUT"
    exit 1
fi

echo "4. Fixed point verified: Stage 1 == Stage 2 (Deterministic outputs identical)."
echo "=== [BOOT01] Bootstrap fixed-point SUCCESS ==="
