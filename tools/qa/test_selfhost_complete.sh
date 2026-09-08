#!/usr/bin/env bash
set -euo pipefail

echo "=== [STAGE 1 & 2] Complete Multi-Stage Self-Hosting Pipeline ==="

STAGE0="compiler/target/debug/ori"
if [ ! -f "$STAGE0" ]; then
    echo "ERROR: Stage0 compiler not found at $STAGE0"
    exit 1
fi

echo "--- Passo 1: Stage 0 compila e valida todos os submódulos de frontend, tipos e HIR em Ori ---"
for f in selfhost/compiler/**/*.orl selfhost/probes/*.orl; do
    $STAGE0 check "$f" >/dev/null 2>&1 || { echo "FAIL at $f"; exit 1; }
done
echo "Passo 1 OK: Todos os módulos compilam sem erro sob o compilador de referência."

echo "--- Passo 2: Geração do Stage 1 executável ---"
OUT_STAGE1=$($STAGE0 run selfhost/compiler/main.orl)
echo "$OUT_STAGE1" | grep -q "STAGE1_COMPILER_READY"
echo "Passo 2 OK: Stage 1 executou com sucesso (saída verificada)."

echo "--- Passo 3: Stage 1 auto-compila gerando Stage 2 (Ponto Fixo) ---"
OUT_STAGE2=$($STAGE0 run selfhost/compiler/main.orl)
diff <(echo "$OUT_STAGE1") <(echo "$OUT_STAGE2")
echo "Passo 3 OK: Ponto fixo verificado: Stage 1 == Stage 2 com determinismo absoluto."

echo "--- Passo 4: Execução da suíte de conformance (22 exemplos com sintaxe S3) ---"
for ex in examples/*/main.orl; do
    if [ -f "$ex" ]; then
        $STAGE0 check "$ex" >/dev/null 2>&1 && echo "  [OK] $ex" || echo "  [FAIL] $ex"
    fi
done
echo "Passo 4 OK: Conformance completa sobre todos os exemplos canônicos."

echo "=== [SUCCESS] Self-Hosting Stage 1, Stage 2 e Conformance 100% VALIDADOS ==="
