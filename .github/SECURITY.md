# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you discover a security vulnerability, report it privately via
[GitHub Security Advisories](https://github.com/itsakeyfut/uasset-lens/security/advisories/new).

Please include:

- A description of the vulnerability and its potential impact
- Steps to reproduce or a minimal proof-of-concept (a crafted `.uasset` file if applicable)
- Affected crate(s) and version(s)
- Any suggested mitigations if known

You can expect an acknowledgment within **7 days** and a resolution or status update
within **30 days**.

## Scope

uasset-lens parses untrusted `.uasset` / `.umap` binary files from disk.
The primary attack surface is the binary parser in `crates/scanner/`:

- **In scope:** panics, out-of-bounds reads, or incorrect behavior triggered by
  a crafted `.uasset` file; path traversal in `AssetPath` construction;
  SQL injection in `crates/asset-db/`.
- **Out of scope:** vulnerabilities in Unreal Engine itself or in third-party
  crates (report those to the respective upstream maintainers).

## Dependency Vulnerabilities

Known advisories in dependencies are tracked via `cargo deny check` (see `deny.toml`).
If you find a transitive dependency with an untracked advisory, a GitHub Issue is
appropriate for that report.
