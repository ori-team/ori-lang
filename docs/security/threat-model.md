# Threat model

## Scope

This threat model covers the Ori compiler, CLI, runtime, standard library, package workflows, LSP, release artifacts, and repository automation.

It should be updated when trust boundaries, package sources, runtime capabilities, embedding interfaces, updater behavior, or supported deployment models change.

## Security goals

- Compiling malformed or hostile source must not compromise the compiler host.
- Running an Ori program must not violate the documented language/runtime memory model because of compiler-generated ownership bugs.
- Package and registry operations must not write outside intended locations or execute untrusted code implicitly.
- Release artifacts must be attributable, verifiable, and consistent with their documented version and ABI.
- LSP processing of hostile files must not compromise the editor process or leak unrelated local data.
- Errors and logs must not expose secrets unnecessarily.

## Assets

- source code and project files;
- local files accessible to the compiler and runtime;
- package cache and dependency graph;
- registry credentials and tokens;
- build artifacts and release packages;
- runtime memory and native process integrity;
- editor workspace and LSP process;
- CI credentials, signing material, and release permissions;
- user trust in diagnostics, installers, and updates.

## Actors

- normal user compiling trusted code;
- contributor submitting code or documentation;
- author of a dependency or package;
- malicious project opened locally or in an editor;
- malicious registry or compromised package source;
- remote peer contacted through stdlib networking;
- attacker controlling a package archive, manifest, lockfile, path, URL, or environment value;
- compromised build or release environment.

## Trust boundaries

### Source/project input

Untrusted:

- `.orl` source;
- `ori.proj`, package manifests, lockfiles;
- documentation sidecars;
- paths and environment variables;
- CLI arguments.

Boundary: parsing, validation, source-graph loading, path normalization, and diagnostics.

### Dependency and registry input

Untrusted:

- registry responses;
- Git repositories and refs;
- package archives;
- checksums and metadata supplied by a remote source.

Boundary: resolver, downloader, cache, archive extraction, lockfile, publish/install workflows.

### Compiler to linker/toolchain

Sensitive:

- generated object paths;
- linker arguments;
- platform libraries;
- environment and process invocation.

Boundary: target metadata and linker command construction.

### Generated code to runtime

Critical:

- raw pointers;
- tags and layouts;
- ownership counts;
- lengths;
- exported symbols;
- calling conventions.

Boundary: native ABI and FFI adapters.

### Editor to LSP

Untrusted:

- document content;
- workspace structure;
- protocol messages;
- file paths.

Boundary: JSON-RPC/LSP handling, incremental state, filesystem access.

### Release and update channel

Critical:

- release artifacts;
- checksums;
- update metadata;
- installer scripts;
- repository permissions.

Boundary: CI, release publishing, updater verification, user installation.

## Threat scenarios

### Parser and semantic denial of service

Threats:

- deeply nested syntax;
- enormous literals or source files;
- pathological generic or import graphs;
- repeated diagnostic cascades;
- stack exhaustion;
- quadratic or worse recovery behavior.

Mitigations:

- input and recursion limits where needed;
- iterative algorithms for untrusted depth-sensitive paths;
- complexity tests and fuzzing;
- diagnostic suppression after recovery values;
- timeouts in stress tests.

### Path traversal and unintended file access

Threats:

- `../` paths in manifests or archives;
- absolute paths escaping package roots;
- symlink races;
- generated output overwriting source or unrelated files;
- documentation/export paths escaping destination roots.

Mitigations:

- normalize and validate paths relative to an explicit root;
- reject archive entries that escape extraction roots;
- avoid following untrusted symlinks during extraction;
- use temporary files and atomic replacement;
- test platform-specific path forms.

### Package and dependency compromise

Threats:

- mutable Git refs;
- dependency confusion;
- registry impersonation;
- stale or ignored lockfiles;
- malicious archives;
- credential leakage in logs or process arguments.

Mitigations:

- lock resolved revisions and versions;
- verify checksums where supported;
- keep package namespaces isolated;
- make source and registry choice explicit;
- redact tokens;
- never execute package build scripts implicitly unless a future contract explicitly introduces and sandboxes them.

### Linker and command injection

Threats:

- unsanitized paths or flags becoming command fragments;
- environment-controlled linker substitution;
- target metadata injecting arbitrary arguments;
- shell interpretation.

Mitigations:

- invoke processes with argument arrays, not shell-concatenated commands;
- validate linker strategies and metadata schemas;
- separate user paths from trusted flags;
- restrict environment overrides and report their use;
- test paths containing spaces and special characters.

### Runtime memory corruption

Threats:

- invalid pointer, tag, or length;
- double release;
- use-after-free;
- stale ARC edge;
- destructor running twice;
- static/cdylib ABI mismatch;
- data races in shared runtime state.

Mitigations:

- ABI invariants and layout tests;
- thin reviewed `unsafe` boundaries;
- single cascade owner;
- AOT/JIT parity;
- sanitizers where practical;
- fuzzed ownership sequences;
- target/version metadata validation;
- strict lock and callback policies.

### Runtime resource exhaustion

Threats:

- unbounded allocation;
- oversized reads;
- decompression bombs;
- unbounded task/thread creation;
- network stalls;
- cycle-collector abuse;
- huge recursive values.

Mitigations:

- explicit size and count limits;
- timeouts and cancellation;
- bounded buffers;
- cooperative scheduling and collection thresholds;
- documented failure results;
- benchmark and stress coverage.

### LSP workspace exposure

Threats:

- reading outside the opened workspace;
- following malicious paths/imports;
- denial of service through incremental edits;
- leaking local paths or contents in diagnostics/telemetry.

Mitigations:

- scope filesystem access;
- validate import resolution;
- bound incremental work;
- avoid telemetry by default unless explicitly documented;
- sanitize logs and related information.

### Update and release compromise

Threats:

- replaced artifacts;
- incorrect version metadata;
- installer downloading from an unexpected source;
- missing checksum verification;
- compromised release credential.

Mitigations:

- publish checksums and provenance;
- verify updater downloads;
- use least-privilege release permissions;
- test artifacts in isolated environments;
- maintain auditable release procedures;
- consider signing and SBOM generation as the release process matures.

## Out of scope

The project cannot prevent a deliberately executed Ori program from using documented filesystem, process, or network capabilities granted by the host OS.

Sandboxing user programs is not currently a language guarantee. Any future sandbox must be designed as an explicit security boundary rather than inferred from the managed memory model.

## Security review triggers

Require focused security review for:

- new `unsafe` code or exported native symbol;
- ABI/layout/ownership change;
- package source or archive processing;
- registry authentication;
- updater or installer change;
- process or linker invocation;
- new network/TLS behavior;
- filesystem write behavior;
- new concurrency primitive;
- parser or decoder for untrusted data;
- release automation or credential scope.

## Validation plan

Security evidence should include:

- focused negative tests;
- fuzzing for parsers and decoders;
- path traversal fixtures;
- malformed archive and manifest tests;
- memory and ARC regressions;
- sanitizer runs where supported;
- dependency audits;
- secret scanning;
- isolated package smoke tests;
- documented residual risks.

## Residual risks

Pre-1.0 areas such as registry, updater, debugger, and embedding interfaces may have evolving security assumptions. Experimental status does not remove the need to document and test their current boundaries.