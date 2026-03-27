# Sona CLI

`sona-cli` 是 Sona 当前面向源码构建场景提供的命令行入口，专注于离线批量转写。

这个 v1 版本只覆盖一条核心工作流：

- 转写单个本地音频或视频文件
- 以 `json`、`txt`、`srt` 或 `vtt` 导出结果
- 与桌面版共享同一份预置模型元数据

## 当前范围

- 在 `src-tauri/` 目录内已支持：`cargo run --bin sona-cli -- ...`
- 在 `src-tauri/` 目录内已支持：`cargo build --release --bin sona-cli`
- 暂不包含：桌面安装包集成、独立 Release 压缩包、实时录音、AI 润色、AI 翻译

## 使用示例

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

如果不传 `--output`，`sona-cli` 会把 `json` 输出到 `stdout`。

## 配置文件

通过 `--config` 显式传入 TOML 配置文件。

最小示例：

```toml
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
format = "srt"
```

支持的配置项：

- `models_dir`
- `model_id`
- `vad_model_id`
- `punctuation_model_id`
- `itn_model_ids`
- `language`
- `threads`
- `enable_itn`
- `vad_buffer_size`
- `format`

命令行参数优先级高于配置文件。

## 必需的伴随模型

某些离线模型依赖伴随资产：

- 如果所选离线模型要求 VAD，必须传 `--vad-model-id` 或在配置文件里设置 `vad_model_id`
- 如果所选离线模型要求标点，必须传 `--punctuation-model-id` 或在配置文件里设置 `punctuation_model_id`

CLI 不会自动帮你选择这些伴随模型。缺失时会直接失败并给出错误提示。

## 常用参数

```text
sona-cli transcribe <input>
  --config <path>
  --output <path>
  --format <json|txt|srt|vtt>
  --language <code>
  --model-id <id>
  --models-dir <path>
  --vad-model-id <id>
  --punctuation-model-id <id>
  --itn-model-id <id>    # 可重复
  --threads <n>
  --enable-itn
  --disable-itn
  --vad-buffer <seconds>
  --save-wav <path>
  --quiet
```

## 手工验收

在本机已经安装桌面版模型的前提下，可以这样验证：

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

预期结果：

- 进度输出到 `stderr`
- 指定的导出文件被创建
- 所选预置模型与伴随模型 id 能在 `models_dir` 下正确解析
