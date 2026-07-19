# Sona

[English](README.md) | [简体中文](README.zh-CN.md)

**Sona** 是一款功能强大的离线转录（字幕）编辑器，由 [Tauri](https://tauri.app)、[React](https://react.dev) 和 [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 构建。它使用高性能的 Rust 后端，直接在您的本地机器上提供快速、准确且私密的语音转文本能力。

## ✨ 特性

- **🔒 离线与隐私**：所有语音处理都在您的设备本地进行。没有任何数据会离开您的机器。
- **🎙️ 实时转录**：以低延迟实时录制和转录音频。
- **📁 批量处理**：导入多个音频或视频文件，在后台进行批量转录。
- **🗂️ 工作区组织**：通过 `工作区`、`项目` 与 `Inbox` 整理已保存的录音和导入内容。
- **📝 交互式编辑器**：与音频播放同步的富文本编辑器，支持文本校对、说话人标签和版本快照。
- **👥 说话人档案与校对**：建立本地说话人档案，逐段修正说话人标签，并在导出前集中检查候选或匿名说话人分组。
- **✨ LLM 助手**：使用 OpenAI、Anthropic、Gemini 或 Ollama 对转录文本进行润色、翻译和摘要。
- **🗣️ 实时字幕与语音输入**：复用同一套离线实时转录能力，既可显示悬浮字幕，也可向其他应用直接输入文字。
- **📤 智能导出**：支持多种格式（TXT、SRT、VTT、JSON）和双语字幕的导出。
- **🛟 恢复、备份与诊断**：恢复中断任务、导出包含配置、工作区和文本历史的轻量备份，并在应用内检查模型与运行时健康状态。
- **🔔 通知与自动化**：通过顶部通知中心查看更新、恢复和自动化结果，并在设置中配置文件夹自动化规则。
- **🤖 强大的语音识别模型**：由 **SenseVoice**、**Whisper** 和 **Paraformer** 等最先进的模型驱动。

## 🚀 快速开始

### 从 GitHub Releases 下载

安装 Sona 最简单的方法是从 [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) 页面下载适合您平台的预编译二进制文件。

### 用户指南

如果您想查看面向终端用户的完整使用说明，请阅读[用户指南](docs/user-guide.zh-CN.md)。其中包含首次设置、`Live Record`、`Batch Import`、`工作区` / `项目` / `Inbox`、转录编辑、说话人校对、版本快照、LLM 功能、`语音输入法`、导出、`仪表盘` / 备份 / 恢复入口，以及常见问题。

## 📚 文档中心

根据您要完成的工作选择对应文档：

| 文档 | 内容 | 语言 |
| --- | --- | --- |
| 用户指南 | 安装、首次设置、转录、编辑和导出 | [简体中文](docs/user-guide.zh-CN.md) · [English](docs/user-guide.md) |
| CLI 指南 | 独立命令、自动化、实时转录和配置 | [简体中文](docs/cli.zh-CN.md) · [English](docs/cli.md) |
| HTTP API 参考 | 服务配置、认证、接口和 Webhook | [简体中文](docs/api.zh-CN.md) · [English](docs/api.md) |
| 开发指南 | 本地环境、测试、桌面构建和 CLI 构建 | [简体中文](docs/development.zh-CN.md) · [English](docs/development.md) |
| 架构指南 | 包角色、依赖方向、兼容性边界和验证 | [简体中文](docs/architecture.zh-CN.md) · [English](docs/architecture.md) |
| Nightly 工作流 | Nightly 触发条件、构建任务、产物和发布 | [简体中文](docs/nightly-workflow.zh-CN.md) · [English](docs/nightly-workflow.md) |
| 参与贡献 | 分支、验证、提交和 Pull Request 要求 | [English](CONTRIBUTING.md) |
