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

<a id="composition-roots"></a>
## 组合根

- 桌面端：`platforms/desktop/src/app/setup.rs` 与 `platforms/desktop/src/platform/` 组合桌面运行时。
- CLI：`platforms/cli/src/lib.rs` 与各个命令模块组合 CLI 命令。
- UniFFI/移动端：`adapters/uniffi_bind/src/application_context.rs` 与 `adapters/uniffi_bind/src/facade.rs` 组合面向移动端的接口。目录名 `adapters/uniffi_bind` 是历史遗留名称：`sona-uniffi-bind` 的角色是 host，而不是适配器角色。

<a id="error-boundaries"></a>
## 错误边界

领域错误和应用错误在 Core、Application 与适配器代码中保持类型化。Tauri、UniFFI、CLI 和 HTTP 路由处理函数在最后为调用方执行兼容性映射。它们是唯一公开的字符串转换边界：内部层不得把类型化失败替换为公开字符串。

<a id="compatibility-policy"></a>
## 兼容性策略

公开调用方契约可能需要字符串、状态码与字符串元组或特定语言的错误值。将这类转换保留在最终的 Tauri、UniFFI、CLI 或 HTTP 边界，才能保持既有契约。新的领域和应用 API 必须公开类型化错误；适配器和 Host 不得把兼容性字符串移入内部。外部契约需要变更时，应增加明确的边界映射与聚焦的契约测试，而不是削弱内部类型化 API。

<a id="reviewed-exceptions"></a>
## 已评审例外

以下 outbound-adapter 依赖是当前已评审的例外。它们在策略测试中被显式命名，且在实现变化时必须重新评估。

| 依赖边 | 原因 |
| --- | --- |
| `sona-model-downloads->sona-runtime-fs` | model installation completeness currently reuses the runtime filesystem probe |
| `sona-recovery-fs->sona-runtime-fs` | the recovery adapter currently composes the shared real filesystem, path-status, clock, and atomic JSON helpers |

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
