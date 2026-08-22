# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | yes |
| < 1.0 tags | no |

Project Jericho is pre-release; only the latest `main` receives security fixes.

## Reporting a Vulnerability

**Do not open a public issue for security problems.**

Use GitHub's [private vulnerability reporting](https://github.com/noelchesco5/jericho/security/advisories/new),
or contact the maintainer directly through the repository owner account.

Include:

- Affected module (`ollama.rs`, `system.rs`, `rag/`, `gui/`, ...)
- Steps to reproduce or a proof of concept
- Impact assessment
- Rust toolchain and OS used

You can expect an initial response within **72 hours**.

## Scope

In scope:

- Anything that breaks the local-first guarantee: code that sends data to any
  endpoint other than `localhost`/`127.0.0.1`
- Prompt/RAG content injection leading to arbitrary command execution
- Unsafe deserialization of config files (`config/`, `src/config.rs`)
- Path traversal via RAG document indexing (`src/rag/`)
- Privilege escalation through the system health monitor (`src/system.rs`)
- Supply-chain issues in pinned dependencies (`Cargo.lock`)

Out of scope:

- Issues requiring a malicious local user attacking themselves
- Vulnerabilities in Ollama itself (report upstream at ollama.com)
- Denial of service against the local GUI

## Design Assumptions

Jericho trusts the loopback interface. The app assumes:

1. Ollama runs on `127.0.0.1` and no remote inference is ever contacted
2. No telemetry leaves the machine — this is a hard requirement, not a feature
3. Models pulled via `ollama pull` are trusted per Ollama's own registry model

A patch that violates any of these assumptions will be treated as a critical
security regression regardless of whether it is exploitable.

## Disclosure

We follow coordinated disclosure: please give us 90 days before publishing
details. We will credit reporters in release notes unless anonymity is requested.
