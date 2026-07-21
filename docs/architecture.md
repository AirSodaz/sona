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

<a id="directory-vs-role"></a>
## Directory versus role

Directory names are organizational. The reviewed role lives in each package's
`[package.metadata.sona] role` field and in the role table above. Do not infer
role from path alone.

| Path | Package | Role | Notes |
| --- | --- | --- | --- |
| `core/` | `sona-core` | core | Domain contracts and Core-owned ports |
| `adapters/sync/` | `sona-sync` | application | Directory says adapters; role is application |
| `adapters/api_server/` | `sona-api-server` | inbound-adapter | |
| `adapters/ts_bind/` | `sona-ts-bind` | inbound-adapter | |
| `adapters/archive/` | `sona-archive` | outbound-adapter | |
| `adapters/export/` | `sona-export` | outbound-adapter | |
| `adapters/local_asr/` | `sona-local-asr` | outbound-adapter | |
| `adapters/media_detector/` | `sona-media-detector` | outbound-adapter | |
| `adapters/model_downloads/` | `sona-model-downloads` | outbound-adapter | |
| `adapters/online_asr/` | `sona-online-asr` | outbound-adapter | |
| `adapters/online_llm/` | `sona-online-llm` | outbound-adapter | |
| `adapters/recovery_fs/` | `sona-recovery-fs` | outbound-adapter | |
| `adapters/runtime_fs/` | `sona-runtime-fs` | outbound-adapter | |
| `adapters/sqlite/` | `sona-sqlite` | outbound-adapter | Owns `SqliteApplicationContext` |
| `adapters/sync_webdav/` | `sona-sync-webdav` | outbound-adapter | |
| `adapters/uniffi_bind/` | `sona-uniffi-bind` | host | Historical directory name; host composition root, not an adapter |
| `platforms/desktop/` | `sona` | host | Desktop Tauri host |
| `platforms/cli/` | `sona-cli` | host | |
| `tools/uniffi_bindgen/` | `sona-uniffi-bindgen` | tool | |

Physical relocation of historical paths is out of scope until a dedicated slice
owns the move.

<a id="composition-roots"></a>
## Composition roots

- Desktop: `platforms/desktop/src/app/setup.rs` plus `platforms/desktop/src/platform/` compose the desktop runtime.
- CLI: `platforms/cli/src/lib.rs` and individual command modules compose CLI commands.
- UniFFI/mobile: `adapters/uniffi_bind/src/application_context.rs` and `adapters/uniffi_bind/src/facade.rs` compose the mobile-facing surface. The `adapters/uniffi_bind` directory name is historical: `sona-uniffi-bind` has the host role, not an adapter role.

Hosts share the SQLite composition type `SqliteApplicationContext` from
`sona-sqlite`, but each host still owns its own wiring, lifecycle, and error
mapping. There is no separate shared application-composition crate yet.

<a id="host-capability-matrix"></a>
## Host capability matrix

Capabilities are derived from current workspace dependencies and product scope.
A check mark means the host wires the capability today. "Out of scope" means an
intentional product boundary, not a missing dependency by accident.

| Capability | Desktop (`sona`) | CLI (`sona-cli`) | UniFFI (`sona-uniffi-bind`) |
| --- | --- | --- | --- |
| SQLite / History / Tag | yes | yes | yes |
| Local ASR | yes | yes | yes |
| Online ASR | yes | no | yes |
| Online LLM | yes | yes | yes |
| Model downloads | yes | yes | no |
| Media detector | yes | yes | no |
| API server | yes | yes | no |
| Sync (application + WebDAV) | yes | out of scope | yes |
| TypeScript/Tauri contract bind | yes | no | no |
| Archive / export / recovery / runtime-fs | yes | yes | yes |

CLI Sync remains undefined product scope and must not be added until that scope
is specified. UniFFI gaps for model downloads and media detection are current
host wiring limits, not Core port absences.

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

<a id="compatibility-debt"></a>
## Compatibility debt inventory

These items are explicit and allowed during the current compatibility window.
They are not free cleanups; do not rename or delete them without a dedicated
migration slice and contract tests.

### Project to Tag

- **Canonical write model:** Tag (`TagStore` / SQLite tag tables).
- **Removed empty module:** `core/src/project/` is gone; do not recreate an empty
  Project core module.
- **Host compatibility leaves that still use Project naming:**
  - Desktop Tauri: `history_update_project_assignments`, `history_reassign_project`
    in `platforms/desktop/src/commands/history.rs` (delegates to tag assignment).
  - UniFFI JSON: Project-named history/config helpers under
    `adapters/uniffi_bind/src/` (for example project assignment and effective-config
    project JSON parameters).
  - Desktop frontend product paths still named Project:
    `platforms/desktop/frontend/src/types/project.ts`,
    `services/projectService.ts`, `stores/projectStore.ts`,
    `components/projects/*`, and `components/ProjectsView.tsx`.
- **Policy:** keep public Project names during the compatibility window; physical
  frontend/API renames are a later slice.

### Other reviewed debt

- Outbound adapter edges listed under [Reviewed exceptions](#reviewed-exceptions).
- Sync legacy secret-store registration and raw WebDAV wire shapes as host
  delegates for older callers.
- `sona-sqlite` depends on `tempfile` in production dependencies because
  `Database` owns a read-only snapshot `TempDir` lifecycle; this is not a
  mistaken dev-only dependency.
- CLI Sync remains out of scope until product scope is defined.

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
