# Sona CLI

`sona-cli` 是 Sona 的独立命令行程序。桌面端 Tauri 应用已经不再内嵌 CLI 子命令，因此打包后的桌面应用请使用 `sona-cli`，不要再把 `Sona` 当作命令行入口。

当前独立 CLI 已提供的命令如下：

- `path-status`
- `init-config`
- `models list`
- `models download`
- `models delete`

离线转写和无头 API 服务正在迁移到 `sona-cli`，目前还不属于现阶段独立 CLI 的已发布能力。

## 运行方式

- 安装包：使用同平台安装包里附带的 `sona-cli`
- 源码：`cargo run -p sona-cli -- <command> ...`

示例：

```bash
cargo run -p sona-cli -- path-status .
cargo run -p sona-cli -- init-config
cargo run -p sona-cli -- models list --json
```

## 命令

### `path-status`

通过共享的运行时状态契约解析一个文件系统路径，并将 JSON 输出到 `stdout`。

```bash
sona-cli path-status ./models
```

示例输出：

```json
{
  "kind": "directory",
  "path": "C:/work/models",
  "error": null
}
```

### `init-config`

生成一份带注释的起始配置文件，供后续独立 CLI 工作流使用。

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml --force
```

- 默认输出路径：`./sona-cli.toml`
- 已有文件默认受保护，传入 `--force` 才会覆盖
- 状态文本写入 `stderr`

### `models list`

列出预置模型，并可按模式、类型、语言、安装状态过滤。

```bash
sona-cli models list
sona-cli models list --mode offline --type whisper
sona-cli models list --language zh --installed
sona-cli models list --json
```

传入 `--json` 会输出完整的机器可读结构，包括 `install_path`。

### `models download`

下载一个预置模型到解析出的模型目录。

```bash
sona-cli models download sherpa-onnx-whisper-turbo
sona-cli models download silero-vad --models-dir ./models --yes
```

如果目标预置模型依赖伴生资源，`sona-cli` 也会同时下载所需的 VAD 或标点模型。

### `models delete`

删除一个已安装的预置模型。

```bash
sona-cli models delete sherpa-onnx-whisper-turbo --yes
sona-cli models delete silero-vad --models-dir ./models --yes
```

不会自动删除伴生模型。

## 全局参数

```text
sona-cli
  -V, --version
  -h, --help
```

可以通过 `sona-cli <command> --help` 查看对应命令的参数说明。
