# Security Policy

## Supported Versions

Pre-v0.1. Only the latest `main` and the most recent tagged release receive
security fixes.

| Version | Supported |
|---------|-----------|
| `main` | Yes |
| Latest `v0.x` tag | Yes |
| Older tags | No |

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately via one of:

1. Email **hi@lucasmarques.me** with the subject `[kaizen security] <short summary>`.
2. GitHub's private vulnerability reporting: **Security → Report a vulnerability**
   on the repository.

Please include:

- A description of the vulnerability and its impact.
- Steps to reproduce, or a proof-of-concept.
- The affected version / commit.
- Any known mitigations.

## Response

- Acknowledgement within **72 hours**.
- Triage and severity assessment within **7 days**.
- Coordinated disclosure: we aim to ship a fix before public disclosure and
  will credit reporters in the release notes unless you request otherwise.

## Scope

In scope:

- The `kaizen` crate and its binary.
- CI / release workflows in this repository.
- The sync / ingest contract implementation.

Out of scope:

- Third-party dependencies — report upstream; we'll bump once patched.
- User misconfiguration (e.g., committing `.kaizen/kaizen.db` to a public repo).
- Attacks requiring root / local privilege that bypass OS isolation.

## Redaction guarantees

The sync path applies `redact` to every outbound event before any HTTP POST.
If you find a code path that bypasses redaction or leaks secrets, env vars,
absolute paths, or git emails, treat it as a security issue and report it here.
