# Sona Architecture

[English](architecture.md) | [简体中文](architecture.zh-CN.md) | [Project README](../README.md) | [Contributing](../CONTRIBUTING.md)

<a id="architecture-roles"></a>
## Architecture roles

Sona uses six stable roles. The role is the reviewed dependency contract; workspace paths mirror that contract so the directory tree is also a reliable navigation aid.

| Package | Role |
| --- | --- |
| `sona-core` | core |
| `sona-application` | application |
| `sona-sync` | application |
| `sona-api-server` | inbound-adapter |
| `sona-ts-bind` | inbound-adapter |
| `sona-archive` | outbound-adapter |
| `sona-export` | outbound-adapter |
| `sona-sherpa-onnx` | outbound-adapter |
| `sona-llama-cpp` | outbound-adapter |
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

Each workspace package lives under the root for its reviewed role. The `[package.metadata.sona] role` field remains the machine-readable contract, and `scripts/crate-boundaries.test.js` rejects path/role mismatches.

| Path | Package | Role | Notes |
| --- | --- | --- | --- |
| `core/` | `sona-core` | core | Domain contracts and Core-owned ports |
| `application/` | `sona-application` | application | Shared use-case services |
| `application/sync/` | `sona-sync` | application | Provider-neutral Sync application runtime |
| `adapters/api_server/` | `sona-api-server` | inbound-adapter | |
| `adapters/ts_bind/` | `sona-ts-bind` | inbound-adapter | |
| `adapters/archive/` | `sona-archive` | outbound-adapter | |
| `adapters/export/` | `sona-export` | outbound-adapter | |
| `providers/asr/local/sherpa_onnx/` | `sona-sherpa-onnx` | provider | |
| `providers/asr/local/llama_cpp/` | `sona-llama-cpp` | provider | Qwen3-ASR batch inference through llama.cpp |
| `adapters/media_detector/` | `sona-media-detector` | outbound-adapter | |
| `adapters/model_downloads/` | `sona-model-downloads` | outbound-adapter | |
| `providers/asr/online/` | `sona-online-asr` | provider | |
| `adapters/online_llm/` | `sona-online-llm` | outbound-adapter | |
| `adapters/recovery_fs/` | `sona-recovery-fs` | outbound-adapter | |
| `adapters/runtime_fs/` | `sona-runtime-fs` | outbound-adapter | |
| `adapters/sqlite/` | `sona-sqlite` | outbound-adapter | Owns `SqliteApplicationContext` |
| `adapters/sync_webdav/` | `sona-sync-webdav` | outbound-adapter | |
| `platforms/desktop/` | `sona` | host | Desktop Tauri host |
| `platforms/cli/` | `sona-cli` | host | |
| `platforms/uniffi/` | `sona-uniffi-bind` | host | Mobile / UniFFI composition root |
| `tools/uniffi_bindgen/` | `sona-uniffi-bindgen` | tool | |

Role roots are `core/`, `application/`, `adapters/`, `providers/`, `platforms/`, and `tools/`. Inbound and outbound adapters share the `adapters/` root and remain distinguished by manifest metadata. Provider crates under `providers/` implement engine-specific ASR backends behind Core ports.

<a id="composition-roots"></a>
## Composition roots

- Desktop: `platforms/desktop/src/app/setup.rs` plus `platforms/desktop/src/platform/` compose the desktop runtime.
- CLI: `platforms/cli/src/lib.rs` and individual command modules compose CLI commands.
- UniFFI/mobile: `platforms/uniffi/src/application_context.rs` composes the mobile-facing runtime, and `platforms/uniffi/src/lib.rs` publishes the exported surface.

<a id="desktop-frontend-dependencies"></a>
### Desktop frontend dependency direction

Desktop frontend imports point from views and hooks toward feature services and stores, then toward stable domain DTOs and `services/tauri` platform gateways. `src/types/` and `src/constants/` must not import services, stores, hooks, or components. Feature services may coordinate stores but must not import hooks or components; stores must not import UI modules. The complete production import graph must remain acyclic.

All static and dynamic `@tauri-apps/*` imports live under `src/services/tauri/platform/`, except the centralized `@tauri-apps/api/core` invocation adapter at `src/services/tauri/invoke.ts`. Tauri command contracts depend only on generated bindings, command names, and DTOs owned by `src/types/`; they never depend on a business service. `scripts/frontend-architecture.test.js` enforces these rules in `test:scripts` and PR CI.

The UniFFI surface is exactly two layers: an `#[uniffi::export]` free function in `lib.rs` delegates straight into the matching `*_bridge` module. The former `SonaCoreFacade` type in `facade.rs` forwarded every call a second time without adding behaviour and has been merged into `lib.rs`. Do not reintroduce an intermediate forwarding type; `scripts/multisurface-contracts.test.js` enforces this, allowing only `release_application_context` to call the composition root directly.

Desktop and UniFFI share the SQLite composition type `SqliteApplicationContext` from `sona-sqlite`, but each host still owns its own wiring, lifecycle, and error mapping. The stateless CLI does not depend on this composition type. There is no separate shared application-composition crate yet.

**One context per application-data directory.** The UniFFI registry caches contexts by normalized path, and its capacity is a *soft* limit: an entry a caller still holds is never evicted, because reopening that path would create a second connection pool, migration run, and service graph for one directory. The cache may exceed capacity until its callers release.

**Two ways in, one implementation.** UniFFI bridges take a `ContextSource` rather than a directory string, so the registry lookup happens at one edge instead of inside domain code. A source is either a path — resolved through the registry, which is what the legacy free functions pass — or an owned context, which resolves to itself and never touches the registry.

`SonaContext` is the explicit composition root built on that: it resolves once at construction, holds the result, and exposes the directory-scoped operations as methods. Holding it also pins the registry entry, so the handle's context stays the one context for its directory for as long as it lives.

Its operations are generated. After adding or changing a directory-scoped export, run:

```text
pnpm run generate:sona-context
```

`scripts/sona-context-generated.test.js` fails when the checked-in file no longer matches the generator, and `scripts/multisurface-contracts.test.js` asserts the handle covers every directory-scoped operation — omissions must be justified in `LIFECYCLE_ONLY`.

The free functions remain: this is additive, and the exported ABI kept every existing entry point.

### Application ownership today

Use-case services (History, Tag, Automation, Backup, Recovery, Config, Dashboard, Diagnostics, Export, StorageUsage, TaskLedger, LLM runtime, and LLM tasks) live in `sona-application` (`application/`). Each service holds only the port dependencies it needs and delegates to Core-owned port traits. Domain types, port trait definitions, and errors remain in `sona-core`.

The other standalone Application-role package is `sona-sync` (`application/sync/`), isolated because Sync needs a provider-neutral vault and lifecycle. Do not create additional Application crates per domain; consolidate new use-case services into `sona-application` unless a clear isolation boundary warrants a separate crate.

Live transcription is coordinated by `sona-application` through `LiveTranscriptionCoordinator`. Core owns the ASR contract, typed audio-frame cursor, inference-spec equality, and per-consumer output policy; it does not own capture devices or UI lifecycles. `StreamingAsrFactoryPort` is implemented by the Desktop composition root, which selects the local or online adapter and reuses its model/connection resources. Desktop owns CPAL capture leases, recording writers, Tauri commands, and event delivery. CLI and UniFFI/Android use the same Core frame port while retaining independent sessions, so their future adoption of the coordinator does not require a second streaming API. Only one pipeline feeds a source frame for an identical source epoch, input transform, and inference spec; consumers keep independent mailboxes and output post-processing.

### Core module map (orientation)

- `core/src/domain/` holds shared **product identity enums** used across LLM and automation (for example `LlmProvider`, polish presets, summary templates). It is not the home of all domain logic; History, Tag, Transcription, and other domains live in their own modules under `core/src/`.
- `core/src/history/` owns history records, validation and editing rules, and the `HistoryStore` trait (`history/store.rs`, re-exported as `sona_core::history_store`); query and mutation use-case services live in `application/src/history/`.

<a id="port-placement"></a>
## Port placement

Core owns every port, but not in one directory. Two placements exist and the distinction is the reviewed contract, not a stylistic preference.

| Kind | Home | Test |
| --- | --- | --- |
| Capability port | `core/src/ports/<capability>.rs` | Domain-agnostic; more than one domain could use it |
| Domain-owned port | `core/src/<domain>/` | Only that domain's use cases call it |

**Capability ports** name an infrastructure capability, never a domain aggregate: `FileSystem`, `PathProvider`, `UnixMillisClock`, `EventEmitter`, `BatchTranscriber`, `LlmCompletionPort`, `GpuAvailabilityProvider`. A trait belongs here only if moving it into a single domain would be wrong.

**Domain-owned ports** are the stores, repositories, and collaborators that belong to one domain: `HistoryStore`, `TagStore`, `AutomationStore`, `BackupStateRepository`, `RecoverySnapshotStore`, `SyncObjectStore`. They live under their domain module so the domain stays self-describing.

Two rules follow, and both are enforced by `scripts/core-port-placement.test.js`:

1. No `*Store` or `*Repository` trait may be declared in `core/src/ports/`. A persistence-shaped trait is by definition owned by a domain.
2. No trait in `core/src/ports/` may be named after a domain aggregate (History, Tag, Automation, Backup, Recovery, TaskLedger, Dashboard, StorageUsage, AppConfig).

New domain ports should use `core/src/<domain>/ports.rs`, the form already used by `backup`, `dashboard`, `export`, `storage_usage`, and `sync`. Existing `repository.rs` / `store.rs` placements in `automation`, `config`, `history`, `recovery`, `tag`, and `task_ledger` are grandfathered: they satisfy both rules above, and consolidating their file names is a separate slice, not a free cleanup.

<a id="host-capability-matrix"></a>
## Host capability matrix

Capabilities are derived from current workspace dependencies and product scope. A check mark means the host wires the capability today. "Out of scope" means an intentional product boundary, not a missing dependency by accident.

| Capability | Desktop (`sona`) | CLI (`sona-cli`) | UniFFI (`sona-uniffi-bind`) |
| --- | --- | --- | --- |
| SQLite / History / Tag | yes | out of scope | yes |
| Local ASR (sherpa-onnx) | yes | yes | yes |
| Local ASR (llama.cpp) | yes | yes | no |
| Online ASR | yes | yes | yes |
| Online LLM | yes | out of scope | yes |
| Model downloads | yes | yes | yes |
| Media detector | yes | yes | no |
| API server | yes | yes | no |
| Sync (application + WebDAV) | yes | out of scope | yes |
| TypeScript/Tauri contract bind | yes | no | no |
| Archive / recovery | yes | out of scope | yes |
| Export / runtime-fs | yes | yes | yes |

The Desktop and CLI model downloader installs the `ggml-org/Qwen3-ASR-0.6B-GGUF`
and `ggml-org/Qwen3-ASR-1.7B-GGUF` Q8_0 presets as verified multi-file bundles.
The main GGUF and matching mmproj are downloaded into a staging directory and
published together, so an incomplete pair is never reported as installed.
UniFFI and Android continue to expose sherpa-onnx local ASR models only.

CLI is a stateless transcription host: application persistence, History/Tag, Online LLM, archive/recovery, and Sync are intentional product boundaries. Online ASR credentials come from an environment variable and are never stored by the CLI. UniFFI's media detection gap is a current host wiring limit, not a Core port absence.

<a id="error-boundaries"></a>
## Error boundaries

Domain and application errors remain typed across Core, Application, and adapter code. Tauri, UniFFI, CLI, and HTTP route handlers perform the final compatibility mapping for their callers. They are the only public string-conversion boundaries: internal layers must not replace typed failures with public strings.

<a id="compatibility-policy"></a>
## Compatibility policy

Public caller contracts may require strings, status-and-string tuples, or language-specific error values. Keep that conversion at the final Tauri, UniFFI, CLI, or HTTP boundary, where it can preserve the established contract. New domain and application APIs must expose typed errors; adapters and hosts must not move compatibility strings inward. When an external contract changes, add an explicit boundary mapping and a focused contract test rather than weakening the inner typed API.

<a id="reviewed-exceptions"></a>
## Reviewed exceptions

There are currently no registered outbound-adapter-to-outbound-adapter exceptions. Each outbound adapter should depend only on Core (and reviewed Application edges such as `sona-sync-webdav -> sona-sync`).

<a id="compatibility-debt"></a>
## Compatibility debt inventory

These items are explicit and allowed during the current compatibility window. They are not free cleanups; do not rename or delete them without a dedicated migration slice and contract tests.

### Project to Tag

- **Canonical write model:** Tag (`TagStore` / SQLite tag tables).
- **Removed empty module:** `core/src/project/` is gone; do not recreate an empty Project core module.
- **Host compatibility leaves that still use Project naming:**
  - Desktop Tauri: `history_update_project_assignments`, `history_reassign_project` in `platforms/desktop/src/commands/history.rs` (delegates to tag assignment).
  - UniFFI JSON: Project-named history/config helpers under `platforms/uniffi/src/` (for example project assignment and effective-config project JSON parameters).
  - Desktop frontend product paths still named Project: `platforms/desktop/frontend/src/types/project.ts`, `services/projectService.ts`, `stores/projectStore.ts`, `components/projects/*`, and `components/ProjectsView.tsx`.
- **Policy:** keep public Project names during the compatibility window; physical frontend/API renames are a later slice.

### UniFFI typed-contract inventory

Every `*_json` UniFFI export is classified in `scripts/multisurface-contracts.test.js` (`UNIFFI_JSON_ONLY_EXPORTS`). A new `*_json` export without a `*_v1` sibling fails that test until it is classified.

- **`dynamic-leaf`** — permanently allowed. The payload is arbitrary user config, a provider extension document, or the legacy Project compatibility surface, so this binding does not own its schema.
- **`pending-migration`** — reviewed debt. The export transports a complete snapshot, record, request, or result as a JSON string and must gain a typed `_v1` sibling. Only `complete_llm_json` remains: `LlmCompletionOptions` pulls in the response-format, prompt-cache, and capability policy trees, which is a separate slice.

Typed domains keep both surfaces: `_json` stays as a compatibility delegate and `_v1` carries the typed contract. Every domain is migrated: Tag, History, Task Ledger, Recovery, Automation, Storage Usage, Export, Backup, Dashboard, Diagnostics, Sync, and LLM.

Some `*_json` exports are permanent by design rather than debt. Besides arbitrary config and the legacy Project surface, the **parser entry points** (`llm_config_from_json`, `polish_segments_request_from_json`, and their two siblings) exist precisely to turn the app's stored JSON into a typed record — the typed form is their *output*, so a typed input would make them identity functions.

### Credentials never cross as printable fields

UniFFI renders a `Record` as a Kotlin `data class`, and its generated `toString()` prints every field. A credential held as a plain `String` would therefore leak into any log line that formats the record. Credentials cross as object handles instead — `FfiSecret`, `FfiOnlineAsrApiKey` — whose Kotlin `toString()` is their identity and whose Rust `Debug` redacts the value. Their `expose()` reader is deliberately not `#[uniffi::export]`ed, so the secret never becomes a readable property on the generated handle.

`scripts/multisurface-contracts.test.js` enforces this: any `uniffi::Record` field whose name looks like a credential must be an opaque handle.

### Other reviewed debt

- Resolved outbound edges: `model-downloads` and `recovery-fs` no longer depend on `runtime-fs`; completeness rules live in Core and I/O stays local to each adapter.
- Sync legacy secret-store registration and raw WebDAV wire shapes as host delegates for older callers.
- `sona-sqlite` depends on `tempfile` in production dependencies because `Database` owns a read-only snapshot `TempDir` lifecycle; this is not a mistaken dev-only dependency.
- CLI persistence, History/Tag, Online LLM, archive/recovery, and Sync remain intentionally out of scope.

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

Run the host capability matrix contract when changing host dependencies or the matrix rows above:

```text
rtk node --test scripts/host-capability-matrix.test.js
```

Run the host wiring inventory when changing host composition roots or production wiring of matrix capabilities:

```text
rtk node --test scripts/host-wiring-inventory.test.js
```

Run the port placement contract after adding or moving a port trait in Core:

```text
rtk node --test scripts/core-port-placement.test.js
```
