# Security Policy

## Supported Versions

| Version / Branch | Supported |
| --- | --- |
| `master` (current development branch) | Yes |
| Latest pre-release (`0.3.x-alpha`) | Best effort |
| Older releases and snapshots | No |

## Reporting a Vulnerability

Please do **not** open public issues for security reports.

Use GitHub private reporting:

- <https://github.com/raillen/zenithlang/security/advisories/new>

If the link is unavailable, use the repository Security tab and submit a private report there.

## What To Include In The Report

Please include:

- affected version, branch, or commit
- impact summary (what can happen)
- clear reproduction steps
- proof of concept (minimal)
- suggested fix (if available)

## Response Targets

Current targets (best effort):

- acknowledgment: within 72 hours
- initial triage: within 7 days
- status updates: every 7 days for confirmed issues

## Disclosure Process

- A fix is prepared and validated.
- Security release notes are published when appropriate.
- Public disclosure happens after a fix is available (or risk is accepted and documented).

## Scope

In scope:

- compiler (`compiler/`)
- runtime (`runtime/`)
- standard library (`stdlib/`)
- build and automation files in this repository

Out of scope:

- unrelated third-party systems and services
- code outside this repository
- `zenith-website/` (excluded from the language repository)

## Safe Harbor

Good-faith security research is welcome.

Please avoid:

- privacy violations
- data destruction
- service disruption
- social engineering, phishing, or physical attacks

This project currently does not offer a bug bounty program.