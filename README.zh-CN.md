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

### CLI

Sona 现在通过独立的 `sona-cli` 二进制提供命令行工作流。桌面 Tauri 应用不再解析 CLI 子命令；release 和 nightly 构建会把同平台的 `sona-cli` 放进桌面安装包资源中，因此它可以复用随包携带的 Sherpa-onnx 动态链接库。

安装包说明：

- Windows/macOS/Linux 安装包会包含与平台匹配的 `sona-cli` 资源。
- `sona-cli` 与桌面可执行文件相互独立，也可以单独构建为发布二进制。

如果您是从源码构建，可以直接在 workspace 中运行或构建 CLI：

```bash
cargo run -p sona-cli -- transcribe ./sample.mp4 -c ./sona-cli.toml --output ./sample.srt
cargo run -p sona-cli -- transcribe-live --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17
cargo run -p sona-cli -- serve --host 127.0.0.1 --port 14200
pnpm run build:sona-cli
```

当前独立 CLI 范围：

- 单文件离线转写
- 使用本地流式模型实时转写麦克风或原始 stdin 音频
- 预置模型列表查看、模型下载与模型删除
- 从已有 segment JSON 导出转录
- 共享的历史列表、工作区查询、转录/快照查看与写操作
- 备份归档导出、检查与显式确认后的恢复
- 通过 `sona-cli serve` 启动共享本地 HTTP API 服务
- 运行时路径状态检查
- 带注释的 `sona-cli.toml` 初始模板生成

完整 CLI 说明和 `sona-cli init-config` TOML 模板工作流请查看 [docs/cli.zh-CN.md](docs/cli.zh-CN.md)。

### 从源码构建

#### 前置条件

*   **Node.js**: v20 或更高版本 (用于构建前端)。
*   **Rust**: 稳定版 (用于 Tauri 后端)。
*   **包管理器**: 通过 Corepack 使用 `pnpm` (推荐)。

##### Linux 依赖
如果您使用的是 Linux (Ubuntu/Debian)，请确保您已安装必要的系统依赖：

```bash
sudo apt-get update
sudo apt-get install libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libasound2-dev
```

#### 安装步骤

1.  **克隆仓库**
    ```bash
    git clone https://github.com/AirSodaz/sona.git
    cd sona
    ```

2.  **安装依赖**
    ```bash
    corepack enable
    pnpm install
    ```

3.  **运行应用**
    ```bash
    pnpm run tauri dev
    ```

4.  **运行前端测试**
    ```bash
    pnpm test
    ```

## 📦 模型管理

Sona 允许您选择最适合您需求的 AI 模型，无论是离线转录还是在线助手。

### 离线转录
1.  进入 **Settings > Model Settings**（设置 > 模型设置）。
2.  从精心挑选的高性能模型列表中进行选择：
    *   **SenseVoice**：多语言支持和情感识别的最佳选择。
    *   **Whisper (Tiny)**：OpenAI Whisper 模型的轻量级版本。
    *   **Paraformer**：专为流式识别优化。
3.  点击 **Download**（下载）。模型将自动保存在本地。

### LLM 助手（润色、翻译与摘要）
1.  进入 **Settings > LLM Service**（设置 > LLM 服务）。
2.  选择您的服务提供商（OpenAI、Anthropic、Gemini 或 Ollama）。
3.  输入您的 API 密钥和 Base URL（如果适用）。
4.  选择为润色、翻译和摘要生成提供支持的模型。

## 🏗️ 构建

要构建生产环境用的应用程序：

```bash
pnpm run tauri build
```

桌面安装包会根据构建目标生成在 `target/release/bundle` 或 `target/<triple>/release/bundle` 目录中。
