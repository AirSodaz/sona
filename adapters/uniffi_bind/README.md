# `sona-uniffi-bind`

| | |
| --- | --- |
| Package | `sona-uniffi-bind` |
| Path | `adapters/uniffi_bind/` (**historical**; directory says “adapter”) |
| Role | **`host`** — mobile / UniFFI composition root, not an inbound adapter |
| Declared in | `[package.metadata.sona] role` in this crate’s `Cargo.toml` |

## What this crate is

The UniFFI-facing **host**: it owns context lifecycle, wires concrete outbound
adapters and the Sync application, maps typed domain errors to binding results,
and exports the mobile surface (`facade`, bridges, mappers).

Primary composition entry points:

- `src/application_context.rs` — per application-data-dir host context / registry
- `src/facade.rs` — UniFFI-facing API surface

## What this crate is not

- Not an inbound HTTP/TS adapter (`sona-api-server`, `sona-ts-bind` are)
- Not a place to put Core domain rules
- Not “an adapter” for workspace dependency-direction policy

## Dependency rules

Hosts may compose Core, Application, Inbound Adapter, and Outbound Adapter.
This package does that for Android / UniFFI consumers.

Current intentional gaps (product / wiring limits, not missing Core ports) are
locked by the host capability matrix — for example no `sona-model-downloads`,
`sona-media-detector`, or `sona-api-server` dependency today. Do not add them
without updating that matrix and its tests.

## Further reading

- [Architecture guide (EN)](../../docs/architecture.md) — host role, capability matrix, composition roots
- [架构指南（简体中文）](../../docs/architecture.zh-CN.md)
- Physical path relocation (for example toward `platforms/…`) is a dedicated later slice
