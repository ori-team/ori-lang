<p align="center">
  <img src="branding/ori-logo-w_text.svg" alt="Ori" width="280">
</p>

# Ori

Ori is a reading-first, explicitly typed programming language compiled to native code. The compiler is written in Rust and supports native AOT compilation, with an in-process JIT route for `ori run` when the runtime cdylib is available.

**Current version: `0.3.8`**  
**Language surface: S3**  
**Native ABI: `ori-native-abi-1`**  
**Maturity: pre-1.0, active development**

Ori exists for compiler study, AI-assisted programming, and readable programming with lower cognitive load. It is a serious engineering project, but it does not claim industrial-language maturity.

**Languages:** English (primary) · [Português](README.pt-BR.md) · [日本語](README.ja.md)

## Start here

| Goal | Document |
|---|---|
| Install Ori | [Installation](docs/install.md) |
| Learn the language | [Language tour](docs/language/tour.md) |
| Create a project | [First project](docs/guides/first-project.md) |
| View CLI commands | [CLI reference](docs/guides/cli-reference.md) |
| Read the language contract | [Specification](docs/spec/README.md) |
| Understand the repository | [Project start](PROJECT_START.md) |
| Navigate all documentation | [Documentation atlas](docs/ATLAS.md) |
| Contribute | [Contributing](CONTRIBUTING.md) |
| Report a vulnerability | [Security policy](SECURITY.md) |

## Language snapshot

Ori makes important behavior visible:

```ori
module app.hello

import ori.io = io

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

Core ideas include:

- explicit module identity and imports;
- explicit public contracts and local types;
- `optional[T]` for absence;
- `result[T, E]` and `try` for recoverable failure;
- structs, enums, traits, generics, and pattern matching;
- deterministic cleanup with `using`;
- native code generation and a versioned runtime ABI;
- stable, actionable diagnostic codes.

## Install and run

Full instructions: [docs/install.md](docs/install.md).

After installing a release package:

```bash
ori --version
ori doctor
ori new hello
ori run hello/main.orl
```

For compiler development:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
cargo --manifest-path compiler/Cargo.toml run -p ori-driver -- run examples/hello/main.orl
```

The Cargo workspace lives under `compiler/`.

## Repository

```text
compiler/       compiler, runtime source, LSP, and CLI
stdlib/         Ori source modules and sidecar documentation
runtime/        staged native runtime artifacts
examples/       executable projects and integration examples
docs/           product, architecture, specification, implementation, and operations
extensions/     local editor integrations
tools/          QA, benchmark, package, release, and documentation tooling
```

See [docs/architecture/overview.md](docs/architecture/overview.md) for the current system design.

## Documentation model

The repository uses one canonical source per subject:

- product and status: `docs/product/`;
- current architecture: `docs/architecture/`;
- normative language and ABI contracts: `docs/spec/`;
- implementation standards: `docs/implementation/`;
- quality and conformance: `docs/quality/`;
- security engineering: `docs/security/`;
- decisions and proposals: `docs/decisions/` and `docs/rfcs/`;
- active complex plans: `docs/plans/`;
- operations: `docs/operations/`;
- historical evidence: `docs/archive/`.

Use [docs/ATLAS.md](docs/ATLAS.md) as the canonical map.

## Status and limitations

Ori is pre-1.0. The current status, supported routes, priorities, and structural work are documented in [docs/product/status.md](docs/product/status.md). Compatibility rules live in [Spec 18](docs/spec/18-stability-and-compatibility.md), and native ABI rules live in [Spec 19](docs/spec/19-abi.md).

## License

Ori is dual-licensed under Apache-2.0 OR MIT. See [LICENSE](LICENSE), [LICENSE-APACHE](LICENSE-APACHE), and [LICENSE-MIT](LICENSE-MIT).