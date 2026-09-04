<p align="center">
  <img src="branding/ori-logo-w_text.svg" alt="Ori" width="280">
</p>

# Ori

Ori は、**読みやすさを優先する明示的型付けのネイティブコンパイル言語**です。コンパイラは Rust で実装され、ネイティブ AOT コンパイルと、互換性のあるランタイム cdylib が利用できる場合のインプロセス JIT (`ori run`) を提供します。

**現在のバージョン: `0.3.8`**  
**言語サーフェス: S3/0.4**  
**ネイティブ ABI: `ori-native-abi-1`**  
**成熟度: pre-1.0、活発に開発中**

Ori は、コンパイラ研究、AI 支援プログラミング、そして認知負荷を下げる言語・診断・ドキュメント設計のためのプロジェクトです。技術的に真剣なプロジェクトですが、まだ産業用言語と同等の成熟度を主張していません。

**言語:** [English](README.md) · [Português](README.pt-BR.md) · 日本語

## はじめに

| 目的 | ドキュメント |
|---|---|
| Ori をインストールする | [Installation](docs/install.md) |
| 言語を学ぶ | [Language tour](docs/language/tour.md) |
| 最初のプロジェクトを作る | [First project](docs/guides/first-project.md) |
| CLI を確認する | [CLI reference](docs/guides/cli-reference.md) |
| 言語仕様を読む | [Specification](docs/spec/README.md) |
| リポジトリを理解する | [Project start](PROJECT_START.md) |
| 全ドキュメントを探す | [Documentation ATLAS](docs/ATLAS.md) |
| コントリビュートする | [Contributing](CONTRIBUTING.md) |
| 脆弱性を報告する | [Security policy](SECURITY.md) |

> 現在、日本語版は簡潔なプロジェクト案内です。規範的な仕様と主要な技術ドキュメントは英語を正本とします。

## 言語の例

```ori
module app.hello

import ori.io as io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("division by zero")
    end

    return ok(a / b)
end

main() -> result[void, string]
    const answer: int = try divide(84, 2)
    io.print(f"answer: {answer}")
    return ok()
end
```

Ori の中心的な考え方:

- ファイル先頭の明示的な `module`;
- 明示的な公開契約と限定されたローカル型推論;
- 欠如を表す `optional[T]`;
- 回復可能な失敗を表す `result[T, E]` と `try`;
- struct、enum、trait、generic、pattern matching;
- `using` による決定的なクリーンアップ;
- Cranelift によるネイティブコード生成;
- バージョン管理されたランタイム ABI;
- 安定した、行動可能な診断コード。

## インストールと実行

完全な手順: [docs/install.md](docs/install.md)

リリースパッケージをインストールした後:

```bash
ori --version
ori doctor
ori new hello
ori run hello/main.orl
```

コンパイラ開発:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
cargo --manifest-path compiler/Cargo.toml run -p ori-driver -- run examples/hello/main.orl
```

Cargo workspace は `compiler/` にあります。

## リポジトリ構成

```text
compiler/       コンパイラ、ランタイムソース、LSP、CLI
stdlib/         Ori 標準ライブラリと sidecar ドキュメント
runtime/        ターゲット別に準備されたネイティブランタイム
examples/       実行可能な Ori プロジェクト
docs/           製品、アーキテクチャ、仕様、実装、品質、運用
extensions/     ローカルのエディタ統合
tools/          QA、ベンチマーク、パッケージ、リリース、ドキュメントツール
```

現在のアーキテクチャ: [docs/architecture/overview.md](docs/architecture/overview.md)

## ドキュメントモデル

各テーマには一つの正本があります。

- 製品と現在の状態: `docs/product/`
- 現在のアーキテクチャ: `docs/architecture/`
- 規範的な言語・ランタイム・ABI 契約: `docs/spec/`
- 実装標準: `docs/implementation/`
- 品質と適合性: `docs/quality/`
- セキュリティ: `docs/security/`
- 決定と提案: `docs/decisions/` と `docs/rfcs/`
- 複雑なアクティブ計画: `docs/plans/`
- 運用: `docs/operations/`
- 歴史資料: `docs/archive/`

正本の地図として [docs/ATLAS.md](docs/ATLAS.md) を使用してください。

## 状態と制限

Ori は pre-1.0 です。現在の実装、優先順位、既知の制限は [docs/product/status.md](docs/product/status.md) にあります。互換性契約は [Spec 18](docs/spec/18-stability-and-compatibility.md)、ネイティブ ABI は [Spec 19](docs/spec/19-abi.md) にあります。

## ライセンス

Ori は Apache-2.0 OR MIT のデュアルライセンスです。[LICENSE](LICENSE)、[LICENSE-APACHE](LICENSE-APACHE)、[LICENSE-MIT](LICENSE-MIT) を参照してください。