# Security policy

## Supported versions

| Version / branch | Support |
|---|---|
| `master` and current `0.3.8` development line | Active security maintenance |
| Latest published 0.3.x release | Best effort |
| Older snapshots and archived builds | Not supported |

## Reporting a vulnerability

Do **not** open a public issue for a suspected vulnerability.

Use GitHub private vulnerability reporting for this repository:

- open the repository **Security** tab;
- choose **Report a vulnerability**;
- submit a private advisory report.

Repository: `raillen/ori-lang`.

If private reporting is unavailable, contact the repository owner through a private channel listed on the owner's GitHub profile. Do not include exploit details in a public discussion.

## What to include

Provide, when possible:

- affected version, branch, or commit;
- affected platform and target triple;
- impact summary;
- clear reproduction steps;
- minimal proof of concept;
- whether the issue affects compiler, runtime, package flow, LSP, installer, or release artifacts;
- suggested mitigation or fix, when available;
- whether public disclosure is already occurring elsewhere.

Remove real secrets, personal data, and unrelated private source from the report.

## Response targets

Best-effort targets:

- acknowledgement within 72 hours;
- initial triage within 7 days;
- periodic status updates for confirmed issues;
- coordinated disclosure after a validated fix or documented risk decision.

These targets are not a paid support or bug-bounty agreement.

## Disclosure process

1. The report is acknowledged privately.
2. Impact and affected versions are validated.
3. A fix and regression test are prepared.
4. Relevant supported targets and release artifacts are tested.
5. Security release notes and upgrade guidance are prepared when needed.
6. Public disclosure occurs after a fix is available or a disclosure plan is agreed.

## Scope

In scope:

- compiler and CLI under `compiler/`;
- runtime source and staged artifacts;
- standard library;
- package, registry, lockfile, updater, installer, and release workflows;
- LSP and repository editor integrations;
- build, CI, documentation generation, and automation in this repository.

Examples of relevant issues:

- compiler or LSP memory-safety problems;
- runtime double release, use-after-free, invalid layout, or ABI mismatch;
- package archive path traversal;
- command or linker injection;
- registry token leakage;
- malicious project causing unintended filesystem access;
- update or installer integrity failure;
- denial of service with significant practical impact;
- release artifact substitution or metadata mismatch.

Out of scope:

- unrelated third-party services;
- code outside this repository;
- vulnerabilities requiring an already compromised host with no additional impact;
- social engineering or physical attacks;
- normal behavior of an intentionally executed program using documented OS capabilities.

## Safe harbor

Good-faith security research is welcome.

Please:

- avoid privacy violations and unnecessary data access;
- avoid destructive testing and service disruption;
- use minimal proofs of concept;
- stop when unintended impact is observed;
- give maintainers reasonable time to investigate and remediate;
- do not use phishing, social engineering, or credential theft.

The project currently offers no bug bounty.

## Security engineering documentation

See:

- [`docs/security/threat-model.md`](docs/security/threat-model.md);
- [`docs/security/unsafe-code-policy.md`](docs/security/unsafe-code-policy.md);
- [`docs/spec/16-runtime-ffi-safety.md`](docs/spec/16-runtime-ffi-safety.md);
- [`docs/spec/19-abi.md`](docs/spec/19-abi.md).