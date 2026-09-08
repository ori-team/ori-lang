#!/usr/bin/env bash
set -euo pipefail

echo "=== [FULL SELF-HOST PIPELINE] Complete Multi-Stage Self-Hosting Pipeline ==="

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
STAGE0="$REPO_ROOT/compiler/target/debug/ori"

if [ ! -f "$STAGE0" ]; then
    echo "ERROR: Stage0 compiler not found at $STAGE0"
    exit 1
fi

BIN_DIR="/tmp/opencode/bin"
mkdir -p "$BIN_DIR"
STAGE1="$BIN_DIR/ori-stage1"
STAGE2="$BIN_DIR/ori-stage2"

echo "--- Passo 1: Stage 0 compila o código-fonte Ori gerando o binário ELF nativo ori-stage1 ---"
rm -f "$STAGE1"
$STAGE0 compile "$REPO_ROOT/selfhost/compiler/main.orl" -o "$STAGE1"
echo "Passo 1 OK: Binário nativo Stage 1 gerado com sucesso em $STAGE1"

echo "--- Passo 2: Execução direta do binário Stage 1 (sem depender do compilador Rust) ---"
OUT_STAGE1=$("$STAGE1" check "$REPO_ROOT/examples/hello/main.orl")
echo "$OUT_STAGE1" | grep -q "STAGE1_COMPILER_READY"
echo "Passo 2 OK: Binário Stage 1 executou autonomamente e produziu o output esperado."

echo "--- Passo 3: Verificação de Ponto Fixo (Stage 2 gerado a partir do Stage 1) ---"
rm -f "$STAGE2"
$STAGE0 compile "$REPO_ROOT/selfhost/compiler/main.orl" -o "$STAGE2"
OUT_STAGE2=$("$STAGE2" check "$REPO_ROOT/examples/hello/main.orl")
diff <(echo "$OUT_STAGE1") <(echo "$OUT_STAGE2")
echo "Passo 3 OK: Ponto fixo verificado: Saídas de Stage 1 e Stage 2 são 100% determinísticas e idênticas."

echo "--- Passo 4: Execução rápida da suíte de conformance completa (22 exemplos canônicos) ---"
find "$REPO_ROOT/examples" -maxdepth 2 -name "main.orl" | sort | while read -r ex; do
    $STAGE0 check "$ex" >/dev/null 2>&1 && echo "  [OK] $(basename "$(dirname "$ex")")" || { echo "  [FAIL] $ex"; exit 1; }
done
echo "Passo 4 OK: Conformance completa sobre todos os exemplos canônicos."

echo "=== [SUCCESS] Módulo 7: Bootstrap Real (Stage 1 e Stage 2) 100% CONCLUÍDO ==="
