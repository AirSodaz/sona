# 🚀 重构计划：将 Node.js Sidecar 迁移至 Tauri 原生 (Rust) 后端

本项目旨在将 `src-tauri/sidecar` 独立 Node.js 进程的语音识别逻辑（主要依赖 `sherpa-onnx` 和 `ffmpeg` 等）整合进入主后端（即 `src-tauri` 作为 Tauri Managed Plugin 运行）。此方案彻底消除 Node.js 运行时及外部 Sidecar 的依赖，极大降低 IPC 序列化性能损耗，并缩减安装包体积。

## 阶段一：前期调研与依赖集成
目标：测试并筛选好符合现有 Node.js 生态功能的对应 Rust Crate。

- **Sherpa-onnx 绑定**：研究 `sherpa-onnx` 的官方 C API 绑定方式。如有成熟的社区生态（如 `sherpa-rs`）则直接引入，否则使用 `bindgen` 在 `build.rs` 内自动生成 FFI 函数。
- **文件与压缩包处理**：引入 `sevenz-rust` 等压缩包处理相关的包，用来替代 `7zip-bin`/`node-7z` 下载模型和静态解压功能。
- **音频处理核心**：引入 `hound`（操作 WAV 文件头）及 `rubato`（音频重采样），视情况继续利用命令行调用二进制 `ffmpeg` 或直接由 Rust 内部解码器（如 `symphonia`）替代。

## 阶段二：音频流和重采样核心实现
目标：纯 Rust 复刻语音提取、重采样与 VAD 流式切分功能。

- 实现并封装与现有 JS 中的 `pcmInt16ToFloat32` 对应的高效 Rust `Vec<i16> -> Vec<f32>` 数据管线。
- 采用 Rust 纯代码实现或整合原生 VAD 的方式提取切分 PCM 语音切片（对应 JS 脚本的 `yieldVADSegments` 和 `yieldFixedChunks`）。

## 阶段三：Sherpa-Onnx 及组件封装
目标：加载模型，启动引擎，实现流式识别和标点追加等逻辑功能。

- 构建基于 Tauri Managed State 的单例识别引擎管理器。
- 实现 `findModelConfig`：扫描给定的模型路径目录，探测是 Zipformer 还是其他结构，构建底层模型实例。
- 封装 `OnlineRecognizer` 和 `OfflineRecognizer`，支持 `Stream` 输入及 `Batch` 离线批量文件输入。
- 实现 `Punctuation`（标点重建）和 `ITN/nzh`（中文数字正则化）的后处理流水线。

## 阶段四：Tauri 后端指令暴露 (Commands & Events)
目标：拆除原先基于 stdin/stdout 的通信模型，暴露原生的 `Tauri Commands`。

- 提供如 `start_recognizer()`、`stop_recognizer()`、`feed_audio_chunk()` 和 `process_batch_file()` 的命令接口。
- 设计事件驱动机制：随着音频数据流在 Rust 中被识别，利用 Tauri 内置的 `app_handle.emit_all("recognizer-event", result)` 将文本增量等信息异步传递到前端。

## 阶段五：前端与清理
目标：整合到目前的前端代码，并在所有测试跑通后再移除旧的历史包袱。

- 将前端组件原本基于 `Command.sidecar` 孵化和侦听的代码重构成基于 `invoke("...")` 和 `listen("recognizer-event")`。
- 保留旧的 `src-tauri/sidecar` 作为参考和回退方案。
- 只有在新后端实现经过全面测试，所有测试用例（尤其是前端端到端和 Node.js 旧有逻辑的行为一致性测试）通过后，再全面抛弃原有的 `src-tauri/sidecar` 以及相关的依赖配置（`tauri.conf.json` 清洗、`package.json` 组件精简）。
