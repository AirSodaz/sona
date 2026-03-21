# Sona

[English](README.md) | [简体中文](README.zh-CN.md)

**Sona** 是一款功能强大的离线转录（字幕）编辑器，由 [Tauri](https://tauri.app)、[React](https://react.dev) 和 [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 构建。它使用高性能的 Rust 后端，直接在您的本地机器上提供快速、准确且私密的语音转文本能力。

## ✨ 特性

- **🔒 离线与隐私**：所有语音处理都在您的设备本地进行。没有任何数据会离开您的机器。
- **🎙️ 实时转录**：以低延迟实时录制和转录音频。
- **📁 批量处理**：导入多个音频或视频文件，在后台进行批量转录。
- **📝 交互式编辑器**：与音频播放同步的富文本编辑器，方便进行校对。
- **✨ AI 助手**：使用 OpenAI、Anthropic、Gemini 或 Ollama 对转录文本进行润色和翻译。
- **📤 智能导出**：支持多种格式（TXT、SRT、VTT、JSON）和双语字幕的导出。
- **🤖 强大的语音识别模型**：由 **SenseVoice**、**Whisper** 和 **Paraformer** 等最先进的模型驱动。

## 🚀 快速开始

### 从 GitHub Releases 下载

安装 Sona 最简单的方法是从 [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) 页面下载适合您平台的预编译二进制文件。

### 从源码构建

#### 前置条件

*   **Node.js**: v20 或更高版本 (用于构建前端)。
*   **Rust**: 稳定版 (用于 Tauri 后端)。
*   **包管理器**: `pnpm` (必须)。

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
    pnpm install
    ```
    *（注意：这会自动运行 `setup-ffmpeg.js` 预构建脚本，为 Tauri 后端下载 ffmpeg-static。）*

3.  **运行应用**
    ```bash
    pnpm run tauri dev
    ```

## 📦 模型管理

Sona 允许您选择最适合您需求的 AI 模型，无论是离线转录还是在线助手。

### 离线转录
1.  进入 **Settings > Models**（设置 > 模型）。
2.  从精心挑选的高性能模型列表中进行选择：
    *   **SenseVoice**：多语言支持和情感识别的最佳选择。
    *   **Whisper (Tiny)**：OpenAI Whisper 模型的轻量级版本。
    *   **Paraformer**：专为流式识别优化。
3.  点击 **Download**（下载）。模型将自动保存在本地。

### AI 助手（润色与翻译）
1.  进入 **Settings > AI Service**（设置 > AI 服务）。
2.  选择您的服务提供商（OpenAI、Anthropic、Gemini 或 Ollama）。
3.  输入您的 API 密钥和 Base URL（如果适用）。
4.  选择一个模型来为润色和翻译功能提供支持。

## 🛠️ 开发命令

为了帮助开发，这里有一些您可能需要的常用命令：

*   **运行前端测试**: `pnpm test`
*   **构建前端**: `pnpm build`
*   **检查 Rust 后端**: `cargo check --manifest-path src-tauri/Cargo.toml`
*   **测试 Rust 后端**: `cargo test --manifest-path src-tauri/Cargo.toml`

## 🏗️ 构建

要构建生产环境用的应用程序：

```bash
pnpm run tauri build
```

可执行文件将生成在 `src-tauri/target/release/bundle` 目录中。
