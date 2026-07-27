# Documentation tooling

This directory contains validation and migration tools for the canonical documentation framework.

## Permanent validation

```bash
python tools/docs/check_docs.py
```

The validator checks:

- canonical document paths from `docs/catalog.yaml`;
- project version agreement with `compiler/Cargo.toml`;
- required project entry points;
- Markdown relative links in active canonical documentation;
- removed project-identity terms;
- basic ATLAS/catalog routing expectations.

It intentionally avoids treating archived syntax or historical version numbers as current status, but retired project identity is forbidden across text files.

## Migration utilities

Migration utilities are temporary and narrowly scoped. They must:

- preserve unrelated content;
- print every changed path;
- be idempotent;
- have a clear removal condition;
- not remain part of ordinary developer workflow after migration completes.

`remove_legacy_identity.py` exists only to safely sanitize large historical text files without replacing or truncating them. Remove it after the migration PR has verified zero remaining matches.