# 🚀 重构计划：将 Node.js Sidecar 迁移至 Tauri 原生 (Rust) 后端

- [x] 1. **前期调研与依赖选型 (Rust)**
  - [x] 确定 `sherpa-onnx` 的 Rust 绑定方案（例如：直接使用 `bindgen` 调用 C API 或使用 `sherpa-rs` 这类已有库）。(最终选定 `sherpa-onnx` crate 防止 Windows bindgen 问题)
  - [x] 确定音频处理库（`hound`, `symphonia` 等替代 `wavefile` 及部分 `ffmpeg` 特性）。 (已安装 `hound`, `symphonia`)
  - [x] 确定解压解包库（使用 `sevenz-rust` 替代 `7zip-bin`/`node-7z`）。 (已安装 `sevenz-rust`)
  - [x] 整理并在 `src-tauri/Cargo.toml` 中引入所有必要的 crates。 (全部引入并且通过 `cargo build` 编译链接测试)

- [x] 2. **核心业务逻辑：音频处理 (Audio Pipeline)**
  - [x] 使用 Rust 实现基于 VAD (Voice Activity Detection) 或固定片段的音频切分（移植 `yieldVADSegments` 和 `yieldFixedChunks`）。
  - [x] 实现音频数据 (PCM/WAV) 内存缓冲区处理、转码及文件写入。

- [x] 3. **核心业务逻辑：引入与封装 `sherpa-onnx`**
  - [x] 实现模型路径自动检测与配置构建 (`findModelConfig`)。
  - [x] 实例化 OnlineRecognizer (流式) / OfflineRecognizer (批量)。
  - [ ] 实现标点符号模型集成 (Punctuation)。(注：当前 `sherpa-onnx` 0.1.8 crate 未开放 Punctuation FFI 接口，暂缓集成)
  - [x] 移植逆文本正则化 (ITN/nzh，中文数字转换) 逻辑。

- [x] 4. **Tauri 主应用集成 (Managed State & Commands)**
  - [x] 设计状态机：将 Recognizer 实例注入并挂载到 Tauri `State`。
  - [x] 实现 `tauri::command`：供前端调用启动、停止、配置模型（替代原有基于 IPC 及子进程启动的方式）。
  - [x] 实现事件机制：将流式音频识别结果、状态变更等，通过 `app_handle.emit_all()` 即时推送到前端界面（替代原有写入 stdout JSON 的方式）。

- [ ] 5. **前端适配与重构 (Frontend Adaptation)**
  - [ ] 移除前端代码中原本通过 `tauri/api/process/Command` 孵化 (spawn) Node/Sidecar 的逻辑。
  - [ ] 修改为直接使用 `@tauri-apps/api/core/invoke` 调用后端的 Rust Command。
  - [ ] 修改事件监听：由监听 `Command::on('line')` 改为监听 Tauri Window 的原生事件 (`listen('recognizer-output')`)。

- [ ] 6. **测试验证与最终清理**
  - [ ] 确保前端所有相关端到端测试跑通。
  - [ ] 确保新旧识别引擎在文本准确率和流式延迟指标上的表现行为一致。
  - [ ] **（测试全部通过后）** 移除 `src-tauri/sidecar` 目录、`package.json` 中的 `sherpa-onnx-node` 等无用前端/Node依赖包。
  - [ ] 移除 `tauri.conf.json` 中的 `sidecar` 配置，直接将 Tauri 构建配置精简。
