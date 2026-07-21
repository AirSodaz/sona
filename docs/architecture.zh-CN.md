# Sona 架构

[English](architecture.md) | [简体中文](architecture.zh-CN.md) | [项目 README](../README.zh-CN.md) | [参与贡献](../CONTRIBUTING.md)

<a id="architecture-roles"></a>
## 架构角色

Sona 使用六种稳定角色。角色是经过评审的依赖契约，而不是根据目录名推断出的描述。

| 包 | 角色 |
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

目录名只用于组织代码。经过评审的角色写在各包的
`[package.metadata.sona] role` 字段以及上文角色表中。不要仅凭路径推断角色。

| 路径 | 包 | 角色 | 说明 |
| --- | --- | --- | --- |
| `core/` | `sona-core` | core | 领域契约与由 Core 所有的端口 |
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
- UniFFI/移动端：`adapters/uniffi_bind/src/application_context.rs` 与 `adapters/uniffi_bind/src/facade.rs` 组合面向移动端的接口。目录名 `adapters/uniffi_bind` 是历史遗留名称：`sona-uniffi-bind` 的角色是 host，而不是适配器角色。

各 Host 共享 `sona-sqlite` 提供的 `SqliteApplicationContext`，但接线、生命周期与错误映射仍由各 Host 自行拥有。目前还没有单独的共享 application-composition crate。

<a id="host-capability-matrix"></a>
## Host 能力矩阵

下表依据当前工作区依赖与产品范围整理。勾选表示该 Host 今天已接线。
“out of scope” 表示有意的产品边界，而不是偶然漏依赖。

| 能力 | Desktop (`sona`) | CLI (`sona-cli`) | UniFFI (`sona-uniffi-bind`) |
| --- | --- | --- | --- |
| SQLite / History / Tag | yes | yes | yes |
| Local ASR | yes | yes | yes |
| Online ASR | yes | no | yes |
| Online LLM | yes | yes | yes |
| Model downloads | yes | yes | no |
| Media detector | yes | yes | no |
| API server | yes | yes | no |
| Sync（application + WebDAV） | yes | out of scope | yes |
| TypeScript/Tauri 契约绑定 | yes | no | no |
| Archive / export / recovery / runtime-fs | yes | yes | yes |

CLI Sync 的产品范围尚未定义，在明确范围之前不得接入。UniFFI 缺少 model downloads 与 media detector 属于当前 Host 接线限制，不是 Core 端口缺失。

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
  - Desktop Tauri：`platforms/desktop/src/commands/history.rs` 中的
    `history_update_project_assignments`、`history_reassign_project`（委托到 tag assignment）。
  - UniFFI JSON：`adapters/uniffi_bind/src/` 下仍带 Project 命名的 history/config 辅助接口
    （例如 project assignment 与 effective-config 的 project JSON 参数）。
  - Desktop 前端仍使用 Project 产品路径：
    `platforms/desktop/frontend/src/types/project.ts`、
    `services/projectService.ts`、`stores/projectStore.ts`、
    `components/projects/*`、以及 `components/ProjectsView.tsx`。
- **策略：** 兼容窗口内保留公开 Project 名称；前端/API 物理重命名属于后续切片。

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
