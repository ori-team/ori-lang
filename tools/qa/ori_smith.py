#!/usr/bin/env python3
"""ori-smith: deterministic semantic program generator for Ori.

Generates small, well-typed, terminating Ori programs from a seeded RNG and
checks AOT/JIT differential equivalence for each one. This is a semantic
fuzzer (Csmith-style), not a byte fuzzer: every generated program must pass
`ori check`, terminate, and produce identical stdout on both backends.

Usage:
    python3 tools/qa/ori_smith.py [--cases N] [--seed S] [--keep-going]

Exit codes: 0 = all cases pass, 1 = divergence or crash found.
"""

from __future__ import annotations

import argparse
import os
import random
import subprocess
import sys
import tempfile
from pathlib import Path


def find_ori(repo: Path) -> Path | None:
    configured = os.environ.get("ORI_BIN")
    if configured:
        candidate = Path(configured)
        return candidate if candidate.is_file() else None
    for candidate in (
        repo / "compiler" / "target" / "debug" / "ori",
        repo / "compiler" / "target" / "release" / "ori",
    ):
        if candidate.is_file():
            return candidate
    return None


class Smith:
    """State machine that tracks declared bindings to keep output well-typed."""

    def __init__(self, rng: random.Random) -> None:
        self.rng = rng
        self.int_vars: list[str] = []
        self.str_vars: list[str] = []
        self.counter = 0

    def fresh(self, prefix: str) -> str:
        self.counter += 1
        return f"{prefix}{self.counter}"

    def int_expr(self, depth: int) -> str:
        r = self.rng
        choices = ["lit", "lit", "var", "binop"]
        if depth > 0:
            choices.append("paren")
        kind = r.choice(choices)
        if kind == "lit":
            return str(r.randint(-20, 50))
        if kind == "var" and self.int_vars:
            return r.choice(self.int_vars)
        if kind == "paren":
            inner = self.int_expr(depth - 1)
            # Parenthesized bare variables confuse the statement parser;
            # only parenthesize real binary operations.
            if any(op in inner for op in ("+", "-", "*", " ")):
                return f"({inner})"
            return inner
        if depth <= 0:
            return str(r.randint(-20, 50))
        lhs = self.int_expr(depth - 1)
        rhs = self.int_expr(0)
        op = r.choice(["+", "-"])
        # Avoid multiplication/division chains: keeps values small (no
        # overflow traps) and semantics total.
        return f"{lhs} {op} {rhs}"

    def bool_expr(self) -> str:
        r = self.rng
        lhs = self.int_expr(1)
        rhs = self.int_expr(1)
        op = r.choice(["==", "!=", "<", ">", "<=", ">="])
        return f"{lhs} {op} {rhs}"

    def gen_struct(self, out: list[str]) -> str:
        self.counter += 1
        name = f"S{self.counter}"
        out.append(f"struct {name}")
        out.append("    x: int")
        out.append("    y: int")
        out.append("end")
        out.append("")
        return name

    def gen_program(self) -> str:
        r = self.rng
        self.int_vars = []
        self.counter = 0
        out = ["module app.smith", "", "import ori.io as io", ""]
        struct_name = self.gen_struct(out)
        out.append("main()")
        # Declare 1-3 integer bindings.
        for _ in range(r.randint(1, 3)):
            var = self.fresh("v")
            out.append(f"    const {var}: int = {self.int_expr(1)}")
            self.int_vars.append(var)
        # Bounded loop with accumulation (always terminates).
        acc = self.fresh("acc")
        idx = self.fresh("i")
        out.append(f"    var {acc}: int = 0")
        out.append(f"    var {idx}: int = 0")
        out.append(f"    while {idx} < {r.randint(1, 8)}")
        out.append(f"        {acc} = {acc} + {self.int_expr(1)}")
        out.append(f"        {idx} = {idx} + 1")
        out.append("    end")
        # Struct construction + field access + conditional print.
        out.append(
            f"    const pt: {struct_name} = {struct_name} {{ x: {self.int_expr(1)}, y: {self.int_expr(1)} }}"
        )
        out.append(f"    if {self.bool_expr()}")
        out.append(f"        io.println(string({acc} + pt.x))")
        out.append("    else")
        out.append(f"        io.println(string({acc} + pt.y))")
        out.append("    end")
        out.append("end")
        return "\n".join(out) + "\n"


def run_backend(ori: Path, src: Path, mode: str) -> tuple[bool, str, str]:
    env = dict(os.environ)
    if mode == "jit":
        env["ORI_USE_JIT"] = "1"
        cmd = [str(ori), "run", str(src)]
    else:
        exe = src.parent / "aot_bin"
        cmd = [str(ori), "compile", str(src), "-o", str(exe)]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        if proc.returncode != 0:
            return False, "", proc.stderr[-2000:]
        cmd = [str(exe)]
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=60, env=env
        )
    except subprocess.TimeoutExpired:
        return False, "", "timeout"
    return proc.returncode == 0, proc.stdout, proc.stderr[-2000:]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cases", type=int, default=25)
    parser.add_argument("--seed", type=int, default=0x4F5249)
    parser.add_argument("--keep-going", action="store_true")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[2]
    ori = find_ori(repo)
    if ori is None:
        print("ori_smith: ori binary not found; build first", file=sys.stderr)
        return 2

    failures = 0
    for case in range(args.cases):
        rng = random.Random(args.seed + case)
        smith = Smith(rng)
        program = smith.gen_program()
        with tempfile.TemporaryDirectory(prefix="ori_smith_") as tmp:
            src = Path(tmp) / "main.orl"
            src.write_text(program, encoding="utf-8")
            ok_aot, out_aot, err_aot = run_backend(ori, src, "aot")
            ok_jit, out_jit, err_jit = run_backend(ori, src, "jit")
            if not ok_aot or not ok_jit:
                print(f"case {case}: backend failure (aot={ok_aot} jit={ok_jit})")
                print("--- program ---")
                print(program)
                print(f"--- aot stderr ---\n{err_aot}")
                print(f"--- jit stderr ---\n{err_jit}")
                failures += 1
            elif out_aot != out_jit:
                print(f"case {case}: DIVERGENCE")
                print("--- program ---")
                print(program)
                print(f"--- aot ---\n{out_aot}\n--- jit ---\n{out_jit}")
                failures += 1
            if failures and not args.keep_going:
                break

    if failures:
        print(f"ori_smith: {failures} failing case(s)", file=sys.stderr)
        return 1
    print(f"ori_smith: OK ({args.cases} cases, seed {args.seed:#x})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
