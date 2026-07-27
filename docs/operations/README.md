# Operations documentation

Operations documents describe repeatable procedures for developing, validating, packaging, releasing, reproducing, and recovering the Ori toolchain.

## Canonical documents

- [`development.md`](development.md) — local environment, commands, runtime staging, and troubleshooting route.
- [`release.md`](release.md) — versioning, artifact creation, smoke validation, provenance, and publication.
- [`reproducible-builds.md`](reproducible-builds.md) — declared inputs, nondeterminism, checksums, SBOM, and artifact provenance.
- [`../install.md`](../install.md) — end-user installation.
- [`../../runtime/README.md`](../../runtime/README.md) — target runtime artifacts and staging details.

## Operations principle

A procedure must be executable from a clearly stated working directory and must identify prerequisites, inputs, outputs, validation, failure handling, and recovery steps.

Do not duplicate command matrices across README, AGENTS, contributing, and operations documents. Short entry documents link here for detail.