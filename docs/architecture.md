# Sona Architecture

[English](architecture.md) | [简体中文](architecture.zh-CN.md) | [Project README](../README.md) | [Contributing](../CONTRIBUTING.md)

<a id="architecture-roles"></a>
## Architecture roles

Sona uses six stable roles. The role is the reviewed dependency contract, not a description inferred from a directory name.

| Package | Role |
| --- | --- |
| `sona-core` | core |
| `sona-sync` | application |
| `sona-api-server` | inbound-adapter |
| `sona-ts-bind` | inbound-adapter |
| `sona-archive` | outbound-adapter |
| `sona-export` | outbound-adapter |
| `sona-local-asr` | outbound-adapter |
| `sona-media-detector` | outbound-adapter |
| `sona-model-downloads` | outbound-adapter |
| `sona-online-asr` | outbound-adapter |
| `sona-online-llm` | outbound-adapter |
| `sona-recovery-fs` | outbound-adapter |
| `sona-runtime-fs` | outbound-adapter |
| `sona-sqlite` | outbound-adapter |
| `sona-sync-webdav` | outbound-adapter |
| `sona` | host |
| `sona-cli` | host |
| `sona-uniffi-bind` | host |
| `sona-uniffi-bindgen` | tool |

Core contains domain contracts and Core-owned ports. Application coordinates use cases through those contracts. Inbound Adapter translates caller input; Outbound Adapter implements a Core-owned port. A Host composes the application for a runtime, while a Tool supports development or code generation.

<a id="dependency-direction"></a>
## Dependency direction

```text
Core <- Application <- Inbound Adapter <- Host
Core <- Outbound Adapter <------------- Host
             ^
             +-- Application may call outbound ports through Core-owned traits
```

Dependencies point only toward the roles shown by this model. Core has no runtime dependency on another workspace role. Application depends on Core; adapters depend on Core or Application; Hosts may compose Core, Application, Inbound Adapter, and Outbound Adapter. Tools have no runtime role dependencies.

<a id="composition-roots"></a>
## Composition roots

- Desktop: `platforms/desktop/src/app/setup.rs` plus `platforms/desktop/src/platform/` compose the desktop runtime.
- CLI: `platforms/cli/src/lib.rs` and individual command modules compose CLI commands.
- UniFFI/mobile: `adapters/uniffi_bind/src/application_context.rs` and `adapters/uniffi_bind/src/facade.rs` compose the mobile-facing surface. The `adapters/uniffi_bind` directory name is historical: `sona-uniffi-bind` has the host role, not an adapter role.

<a id="error-boundaries"></a>
## Error boundaries

Domain and application errors remain typed across Core, Application, and adapter code. Tauri, UniFFI, CLI, and HTTP route handlers perform the final compatibility mapping for their callers. They are the only public string-conversion boundaries: internal layers must not replace typed failures with public strings.

<a id="compatibility-policy"></a>
## Compatibility policy

Public caller contracts may require strings, status-and-string tuples, or language-specific error values. Keep that conversion at the final Tauri, UniFFI, CLI, or HTTP boundary, where it can preserve the established contract. New domain and application APIs must expose typed errors; adapters and hosts must not move compatibility strings inward. When an external contract changes, add an explicit boundary mapping and a focused contract test rather than weakening the inner typed API.

<a id="reviewed-exceptions"></a>
## Reviewed exceptions

The following outbound-adapter dependencies are currently reviewed exceptions. They are intentionally named in the policy test and must be reconsidered when their implementation changes.

| Edge | Reason |
| --- | --- |
| `sona-model-downloads->sona-runtime-fs` | model installation completeness currently reuses the runtime filesystem probe |
| `sona-recovery-fs->sona-runtime-fs` | the recovery adapter currently composes the shared real filesystem, path-status, clock, and atomic JSON helpers |

<a id="verification"></a>
## Verification

Run the stable-guide contract after changing these guides or the role registry:

```text
rtk node --test --test-name-pattern "stable architecture guides" scripts/crate-boundaries.test.js
```

Run the complete crate-boundary script test when changing the package-role registry or its dependency rules:

```text
rtk node --test scripts/crate-boundaries.test.js
```
