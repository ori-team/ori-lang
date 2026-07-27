# Supply-chain security

This document covers dependencies, packages, registries, Git sources, installers, updater behavior, CI, and release artifacts.

## Security objectives

- resolve exactly the dependency versions/revisions recorded by the project;
- prevent package/archive/path escape;
- avoid implicit execution of untrusted package code;
- protect registry and release credentials;
- make artifacts attributable to a source commit and build process;
- detect dependency and workflow compromise;
- provide safe update and rollback behavior.

## Dependency sources

Supported source types may include:

- local path;
- Git repository and revision;
- configured registry.

Each resolved dependency should record:

- canonical package identity;
- source type and locator;
- exact version or Git revision;
- integrity data where available;
- dependency graph relationship;
- namespace used by imports.

Mutable Git branches or tags should resolve to and lock an immutable commit.

## Lockfile

A present lockfile is a reproducibility and integrity input.

Requirements:

- deterministic serialization;
- exact source/revision/version;
- stale metadata rejected or deliberately refreshed;
- `--locked` validates without mutation;
- no hidden fallback to a different source;
- dependency-bearing builds without a required valid lockfile follow the documented conservative behavior.

## Package cache

The cache must:

- isolate packages by identity/source/version;
- avoid path collisions and dependency confusion;
- use atomic installation or staging;
- not trust package-provided paths;
- validate manifests before exposing the package;
- recover from interrupted downloads/installs;
- not store credentials inside package content.

Cache cleanup must not delete unrelated user files.

## Archive extraction

Reject:

- absolute paths;
- `..` traversal;
- platform-specific path escape forms;
- entries outside the extraction root after normalization;
- unsafe symlink/hardlink targets;
- unreasonable entry counts or expanded sizes;
- malformed duplicate metadata that changes final output unpredictably.

Extract into a temporary directory, validate, then move atomically into the final cache/package location.

## Registry communication

Registry clients should:

- require HTTPS for remote production registries;
- validate status, content type, and size;
- use timeouts and bounded retries;
- authenticate only to the intended origin;
- redact tokens from logs and diagnostics;
- avoid following redirects that would leak credentials;
- verify integrity metadata when available;
- distinguish not-found, authentication, integrity, and transport failures.

A hosted official registry remains a deferred operational service until ownership, signing, availability, retention, abuse, and incident policies are approved.

## Publishing

Publishing should:

- validate package identity/version and file allowlist;
- exclude build outputs, secrets, caches, VCS internals, and local configuration;
- produce deterministic archives;
- calculate and publish integrity hashes;
- refuse overwrite of immutable published versions unless policy explicitly supports a controlled yanking model;
- redact authentication data;
- record source commit and tool version.

## Git dependencies

- Use argument-array process invocation or a safe Git library.
- Validate repository locator and destination path.
- Limit clone depth/data where appropriate without losing the exact revision.
- Resolve and record the final commit.
- Treat submodules, hooks, filters, and attributes as untrusted.
- Do not execute repository hooks.
- Avoid placing credentials in process arguments or persisted remotes.

## Installers and updater

Installer/update security requires:

- downloads from an explicit approved origin;
- TLS validation;
- expected release/version selection;
- checksum or stronger signature verification;
- staged replacement and rollback;
- refusal of install layouts that cannot be updated safely;
- no silent privilege escalation;
- clear handling of package-manager-owned installations;
- no shell interpolation of untrusted metadata.

Published scripts should be reviewable and version-pinned where practical. “Pipe remote script to shell” instructions require extra scrutiny and a documented safer download/inspect alternative.

## Rust and third-party dependencies

- Pin through `Cargo.lock`.
- Review new dependencies for maintenance, license, security, transitive size, and feature set.
- Disable unnecessary default features.
- Use dependency audit/advisory tooling in CI.
- Track exceptions with owner, rationale, and expiration/review condition.
- Prefer standard library or existing vetted dependencies for small tasks.

## CI and workflows

- Pin third-party actions to trusted major versions initially and consider commit pinning for high-risk release workflows.
- Use least-privilege `permissions`.
- Separate untrusted PR validation from secret-bearing release jobs.
- Avoid executing changed PR scripts with privileged tokens.
- Protect release/tag workflows and environments.
- Preserve logs/artifacts needed for audit without exposing secrets.
- Require review for workflow and release-script changes.

## Release artifacts

Each artifact should be associated with:

- source commit/tag;
- project version;
- target triple;
- toolchain and dependency lock;
- native ABI version;
- build workflow/run;
- checksum;
- package content inventory.

Maturity targets include:

- SBOM;
- signed checksums or signatures;
- provenance attestations;
- reproducibility comparison;
- public verification instructions.

## Threat response

Potential dependency/package/update/release compromise follows private security reporting.

Response may require:

- disabling publish/update routes;
- revoking tokens;
- removing or yanking malicious versions;
- invalidating caches;
- publishing patched artifacts;
- rotating signing credentials;
- user notification and upgrade/removal guidance;
- incident review and new regression gates.

## Validation

Required test families include:

- path traversal and archive bombs;
- malformed/unknown manifests and lockfiles;
- stale lock rejection;
- duplicate/confused package names;
- Git revision locking;
- registry authentication and redirect handling;
- token redaction;
- checksum mismatch;
- interrupted install/update rollback;
- isolated release package smoke;
- workflow permission review.

## Current boundaries

Ori does not currently define arbitrary package build scripts or plugins as a trusted capability. Introducing code execution during dependency resolution/build requires a dedicated RFC, sandbox/security model, permission design, and migration policy.