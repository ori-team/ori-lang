# Release operations

This procedure defines how to prepare, validate, and publish an Ori release.

## Release inputs

A release includes, as applicable:

- `ori` CLI;
- `ori-lsp`;
- static native runtime;
- runtime cdylib for JIT;
- `runtime-link.json` metadata;
- standard-library source and documentation;
- examples;
- installer or package formats;
- checksums and release notes.

## Release prerequisites

- version and scope are approved;
- current version metadata is consistent;
- changelog release section is complete;
- required compatibility and migration notes exist;
- workspace, conformance, runtime, package, and documentation gates pass;
- runtime ABI change has been classified;
- supported target matrix is known;
- no unresolved critical security issue blocks publication.

## Version consistency

Verify:

- `compiler/Cargo.toml` workspace version;
- package and crate versions;
- `docs/product/status.md`;
- root README files;
- installer/updater metadata;
- runtime-link project version;
- artifact names;
- changelog heading;
- book or generated docs claiming an exact current version.

The current canonical version is **0.3.8** until an explicit release change updates all required sources.

## ABI review

Classify runtime changes:

- additive symbol with compatible layouts;
- compatible implementation fix;
- incompatible layout, lifecycle, mangling, or calling-convention change.

An incompatible change requires a new `ori-native-abi-N` tag, specification update, restaging, package validation, and migration/compatibility notes.

## Validation gates

At minimum:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test diagnostic_catalog
cargo --manifest-path compiler/Cargo.toml test -p ori-lsp
sh tools/qa/daily_fast.sh
```

Run full, performance, platform, and residual gates required by the release scope.

## Runtime staging

Stage the target runtime through canonical scripts. Verify both static and dynamic artifacts.

```bash
sh tools/stage_native_runtime.sh
```

Windows:

```powershell
.\tools\stage_native_runtime.ps1
```

Validate:

- file names match target conventions;
- staticlib and cdylib were built from the release source;
- ABI and project versions match;
- required system libraries are present in metadata;
- no machine-specific local paths are embedded.

## Package creation

Unix-like package:

```bash
sh tools/package_native_release.sh
```

Windows package:

```powershell
.\tools\package_native_release.ps1
```

Distribution-specific packaging such as `.deb` should use the repository's canonical script.

## Isolated smoke tests

Test the produced package outside the source tree.

Unix:

```bash
sh tools/smoke_native_release.sh
```

Windows:

```powershell
.\tools\smoke_native_release.ps1
```

The smoke environment should prevent silent fallback to:

- workspace stdlib;
- workspace runtime;
- development Cargo outputs;
- installed Rust tools when testing the no-Rust path.

Validate:

- `ori --version`;
- `ori doctor`;
- JIT `ori run`;
- AOT compile with documented linker prerequisites;
- executable behavior;
- LSP startup;
- stdlib discovery;
- examples;
- install/uninstall or package-manager behavior.

## Target matrix

For every advertised target, record:

- target triple;
- CI runner/environment;
- compiler and linker route;
- static runtime status;
- cdylib status;
- JIT smoke;
- AOT smoke;
- package format;
- known limitations.

Do not advertise a target based only on successful cross-compilation without runtime smoke evidence.

## Reproducibility and provenance

Record:

- source commit and tag;
- toolchain version;
- dependency lockfile;
- workflow/run identity;
- target and linker;
- build options;
- artifact hashes;
- runtime and ABI metadata;
- package contents.

The project should progressively add:

- signed checksums;
- software bill of materials;
- artifact attestations;
- deterministic archive ordering and timestamps;
- documented reproducibility checks.

## Security release

For a vulnerability fix:

- coordinate through private security reporting;
- minimize public detail before fixes are available;
- validate all affected supported versions/targets;
- publish clear impact and upgrade guidance;
- rotate or revoke compromised credentials/artifacts when needed;
- follow the disclosure process in `SECURITY.md`.

## Publication

Before publication:

- tag points to the validated commit;
- release notes match changelog and migration docs;
- artifacts and checksums are attached;
- download/install documentation uses the released version;
- updater metadata points to the intended artifacts;
- no draft or local build is mislabeled as official.

## Post-release verification

After publication:

- download artifacts from the public release channel;
- verify checksums;
- rerun minimal smoke tests;
- check installer/update routes;
- confirm documentation links;
- verify reported version and ABI;
- record release evidence and any incident.

## Rollback

If a release is invalid:

- stop promotion and updater distribution;
- preserve evidence;
- mark affected assets clearly;
- publish corrective guidance;
- issue a new patch release rather than silently replacing immutable artifacts where possible;
- document root cause and prevention action.

A release is complete only after public artifact verification, not when CI finishes building.