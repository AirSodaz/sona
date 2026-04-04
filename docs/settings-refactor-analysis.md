# Settings 模块分析报告

## 一、模块全貌

Sona 的设置模块以模态对话框形式呈现，包含 **8 个 Tab**，共涉及约 **20 个文件、~2,800 行代码**（不含测试和 CSS）。

### 文件清单

| 文件 | 行数 | 职责 |
|------|------|------|
| `Settings.tsx` | 287 | 模态容器 + 侧栏路由 |
| `useSettingsLogic.ts` | 373 | 全局设置业务逻辑 Hook |
| **Tab 组件** | | |
| `SettingsGeneralTab.tsx` | 168 | 语言、主题、字体、托盘行为 |
| `SettingsMicrophoneTab.tsx` | 354 | 音频输入设备、增益、实时可视化 |
| `SettingsSubtitleTab.tsx` | 152 | 字幕窗口外观与行为 |
| `SettingsModelsTab.tsx` | 317 | 模型中心（ASR/VAD/标点） |
| `SettingsLLMServiceTab.tsx` | 554 | LLM 服务商配置（最复杂） |
| `SettingsLocalTab.tsx` | 156 | VAD 缓冲、ITN 规则排序 |
| `SettingsShortcutsTab.tsx` | 101 | 快捷键展示（只读） |
| `SettingsAboutTab.tsx` | 172 | 版本信息、检查更新 |
| **辅助组件** | | |
| `SettingsLayout.tsx` | 144 | 布局原语（Section / Item / Accordion） |
| `SettingsTabButton.tsx` | 36 | 侧栏按钮 |
| `ModelCard.tsx` | 191 | 模型卡片（下载/删除/进度条） |
| `ItnModelList.tsx` | 203 | ITN 规则拖拽排序列表 |
| `ParameterSettingsModal.tsx` | 201 | 快捷参数设置弹窗（批量/全局） |
| **样式** | | |
| `SettingsShared.css` | — | 各 Tab 共享样式 |
| `SettingsLLMServiceTab.css` | — | LLM Tab 专用样式 |
| `styles/index.css` (settings 部分) | ~180 | 模态框/侧栏/内容区基础样式 |

### 测试覆盖

| 测试文件 | 覆盖范围 |
|----------|----------|
| `Settings.test.tsx` | 模态集成测试（模型下载/删除） |
| `SettingsFocus.test.tsx` | 焦点陷阱、键盘导航 |
| `SettingsLLMServiceTab.test.tsx` | Provider 切换、模型候选、温度验证 |
| `SettingsSubtitleTab.test.tsx` | 字幕配置 |
| `useSettingsLogic.test.ts` | Hook 逻辑、LLM provider 状态迁移 |

---

## 二、当前架构

```
Settings.tsx (Modal Shell)
  ├── useSettingsLogic.ts (业务 Hook)
  │     ├── transcriptStore.config (Zustand)
  │     ├── modelService (下载/安装)
  │     └── llmConfig (LLM 配置)
  ├── SettingsTabButton × 8 (侧栏)
  └── Tab Content (switch 路由)
        ├── SettingsGeneralTab
        ├── SettingsMicrophoneTab
        ├── SettingsSubtitleTab
        ├── SettingsModelsTab
        │     └── ModelCard
        ├── SettingsLocalTab
        │     └── ItnModelList
        ├── SettingsLLMServiceTab
        ├── SettingsShortcutsTab
        └── SettingsAboutTab
```

### 数据流

1. **读取**：`transcriptStore.config`（Zustand） → `useSettingsLogic` → props 透传到各 Tab
2. **写入**：Tab 调用 `updateConfig(patch)` → `setConfig` 合并到 store → `useAppInitialization` 监听变化 → 500ms 防抖写入 `localStorage('sona-config')`
3. **LLM 配置**：额外走 `llmConfig.ts` 中的纯函数处理 provider 切换和迁移

---

## 三、发现的问题与改进方向

### 1. 🔴 AppConfig 单体对象过于臃肿

`AppConfig` 接口定义了 **~68 行字段**，将完全不相关的关注点混在一起：
- 硬件设置（麦克风 ID、增益）
- UI 偏好（主题、字体、语言）
- 模型路径（streaming/offline/VAD/punctuation）
- LLM 服务配置（嵌套的 `llmSettings`）
- 转录参数（VAD buffer、ITN 规则）
- 字幕窗口配置

**影响**：任何一个 Tab 修改任何字段，都会触发所有订阅 `config` 的组件 re-render。

### 2. 🔴 useSettingsLogic 职责过重

这个 373 行的 Hook 承担了太多不相关的职责：
- Tab 状态管理
- 模型下载/安装/删除的完整生命周期
- LLM provider 切换
- 背景下载事件监听（CustomEvent）
- 配置恢复默认值

大部分逻辑只与 Models Tab 和 LLM Tab 相关，却被提升到了顶层。

### 3. 🟡 Props 透传链过长

`Settings.tsx` 从 `useSettingsLogic` 解构了大量值，再逐一传给各 Tab。例如 `SettingsLocalTab` 接收了 7 个 props，其中 `downloads`、`onDownloadITN`、`onCancelDownload`、`installedModels` 都是从顶层穿透的。

### 4. 🟡 配置持久化与副作用耦合

`useAppInitialization` 同时负责：
- 从 localStorage 读取配置
- 应用主题/字体/语言等副作用
- 防抖写入 localStorage
- 同步托盘行为到 Rust 后端

应该拆分成独立的关注点。

### 5. 🟡 LLM 配置复杂度高

`llmConfig.ts` 有 **791 行**，包含大量的"规范化"和"迁移"逻辑（`ensureLlmState`），说明配置 schema 经历了多次变更但缺乏正式的版本管理机制。

### 6. 🟢 Tab 间无懒加载

所有 Tab 组件在 `Settings.tsx` 中被 import 但使用 switch 按需渲染。对于 MicrophoneTab（涉及 IPC 和 canvas）和 ModelsTab（涉及文件系统检查），可以考虑 `React.lazy` 减少初始加载。

### 7. 🟢 样式分散

设置相关的 CSS 分布在三处：`index.css`（~180 行）、`SettingsShared.css`、`SettingsLLMServiceTab.css`，命名约定不完全一致。

---

## 四、可能的重构方向

| 方向 | 描述 | 优先级 |
|------|------|--------|
| **拆分 AppConfig** | 按关注点分成 `UIConfig`、`ModelConfig`、`LlmConfig`、`AudioConfig` 等，各自有独立的 Zustand slice 或 selector | 高 |
| **拆分 useSettingsLogic** | 将模型管理逻辑抽到 `useModelManager`，LLM 逻辑抽到 `useLlmConfig`，Tab 状态保留在 `Settings.tsx` 本地 | 高 |
| **减少 Props 透传** | 让 Tab 自行订阅 store slice，或使用 Context 提供设置操作 | 中 |
| **配置 schema 版本化** | 为 `sona-config` 添加 `version` 字段和正式的迁移管道，替代 `ensureLlmState` 中的临时迁移 | 中 |
| **副作用解耦** | 将主题/字体/语言的应用逻辑拆成独立的 `useThemeEffect`、`useFontEffect` 等 | 中 |
| **懒加载重型 Tab** | 对 `SettingsMicrophoneTab`、`SettingsModelsTab` 使用 `React.lazy` + `Suspense` | 低 |
| **样式统一** | 将设置相关样式合并到 `settings/` 目录下，统一命名规范 | 低 |
