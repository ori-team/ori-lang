# Security engineering

This domain documents how Ori protects users, contributors, build environments, and runtime processes.

## Canonical documents

- [`../../SECURITY.md`](../../SECURITY.md) — public vulnerability-reporting policy.
- [`threat-model.md`](threat-model.md) — assets, trust boundaries, threats, and mitigations.
- [`unsafe-code-policy.md`](unsafe-code-policy.md) — Rust `unsafe`, FFI, pointer, ownership, and ABI requirements.
- [`../spec/16-runtime-ffi-safety.md`](../spec/16-runtime-ffi-safety.md) — normative runtime FFI safety contract.
- [`../spec/19-abi.md`](../spec/19-abi.md) — native ABI contract.

## Security principle

Ori processes source code, project files, dependencies, package archives, registry data, network input, filesystem paths, and runtime values that may be untrusted.

Security must therefore be designed into:

- compiler input handling;
- package and registry workflows;
- runtime memory and FFI boundaries;
- build and linker invocation;
- LSP/editor operation;
- release and update distribution;
- documentation examples and operational guidance.

Security fixes require regression evidence and coordinated disclosure when appropriate.