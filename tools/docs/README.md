# Documentation tooling

This directory contains permanent validation and generation tools for the canonical documentation framework.

## Validation

```bash
python tools/docs/check_docs.py
```

The validator checks:

- canonical document paths from `docs/catalog.yaml`;
- project version agreement with `compiler/Cargo.toml`;
- required project entry points;
- Markdown relative links in active canonical documentation;
- retired project-identity terms across UTF-8 repository text;
- basic ATLAS/catalog routing expectations.

It intentionally distinguishes current status from historical release numbers. Archived documents may discuss older versions, but retired project identity is not allowed anywhere in versioned text.

## CI

`.github/workflows/documentation.yml` runs the validator for documentation, catalog, version, and tooling changes.

A validation failure should be corrected at the canonical source. Do not add broad exclusions merely to silence drift.

## Future tools

The framework may add focused tools for:

- metadata/front-matter validation;
- orphan-document detection;
- EN/PT parallel-document checks;
- executable Ori example validation;
- specification-to-conformance reports;
- ADR/RFC/plan status validation;
- archive classification;
- ATLAS and context-pack generation.

Generators must remain deterministic and inspectable. Generated files do not become independent sources of truth.