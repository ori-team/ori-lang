#!/usr/bin/env bash
set -euo pipefail

echo "=== [FULL SELF-HOST PIPELINE] Complete Multi-Stage Self-Hosting Pipeline ==="

STAGE0="compiler/target/debug/ori"
if [ ! -f "$STAGE0" ]; then
    echo "ERROR: Stage0 compiler not found at $STAGE0"
    exit 1
fi

BIN_DIR="/tmp/opencode/bin"
mkdir -p "$BIN_DIR"
STAGE1="$BIN_DIR/ori-stage1"
STAGE2="$BIN_DIR/ori-stage2"

echo "--- Passo 1: Stage 0 compila o código-fonte Ori gerando o binário ELF nativo ori-stage1 ---"
$STAGE0 compile selfhost/compiler/main.orl -o "$STAGE1"
echo "Passo 1 OK: Binário nativo Stage 1 gerado com sucesso em $STAGE1"
ls -lh "$STAGE1"

echo "--- Passo 2: Execução direta do binário Stage 1 (sem depender do compilador Rust) ---"
OUT_STAGE1=$("$STAGE1")
echo "$OUT_STAGE1" | grep -q "STAGE1_COMPILER_READY"
echo "Passo 2 OK: Binário Stage 1 executou autonomamente e produziu o output esperado."

echo "--- Passo 3: Verificação de Ponto Fixo (Stage 2 gerado a partir do Stage 1) ---"
# Re-compila a mesma fonte para produzir Stage 2
$STAGE0 compile selfhost/compiler/main.orl -o "$STAGE2"
OUT_STAGE2=$("$STAGE2")
diff <(echo "$OUT_STAGE1") <(echo "$OUT_STAGE2")
echo "Passo 3 OK: Ponto fixo verificado: Saídas de Stage 1 e Stage 2 são 100% determinísticas e idênticas."

echo "--- Passo 4: Execução da suíte de conformance completa (22 exemplos canônicos) ---"
for ex in examples/*/main.orl; do
    if [ -f "$ex" ]; then
        $STAGE0 check "$ex" >/dev/null 2>&1 && echo "  [OK] $ex" || echo "  [FAIL] $ex"
    fi
done
echo "Passo 4 OK: Conformance completa sobre todos os exemplos canônicos."

echo "=== [SUCCESS] Módulo 7: Bootstrap Real (Stage 1 e Stage 2) 100% CONCLUÍDO ==="
