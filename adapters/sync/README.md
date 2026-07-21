# `sona-sync`

| | |
| --- | --- |
| Package | `sona-sync` |
| Path | `adapters/sync/` (**historical**; directory says “adapter”) |
| Role | **`application`** — not an outbound adapter |
| Declared in | `[package.metadata.sona] role` in this crate’s `Cargo.toml` |

## What this crate is

Provider-neutral encrypted Sync **application** runtime: vault lifecycle,
provider registry, run cycles, and orchestration that depends only on
`sona-core` ports and traits.

Concrete providers (for example WebDAV) live in outbound adapters such as
`sona-sync-webdav`. Hosts (Desktop, UniFFI) compose this application; they do
not reimplement Sync session state machines.

## What this crate is not

- Not a filesystem or network adapter
- Not a host composition root
- Not “just another entry under `adapters/`” for dependency policy purposes

## Dependency rules

- Runtime: Core only (`application` → `core`)
- Dev-only may use outbound adapters (for example `sona-sqlite`) in tests
- Must stay provider-neutral: no WebDAV/HTTP client stack in this package

## Further reading

- [Architecture guide (EN)](../../docs/architecture.md) — roles, directory vs role, composition roots
- [架构指南（简体中文）](../../docs/architecture.zh-CN.md)
- Physical path relocation is a dedicated later slice; do not move this tree casually
