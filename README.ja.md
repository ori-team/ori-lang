<p align="center">
  <img src="branding/ori-logo-w_text.svg" alt="Ori" width="280">
</p>

# Ori

**Surface S3 (`0.3.0`)** — Auk9-inspired syntax on the Ori engine. Purpose: study / AI / ND readability ([manifesto](docs/spec/00-manifesto.md)). Auk9 lab retired as a product.

Ori は、読みやすさを最優先にした、明示的な型を持つネイティブコンパイル言語です。
コンパイラは Rust で書かれており、プログラムを読み、調べ、診断し、保守しやすくする
ことを目的にしています。

Ori はまだ pre-1.0 です。コンパイラ、言語設計、ツール、ランタイムの開発には有用
ですが、安定した 1.0 契約の前に言語仕様が変わる可能性があります。

**言語:** [English](README.md) | [Português](README.pt-BR.md) | 日本語

**プロジェクトメニュー:** [マニフェスト](docs/spec/00-manifesto.md) | [仕様](docs/spec/README.md) | [計画](docs/planning/README.md) | [パフォーマンス](docs/guides/performance.md) | [標準ライブラリ](stdlib/README.md) | [ランタイム](runtime/README.md) | [例](examples/) | [Changelog](CHANGELOG.md) | [Contributing](CONTRIBUTING.md)

## 目次

- [Ori とは](#ori-とは)
- [Ori が目指すもの](#ori-が目指すもの)
- [現在の状態](#現在の状態)
- [パフォーマンス（要約）](#パフォーマンス要約)
- [クイックスタート](#クイックスタート)
- [最初のプログラム](#最初のプログラム)
- [CLI 概要](#cli-概要)
- [言語の概要](#言語の概要)
- [コンパイラ構成](#コンパイラ構成)
- [標準ライブラリ](#標準ライブラリ)
- [エディタ支援](#エディタ支援)
- [リポジトリ構成](#リポジトリ構成)
- [開発フロー](#開発フロー)
- [リリース構成](#リリース構成)
- [既知の制限](#既知の制限)
- [ロードマップ](#ロードマップ)
- [ライセンス](#ライセンス)

## Ori とは

Ori は、明示的なモジュール（`module`）、明示的な型（`optional[T]` / `result[T, E]`）、
構造化エラー（`try`）、`apply`/`use` による traits、決定的クリーンアップ（`using`）、
ネイティブコード生成を持つ静的型付け言語です。

現在のコンパイラパイプライン:

```text
.orl source
  -> lexer
  -> parser
  -> name resolver
  -> type checker
  -> HIR
  -> Cranelift native backend
  -> runtime-linked binary or JIT execution
```

このリポジトリには、コンパイラ、ランタイム、標準ライブラリのソース、言語仕様、
VS Code 拡張、サンプル、リリース用ツールが含まれています。

## Ori が目指すもの

Ori は、書きやすさよりも読みやすさを優先します。

読者が必要とする情報を、その場で見える形にします。

| 読者の疑問 | Ori で見えるもの |
|---|---|
| このファイルはどこに属するか | 各ファイル先頭の `module` |
| この値の型は何か | 明示的な型注釈 |
| この値は存在しないことがあるか | `optional[T]` |
| この処理は失敗することがあるか | `result[T, E]` |
| リソースはいつ解放されるか | `using` |
| 振る舞いはどこから来るか | `trait` と `apply` / `use` |
| 何が失敗したか | 構造化された診断コード |

この設計は、認知負荷を下げるためのものです。隠れた規則を減らし、推論の連鎖を短くし、
エラーメッセージを読みやすくします。

## 現在の状態

| 領域 | 状態 |
|---|---|
| バージョン | **言語 surface `0.3.0`（S3 cutover）**; Cargo package は release tag まで `0.2.0` のまま可 |
| 安定性 | pre-1.0; S3 は 0.2 構文との hard break; 今後も変更の余地あり |
| コンパイラ | lexer, parser, HIR, type checker, codegen, diagnostics, LSP, driver, runtime を含む Rust workspace |
| ネイティブ backend | Cranelift object code と Ori native runtime |
| `ori run` | runtime cdylib がある場合は既定で JIT; AOT も強制可能 |
| `ori compile` | AOT ネイティブバイナリ生成; link 経路は設定された linker strategy に依存 |
| C backend | debug/transpile 用。機能の完全な同等性はありません |
| 標準ライブラリ | Layer 1 runtime primitives と Layer 2/3 の `.orl` wrappers/algorithms |
| ツール | CLI, formatter, diagnostics catalog, docs export, LSP, VS Code extension |
| テスト | workspace test suite と native release smoke がプロジェクト gate です |

S3 **が**そのユーザー可視の破壊的変更です（[CHANGELOG.md](CHANGELOG.md) `[0.3.0]`）。
Nim 風ローカル推論は **`0.3.1`**（**オプション B**: フィールド / 添字 / 呼び出し /
パイプで型が自明なら注釈省略可）。パイプ `|>` は **サポート継続**。
移行: `ori migrate-syntax`。

## パフォーマンス（要約）

Ori AOT と Python / Rust / C / Go / JS / TS / Ruby / Nim のマイクロベンチ要約は
英語・ポルトガル語のガイドにあります（この README 日本語版はリンクのみ）:

- [docs/guides/performance.md](docs/guides/performance.md) (EN)
- [docs/guides/performance.pt-BR.md](docs/guides/performance.pt-BR.md) (PT)
- 再現: `SAMPLES=5 ./tools/bench/polyglot/run_polyglot_bench.sh`

概要（2026-07-14、ループ GC 修正 + mid-end + list スカラー inline）:
Ori は CPython より約 **30–1400×** 高速、fib で Go/Nim より速く、Rust の約
**1.5×**（list は約 **1.25×**）。詳細は上記ガイド。
## クイックスタート

コンパイラ開発の前提:

- `rust-toolchain.toml` で指定された Rust `1.95.0`
- プラットフォーム linker、または Ori の明示的な linker strategy
- Windows の release smoke scripts には PowerShell
- Linux/macOS の system discovery path には C toolchain

リポジトリのルートで:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- check examples/hello
cargo run -p ori-driver -- run examples/hello
```

Windows で release 形式の package を検証する場合:

```powershell
.\tools\smoke_native_release.ps1
```

Linux または macOS:

```sh
sh tools/smoke_native_release.sh
```

## 最初のプログラム

```ori
module app.hello

import ori.io = io

main()
    io.print("Hello, Ori!")

    const answer: int = 21 * 2
    io.print(f"The answer is {answer}")
end
```

このリポジトリから実行:

```bash
cargo run -p ori-driver -- run examples/hello
```

Ori は `end` でブロックを閉じます。宣言は行で区切り、import は明示的で、binding と
public contract には明示的な型を使います。

## CLI 概要

`ori` CLI は `compiler/crates/ori-driver` にあります。

| コマンド | 目的 |
|---|---|
| `ori check <file.orl>` | parse, resolve, type-check を実行 |
| `ori run <file.orl>` | runtime と環境変数に応じて JIT または AOT で実行 |
| `ori compile <file.orl>` | Cranelift backend でネイティブ実行ファイルを生成 |
| `ori test <file.orl>` | `@test` が付いた関数を実行 |
| `ori fmt <file.orl>` | source を整形して出力 |
| `ori doc file <file.orl>` | documentation comments を Markdown または HTML として抽出 |
| `ori doc export` | stdlib symbols, diagnostics, keywords を JSON として export |
| `ori doctor` | stdlib, runtime, linker, target, JIT の状態を表示 |
| `ori explain <code>` | diagnostic code を説明 |
| `ori summary [path]` | entry file, namespaces, imports, diagnostics count を表示 |
| `ori build <file.orl>` | debug backend で C を出力 |
| `ori lex <file.orl>` | compiler debug 用に token stream を表示 |
| `ori parse <file.orl>` | compiler debug 用に AST を表示 |
| `ori install <name>` | registry placeholder; まだ利用できません |
| `ori publish <path>` | registry placeholder; まだ利用できません |

よく使う環境変数:

| 変数 | 目的 |
|---|---|
| `ORI_STDLIB_ROOT` | `stdlib/` source root を上書き |
| `ORI_RUNTIME_LIB` | native runtime static library を上書き |
| `ORI_RUNTIME_CDYLIB` | JIT が使う runtime cdylib を上書き |
| `ORI_USE_JIT=1` | `ori run` で JIT を強制 |
| `ORI_USE_AOT=1` | `ori run` で AOT を強制 |
| `ORI_USE_BUNDLED_RUST_LLD=1` | `rustc` driver なしで bundled `rust-lld` を使う |
| `ORI_USE_SYSTEM_LINKER=1` | platform linker を直接使う |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | package validation 中に workspace runtime fallback を拒否 |

完全な環境変数一覧は [AGENTS.md](AGENTS.md) にあります。

## 言語の概要

Ori の中心モデルは小さく保たれています。

- すべてのファイルは `module` で始まります。
- import: `import path (A)` / `import path = alias` / bare `import path`。
- top-level declaration は、`public` がない限り private です。
- `struct` と `enum` はデータを定義します。
- `trait` と `apply` / `use` は振る舞いを定義します。
- `optional[T]` は値の不在を表します。
- `result[T, E]` は回復可能な失敗を表します。
- 伝播は **`try expr` のみ**（postfix `?` は S3 で削除）。
- `using` は cleanup を明示します。
- diagnostics は `name.undefined` や `project.circular_import` のような
  安定した code を使います。

`result` の例:

```ori
module app.errors

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("division by zero")
    end

    return ok(a / b)
end

main() -> result[void, string]
    const value: int = try divide(84, 2)
    io.print(f"value: {value}")
    return ok()
end
```

言語の規範的な契約は
[docs/spec/01-overview.md](docs/spec/01-overview.md) から読むのが最短です。

## コンパイラ構成

コンパイラは役割別の crate に分かれています。

| Crate | 役割 |
|---|---|
| `ori-lexer` | tokenization |
| `ori-ast` | AST node definitions |
| `ori-parser` | recursive descent parser |
| `ori-hir` | name resolution と high-level IR |
| `ori-types` | type system, stdlib manifest, checker contracts |
| `ori-codegen` | Cranelift native backend, JIT path, C debug backend |
| `ori-runtime` | native runtime library と runtime ABI |
| `ori-diagnostics` | diagnostic codes と rendering support |
| `ori-lsp` | Language Server Protocol implementation |
| `ori-driver` | CLI, pipeline orchestration, integration tests |

native runtime は `ori compile`, `ori run`, `ori test` の意味論的な基準です。
C backend は debug route であり、async, ARC, collections, runtime behavior の基準では
ありません。

## 標準ライブラリ

標準ライブラリは `ori.*` module にあります。

現在の構成:

| Layer | 場所 | 目的 |
|---|---|---|
| Layer 1 | `compiler/crates/ori-types/src/stdlib.rs` と `compiler/crates/ori-runtime/src/lib.rs` | manifest, ABI, hot runtime primitives |
| Layer 2 | `stdlib/**/*.orl` | runtime primitives の安全な wrappers |
| Layer 3 | `stdlib/**/*.orl` | Ori で書かれた pure algorithms |

利用できる主な領域:

- `ori.io`, `ori.fs`, `ori.path`
- `ori.string`, `ori.bytes`, `ori.convert`
- `ori.list`, `ori.map`, `ori.set`
- `ori.math`, `ori.random`, `ori.time`
- `ori.json`, `ori.net`, `ori.process`
- `ori.task`, `ori.channel`, `ori.concurrent`
- `ori.test` と test helpers

現在の module 一覧は [stdlib/README.md](stdlib/README.md)、規範的な契約は
[docs/spec/12-stdlib.md](docs/spec/12-stdlib.md) を参照してください。

## エディタ支援

Ori には LSP server と VS Code extension があります。
場所は [extensions/vscode-orl](extensions/vscode-orl/) です。

実装済みの支援:

- parser, resolver, type checker 由来の diagnostics;
- hover, go-to-definition, find references, rename;
- semantic tokens, document symbols, workspace symbols, inlay hints;
- 型を使った dot completion;
- Layer 1/Layer 2 stdlib を理解する hover/completion/goto;
- formatting, code actions, code lens, signature help;
- incremental document sync;
- check, run, test, format, doctor, summary の VS Code commands.

拡張のローカル build:

```bash
cd extensions/vscode-orl
npm install
npm run compile
```

先に language server を build します。

```bash
cargo build -p ori-lsp -p ori-driver
```

## リポジトリ構成

```text
ori-lang/
  compiler/crates/        compiler, LSP, runtime, driver の Rust workspace
  docs/spec/              言語と実装の規範的な契約
  docs/planning/          roadmap, backlog, implementation plans
  stdlib/                 標準ライブラリの source modules
  runtime/                target triple ごとの staged runtime artifacts
  examples/               Ori sample programs
  tests/                  E2E fixtures と test documentation
  extensions/vscode-orl/  VS Code extension
  tools/                  staging, smoke, export, validation scripts
  branding/               project logo assets
  _reversa_sdd/           reverse-engineering audit の履歴文書
```

## 開発フロー

よく使う gate:

```bash
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-lsp
```

stdlib 変更の場合:

```bash
cargo test -p ori-types --lib stdlib
cargo test -p ori-driver --test multifile_imports
```

runtime または native backend を変更した場合は、compile/run integration tests の前に
runtime を再 stage します。

```powershell
.\tools\stage_native_runtime.ps1
```

Unix:

```sh
./tools/stage_native_runtime.sh
```

プロジェクトルール:

- bug fix には `compiler/crates/ori-driver/tests/` の regression test が必要です。
- 新しい振る舞いには docs と `CHANGELOG.md` の更新が必要です。
- 新しい diagnostic code は
  [docs/spec/13-error-catalog.md](docs/spec/13-error-catalog.md) に登録します。
- stdlib runtime 変更は manifest, lowering, runtime ABI, tests, docs を同期します。

## リリース構成

release 形式の package は次の形を想定しています。

```text
ori.exe                         # Unix では `ori`
runtime/
  bin/
    rust-lld[.exe]              # optional bundled linker
  {target-triple}/
    ori_runtime.lib             # Windows MSVC static runtime
    libori_runtime.a            # Unix-style static runtime
    ori_runtime.dll             # Windows runtime cdylib for JIT
    libori_runtime.so           # Linux runtime cdylib for JIT
    libori_runtime.dylib        # macOS runtime cdylib for JIT
    runtime-link.json
examples/
README.md
```

`native-route` workflow は Windows MSVC, Windows GNU, Linux GNU, macOS x86_64,
macOS aarch64 を対象にしています。runtime staging の詳細は
[runtime/README.md](runtime/README.md) にあります。

## 既知の制限

現在の pre-1.0 制限:

- Ori はまだ self-hosting ではありません。
- `ori compile` は AOT route であり、動作する linker strategy が必要です。
- C backend は partial で、debug 用です。
- `ori install` と `ori publish` は registry stubs です。
- `ori repl` はまだ backlog です。
- 一部の高度な async shape は maturity plan に known issues として記載されています。
- public contract は 1.0 前に変わる可能性があります。

active backlog は [docs/planning/PENDENTES.md](docs/planning/PENDENTES.md) と
[docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md](docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md)
を参照してください。

## ロードマップ

1.0 に向けた技術的な順序は **stdlib → ABI → Rust 非依存 → self-host 最後** です。

1. stdlib の整理・統合（Layer 2/3）— **次**
2. 最終機能統合後に stable ABI を文書化
3. インストーラ経路での Rust 非依存の仕上げ（JIT + SystemLinker は概ね済み）
4. self-hosting または bootstrap の文書化 — **最後**
5. 1.0 接近時の契約安定性（長期間 breaking を避ける等）

バックログ順: `docs/planning/PENDENTES.md`（M2 → M3 → M1 → M4）。

## ライセンス

Ori は次のどちらかのライセンスで利用できます。

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

利用者が選択できます。
