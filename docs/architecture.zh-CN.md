# Sona 架构

[English](architecture.md) | [简体中文](architecture.zh-CN.md) | [项目 README](../README.zh-CN.md) | [参与贡献](../CONTRIBUTING.md)

<a id="architecture-roles"></a>
## 架构角色

Sona 使用六种稳定角色。角色是经过评审的依赖契约，而不是根据目录名推断出的描述。

| 包 | 角色 |
| --- | --- |
| `sona-core` | core |
| `sona-application` | application |
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

Core 包含领域契约和由 Core 所有的端口。Application 通过这些契约协调用例。Inbound Adapter 转换调用方输入；Outbound Adapter 实现由 Core 所有的端口。Host 为运行时组合应用程序，Tool 支持开发或代码生成。

<a id="dependency-direction"></a>
## 依赖方向

```text
Core <- Application <- Inbound Adapter <- Host
Core <- Outbound Adapter <------------- Host
             ^
             +-- Application may call outbound ports through Core-owned traits
```

依赖只能指向这个模型所示的角色。Core 不依赖其他工作区运行时角色。Application 依赖 Core；适配器依赖 Core 或 Application；Host 可以组合 Core、Application、Inbound Adapter 和 Outbound Adapter。Tool 没有运行时角色依赖。

<a id="directory-vs-role"></a>
## 目录与角色

目录名只用于组织代码。经过评审的角色写在各包的 `[package.metadata.sona] role` 字段以及上文角色表中。不要仅凭路径推断角色。

| 路径 | 包 | 角色 | 说明 |
| --- | --- | --- | --- |
| `core/` | `sona-core` | core | 领域契约与由 Core 所有的端口 |
| `application/` | `sona-application` | application | 用例服务；目录名与角色一致 |
| `adapters/sync/` | `sona-sync` | application | 目录在 adapters 下，角色是 application |
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
| `adapters/sqlite/` | `sona-sqlite` | outbound-adapter | 拥有 `SqliteApplicationContext` |
| `adapters/sync_webdav/` | `sona-sync-webdav` | outbound-adapter | |
| `adapters/uniffi_bind/` | `sona-uniffi-bind` | host | 历史目录名；是 host 组合根，不是适配器 |
| `platforms/desktop/` | `sona` | host | 桌面 Tauri Host |
| `platforms/cli/` | `sona-cli` | host | |
| `tools/uniffi_bindgen/` | `sona-uniffi-bindgen` | tool | |

历史路径的物理搬家不在本指南范围内，需由专门切片负责。

<a id="composition-roots"></a>
## 组合根

- 桌面端：`platforms/desktop/src/app/setup.rs` 与 `platforms/desktop/src/platform/` 组合桌面运行时。
- CLI：`platforms/cli/src/lib.rs` 与各个命令模块组合 CLI 命令。
- UniFFI/移动端：`adapters/uniffi_bind/src/application_context.rs` 组合面向移动端的运行时，`adapters/uniffi_bind/src/lib.rs` 发布对外导出面。目录名 `adapters/uniffi_bind` 是历史遗留名称：`sona-uniffi-bind` 的角色是 host，而不是适配器角色。

UniFFI 导出面只有两层：`lib.rs` 中的 `#[uniffi::export]` 自由函数直接委托到对应的 `*_bridge` 模块。原先 `facade.rs` 中的 `SonaCoreFacade` 类型只是把每个调用再转发一次、不附加任何行为，现已合并进 `lib.rs`。不要重新引入中间转发类型； `scripts/multisurface-contracts.test.js` 会强制这一约束，仅允许 `release_application_context` 直接调用组合根。

各 Host 共享 `sona-sqlite` 提供的 `SqliteApplicationContext`，但接线、生命周期与错误映射仍由各 Host 自行拥有。目前还没有单独的共享 application-composition crate。

**每个应用数据目录只有一个 context。** UniFFI registry 按规范化路径缓存 context，其容量是**软**上限：仍被调用方持有的条目永不驱逐，因为重新打开该路径会为同一个目录创建第二套连接池、第二次迁移和第二个服务图。在调用方释放之前，缓存允许超出容量。

**两个入口，一份实现。** UniFFI bridge 接收 `ContextSource` 而非目录字符串，因此 registry 查表发生在**单一边界**上，而不是散落在领域代码里。一个 source 要么是路径（走 registry，即遗留自由函数传入的形式），要么是已持有的 context （解析为自身，完全不碰 registry）。

`SonaContext` 就是建立在此之上的显式组合根：构造时解析一次并持有结果，把目录相关的操作以方法形式暴露。持有它同时会**钉住** registry 条目，因此只要句柄存活，它的 context 就始终是该目录的唯一 context。

它的操作是**生成的**。新增或修改目录相关导出后，请运行：

```text
pnpm run generate:sona-context
```

当签入文件与生成器输出不一致时 `scripts/sona-context-generated.test.js` 会失败； `scripts/multisurface-contracts.test.js` 断言句柄覆盖了每一个目录相关操作—— 故意不提供的操作必须在 `LIFECYCLE_ONLY` 里写明理由。

自由函数保留：本次改动是**增量的**，导出 ABI 保留了全部既有入口。

### 当前的 Application 归属

用例服务（History、Tag、Automation、Backup、Recovery、Config、Dashboard、Export、StorageUsage、TaskLedger 和 LLM tasks）位于 `sona-application`（`application/`）。每个服务只持有自身需要的 `Arc<dyn Port>` 依赖，并委托给 Core 所有的端口 trait。领域类型、端口 trait 定义和错误类型保留在 `sona-core`。

另一个独立的 Application 角色包是 `sona-sync`（路径 `adapters/sync/`），因为 Sync 需要与具体网络/数据库适配器解耦的、提供商中立的 vault 与生命周期。除非存在明确的隔离边界，否则不要为每个领域再造 Application crate；新的用例服务应集中到 `sona-application`。

实时转录由 `sona-application` 的 `LiveTranscriptionCoordinator` 统一协调。Core 只拥有 ASR 契约、带类型的音频帧游标、推理规范等值比较，以及每个消费者独立的输出策略；不拥有采集设备或 UI 生命周期。`StreamingAsrFactoryPort` 由 Desktop 组合根实现，负责选择 local/online adapter 并复用模型或连接资源。Desktop 仍拥有 CPAL 采集 lease、录音 writer、Tauri 命令和事件投递。CLI 与 UniFFI/Android 使用同一个 Core 音频帧端口，但保留独立 session，未来接入协调器时不需要再造一套 streaming API。对于相同 source epoch、输入变换和推理规范，每个 source frame 只向唯一 pipeline feed 一次；消费者仍有独立邮箱和后处理。

### Core 模块导航

- `core/src/domain/` 存放跨 LLM/自动化使用的**产品身份枚举**（例如 `LlmProvider`、润色预设、摘要模板）。它不是全部领域逻辑的入口；History、 Tag、Transcription 等各自在 `core/src/` 下的独立模块中。
- `core/src/history/` 拥有历史记录、校验与编辑规则，以及 `HistoryStore` trait（`history/store.rs`，对外仍通过 `sona_core::history_store` 重导出）；查询/变更用例服务位于 `application/src/history/`。

<a id="port-placement"></a>
## 端口放置规则

端口全部由 Core 拥有，但不集中在一个目录。存在两种放置方式，这个区分是已评审的契约，不是风格偏好。

| 类别 | 位置 | 判定 |
| --- | --- | --- |
| 能力端口 | `core/src/ports/<capability>.rs` | 领域无关；可能被多个领域使用 |
| 领域自有端口 | `core/src/<domain>/` | 只有该领域的用例会调用 |

**能力端口**以基础设施能力命名，绝不以领域聚合命名：`FileSystem`、 `PathProvider`、`UnixMillisClock`、`EventEmitter`、`BatchTranscriber`、 `LlmCompletionPort`、`GpuAvailabilityProvider`。只有当"把它移进某一个领域"是错的时候，这个 trait 才属于这里。

**领域自有端口**是归属单一领域的 store、repository 与协作者：`HistoryStore`、 `TagStore`、`AutomationStore`、`BackupStateRepository`、`RecoverySnapshotStore`、 `SyncObjectStore`。它们放在各自领域模块下，使领域保持自描述。

由此得出两条规则，均由 `scripts/core-port-placement.test.js` 强制执行：

1. `core/src/ports/` 中不得声明任何 `*Store` 或 `*Repository` trait。持久化形态的 trait 按定义就归属某个领域。
2. `core/src/ports/` 中的 trait 不得以领域聚合命名（History、Tag、Automation、 Backup、Recovery、TaskLedger、Dashboard、StorageUsage、AppConfig）。

新增领域端口应使用 `core/src/<domain>/ports.rs`——`backup`、`dashboard`、 `export`、`storage_usage`、`sync` 已采用该形式。`automation`、`config`、 `history`、`recovery`、`tag`、`task_ledger` 中现有的 `repository.rs` / `store.rs` 放置属于历史沿用：它们满足上述两条规则，统一文件命名是独立的迁移切片，不是顺手可做的清理。

<a id="host-capability-matrix"></a>
## Host 能力矩阵

下表依据当前工作区依赖与产品范围整理。勾选表示该 Host 今天已接线。“out of scope” 表示有意的产品边界，而不是偶然漏依赖。

| 能力 | Desktop (`sona`) | CLI (`sona-cli`) | UniFFI (`sona-uniffi-bind`) |
| --- | --- | --- | --- |
| SQLite / History / Tag | yes | yes | yes |
| Local ASR | yes | yes | yes |
| Online ASR | yes | no | yes |
| Online LLM | yes | yes | yes |
| Model downloads | yes | yes | yes |
| Media detector | yes | yes | no |
| API server | yes | yes | no |
| Sync（application + WebDAV） | yes | out of scope | yes |
| TypeScript/Tauri 契约绑定 | yes | no | no |
| Archive / export / recovery / runtime-fs | yes | yes | yes |

CLI Sync 的产品范围尚未定义，在明确范围之前不得接入。UniFFI 缺少 media detector 属于当前 Host 接线限制，不是 Core 端口缺失。

<a id="error-boundaries"></a>
## 错误边界

领域错误和应用错误在 Core、Application 与适配器代码中保持类型化。Tauri、UniFFI、CLI 和 HTTP 路由处理函数在最后为调用方执行兼容性映射。它们是唯一公开的字符串转换边界：内部层不得把类型化失败替换为公开字符串。

<a id="compatibility-policy"></a>
## 兼容性策略

公开调用方契约可能需要字符串、状态码与字符串元组或特定语言的错误值。将这类转换保留在最终的 Tauri、UniFFI、CLI 或 HTTP 边界，才能保持既有契约。新的领域和应用 API 必须公开类型化错误；适配器和 Host 不得把兼容性字符串移入内部。外部契约需要变更时，应增加明确的边界映射与聚焦的契约测试，而不是削弱内部类型化 API。

<a id="reviewed-exceptions"></a>
## 已评审例外

当前没有已登记的 outbound-adapter 互依例外。每个 outbound adapter 只应依赖 Core（以及已评审的 Application，如 `sona-sync-webdav -> sona-sync`）。

<a id="compatibility-debt"></a>
## 兼容债务清单

下列项在当前兼容窗口内是显式允许的。它们不是“顺手清理”对象；没有专门迁移切片与契约测试时，不要重命名或删除。

### Project 到 Tag

- **规范写模型：** Tag（`TagStore` / SQLite tag 表）。
- **已删除空模块：** `core/src/project/` 已移除；不要再创建空的 Project core 模块。
- **仍使用 Project 命名的 Host 兼容叶节点：**
  - Desktop Tauri：`platforms/desktop/src/commands/history.rs` 中的 `history_update_project_assignments`、`history_reassign_project`（委托到 tag assignment）。
  - UniFFI JSON：`adapters/uniffi_bind/src/` 下仍带 Project 命名的 history/config 辅助接口（例如 project assignment 与 effective-config 的 project JSON 参数）。
  - Desktop 前端仍使用 Project 产品路径： `platforms/desktop/frontend/src/types/project.ts`、 `services/projectService.ts`、`stores/projectStore.ts`、 `components/projects/*`、以及 `components/ProjectsView.tsx`。
- **策略：** 兼容窗口内保留公开 Project 名称；前端/API 物理重命名属于后续切片。

### UniFFI 类型化契约清单

每个 `*_json` UniFFI 导出都在 `scripts/multisurface-contracts.test.js` 的 `UNIFFI_JSON_ONLY_EXPORTS` 中分类。新增没有 `*_v1` 兄弟函数的 `*_json` 导出会让该测试失败，直到它被分类为止。

- **`dynamic-leaf`**（动态叶子）——永久允许。载荷是任意用户配置、提供商扩展文档，或遗留 Project 兼容面，其 schema 不由本绑定拥有。
- **`pending-migration`**（待迁移）——已评审债务。该导出把完整的快照、记录、请求或结果作为 JSON 字符串传输，必须补上类型化的 `_v1` 兄弟函数。目前仅剩 `complete_llm_json`：`LlmCompletionOptions` 会牵出 response-format、 prompt-cache 与 capability policy 三棵类型树，属于独立切片。

已类型化的域保留两套接口：`_json` 作为兼容委托保留，`_v1` 承载类型化契约。所有域均已迁移：Tag、History、Task Ledger、Recovery、Automation、Storage Usage、Export、Backup、Dashboard、Diagnostics、Sync、LLM。

部分 `*_json` 导出是设计使然而非债务。除了任意配置与遗留 Project 接口外， **解析入口**（`llm_config_from_json`、`polish_segments_request_from_json` 及另两个兄弟函数）的存在意义正是把应用存储的 JSON 转成类型化记录——类型化形式是它们的**输出**，因此把入参改成类型化会让它们变成恒等函数。

### 凭据绝不以可打印字段跨界

UniFFI 把 `Record` 生成为 Kotlin `data class`，其自动派生的 `toString()` 会打印每一个字段。因此以普通 `String` 承载的凭据会泄漏进任何格式化该记录的日志。凭据改为以对象句柄跨界——`FfiSecret`、`FfiOnlineAsrApiKey`——它们的 Kotlin `toString()` 是身份标识，Rust 侧 `Debug` 则把值替换为 `<redacted>`。读取值的 `expose()` 刻意不加 `#[uniffi::export]`，所以密钥不会变成生成句柄上的可读属性。

`scripts/multisurface-contracts.test.js` 强制这一约束：`uniffi::Record` 中任何名字看起来像凭据的字段都必须是不透明句柄。

### 其他已评审债务

- 已消除的 outbound 互依：`model-downloads` 与 `recovery-fs` 不再依赖 `runtime-fs`；完整性规则在 Core，I/O 由各适配器本地完成。
- Sync 遗留 secret-store 注册与原始 WebDAV 线格式，作为面向旧调用方的 Host 委托。
- `sona-sqlite` 在生产依赖中使用 `tempfile`，因为 `Database` 拥有只读快照 `TempDir` 生命周期；这不是误放的 dev-only 依赖。
- CLI Sync 在产品范围定义前保持 out of scope。

<a id="verification"></a>
## 验证

修改这些指南或角色注册表后，运行稳定指南契约：

```text
rtk node --test --test-name-pattern "stable architecture guides" scripts/crate-boundaries.test.js
```

修改包角色登记表或其依赖规则后，运行完整的 crate 边界脚本测试：

```text
rtk node --test scripts/crate-boundaries.test.js
```

修改 Host 依赖或上方能力矩阵行后，运行 Host 能力矩阵契约：

```text
rtk node --test scripts/host-capability-matrix.test.js
```

修改 Host 组合根或矩阵能力的生产接线后，运行 Host 接线清单契约：

```text
rtk node --test scripts/host-wiring-inventory.test.js
```

在 Core 中新增或移动端口 trait 后，运行端口放置契约：

```text
rtk node --test scripts/core-port-placement.test.js
```
