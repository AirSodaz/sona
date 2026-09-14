# Sona

[English](README.md) | [简体中文](README.zh-CN.md)

**Sona** 是一款功能强大的离线转录（字幕）编辑器，由 [Tauri](https://tauri.app)、[React](https://react.dev)、[Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 与 [llama.cpp](https://github.com/ggerganov/llama.cpp) 构建。它使用高性能的 Rust 后端，直接在您的本地机器上提供快速、准确且私密的语音转文本能力。

## ✨ 特性

- **🔒 离线与隐私**：语音处理完全在您的设备本地进行。原始录音音频永远不会上传到云端。
- **🎙️ 实时转录**：以低延迟实时录制和转录音频。
- **📁 批量处理**：导入多个音频或视频文件进行后台批量转录，支持任务中心实时取消。
- **☁️ 端到端加密云同步**：基于 WebDAV 的多设备增量同步，客户端端到端加密（E2EE），支持主密码、应急恢复密钥、同步范围（内容/标准/完整模式）与冲突解决中心。
- **📱 跨平台与 Android 支持**：核心架构通过 UniFFI 跨端复用，覆盖桌面端（Windows, macOS, Linux）与 Android 原生客户端。
- **🗂️ 项目中心与自动化流水线**：通过 `项目中心`、`收件箱 (Inbox)` 与 `回收站` 管理内容；支持为项目配置专属流水线（Pipeline），自动绑定热词集、替换规则集、润色场景与翻译语言。
- **📝 交互式富文本编辑器**：与音频播放精准同步的编辑器，支持分段拆分/合并、说话人标签、版本快照、丰富格式（加粗、斜体、下划线、删除线 `Ctrl+Shift+S`、行内代码）与出字预测光标。
- **👥 说话人档案与校对**：建立本地说话人声纹档案，逐段修正说话人标签，并在导出前集中校对候选或匿名说话人分组。
- **✨ LLM 助手**：接入 OpenAI、Anthropic、Gemini、DeepSeek 或 Ollama 对转录文本进行润色、翻译和摘要。
- **🗣️ 实时字幕与语音输入法**：复用同一套离线实时转录能力，既可显示悬浮字幕，也可向其他桌面应用全局听写输入。
- **📤 智能导出**：支持多种格式（TXT、SRT、VTT、JSON、Markdown）和双语字幕导出，支持一键复制到剪贴板。
- **🛟 任务恢复、备份与存储管理**：从任务中心恢复或丢弃中断任务，导出轻量备份，配置存储目录与自定义 FFmpeg 路径，设定音频定期保留清理策略。
- **🤖 先进的语音识别模型**：全面支持 **Qwen3-ASR**（基于 llama.cpp）、**FireRedASR2-AED**、**SenseVoice**、**Whisper**、**Paraformer** 以及默认的 **Silero VAD v5**。

## 🚀 快速开始

### 从 GitHub Releases 下载

安装 Sona 最简单的方法是从 [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) 页面下载适合您平台的预编译二进制文件。

### 用户指南

如果您想查看面向终端用户的完整使用说明，请阅读[用户指南](docs/user-guide.zh-CN.md)。其中包含首次设置、`Live Record`、`Batch Import`、`项目中心` / `Inbox`、转录编辑、说话人校对、版本快照、LLM 功能、`语音输入法`、导出、`仪表盘` / 备份 / 恢复入口，以及常见问题。

## 📚 文档中心

根据您要完成的工作选择对应文档：

| 文档 | 内容 | 语言 |
| --- | --- | --- |
| 用户指南 | 安装、首次设置、转录、编辑和导出 | [简体中文](docs/user-guide.zh-CN.md) · [English](docs/user-guide.md) |
| CLI 指南 | 无状态本地/在线转写、模型管理和配置 | [简体中文](docs/cli.zh-CN.md) · [English](docs/cli.md) |
| HTTP API 参考 | 服务配置、认证、接口和 Webhook | [简体中文](docs/api.zh-CN.md) · [English](docs/api.md) |
| 开发指南 | 本地环境、测试、桌面构建和 CLI 构建 | [简体中文](docs/development.zh-CN.md) · [English](docs/development.md) |
| 架构指南 | 包角色、依赖方向、兼容性边界和验证 | [简体中文](docs/architecture.zh-CN.md) · [English](docs/architecture.md) |
| Nightly 工作流 | Nightly 触发条件、构建任务、产物和发布 | [简体中文](docs/nightly-workflow.zh-CN.md) · [English](docs/nightly-workflow.md) |
| 参与贡献 | 分支、验证、提交和 Pull Request 要求 | [English](CONTRIBUTING.md) |

## 📄 开源协议

本项目采用 [MIT 许可证](LICENSE) 开源。
