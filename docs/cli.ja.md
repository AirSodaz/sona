# Sona CLI

`sona` は、デスクトップアプリ本体の実行ファイルを通じてオフライン文字起こし用の CLI コマンドを提供します。インストーラー版は `sona` を shell の `PATH` に登録しないため、インストール済みアプリのバイナリに CLI サブコマンドを付けて実行してください。ソースからビルドしている場合は、Cargo から同じコマンドを実行できます。

CLI の範囲は意図的に絞っています。現在含まれるのは、単一ファイルとディレクトリのオフライン文字起こし、プリセットモデルの一覧表示 / ダウンロード / 削除、ヘッドレス HTTP API サーバー起動です。`Live Record`、`LLM Polish`、`Translate` は CLI には含まれません。

## 実行方法

- Windows: インストールディレクトリで `Sona.exe transcribe ...` を実行します。
- macOS: `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...` を実行します。
- Linux パッケージ: インストール先の `Sona` バイナリに CLI サブコマンドを付けて実行します。
- AppImage: マウントした AppImage の実行ファイルに CLI サブコマンドを付けて実行します。
- ソース: `cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 --config ./sona-cli.toml`

## よく使うコマンド

### ファイルを文字起こしする

```bash
sona transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

`--output` を指定しない場合、文字起こし結果は JSON として `stdout` に出力されます。`--output` を指定した場合、`--format` を明示しない限り、出力形式はファイル拡張子から推定されます。

### ディレクトリを文字起こしする

```bash
sona transcribe \
  --input-dir ./media \
  --output-dir ./transcripts \
  --format srt \
  --recursive \
  --jobs 1 \
  --config ./sona-cli.toml
```

ディレクトリモードでは、対応している各メディアファイルごとに `--output-dir` へ文字起こしファイルを書き出します。既定では直下のファイルだけを走査します。`--recursive` を追加するとサブディレクトリも対象になり、相対パスを保ったまま出力されます。文字起こし本文はファイルへ書き出され、`stdout` には JSON の成功 / 失敗サマリーが出力されます。

### モデルを一覧表示、ダウンロード、削除する

```bash
sona models list --mode offline --type whisper
sona models list --language zh --installed
sona models download sherpa-onnx-whisper-turbo
sona models delete sherpa-onnx-whisper-turbo
```

選択したプリセットが `silero-vad` や既定の句読点モデルなどの関連モデルを必要とする場合、`models download` はそれらも自動でダウンロードします。
`models delete` は指定したモデルだけを削除します。関連モデルは自動削除されません。

### API サーバーを起動する

```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key
```

HTTP API のエンドポイントとリクエスト例は [api.ja.md](api.ja.md) を参照してください。

## 設定ファイル

`--config` で TOML ファイルを渡します。コマンドラインフラグは設定ファイルの値を上書きします。

最小限の `transcribe` 設定例:

```toml
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
gpu_acceleration = "auto"
hotwords = "Sona,offline ASR"
format = "srt"
```

### `transcribe` 設定キー

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `models_dir` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | CLI がデスクトップアプリのモデルを検出できない場合に明示します。 |
| `model_id` | `--model-id` を渡さない場合は必須 | オフラインプリセットモデル ID | None | `sona models list --mode offline` で ID を確認します。 |
| `vad_model_id` | Optional | プリセットモデル ID | 必要な場合は `silero-vad` | 選択したモデルが VAD を必要とする場合に使用され、既定値を上書きできます。 |
| `punctuation_model_id` | Optional | プリセットモデル ID | 必要な場合は `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | 選択したモデルが句読点モデルを必要とする場合に使用され、既定値を上書きできます。 |
| `language` | Optional | `auto` または `zh`、`en`、`ja` などのモデル言語コード | `auto` | 自動言語検出を上書きします。 |
| `threads` | Optional | `0` より大きい整数 | `4` | 認識処理のスレッド数。 |
| `enable_itn` | Optional | `true` または `false` | `false` | 逆テキスト正規化を有効にします。 |
| `hotwords` | Optional | カンマ区切り語句 | None | ASR 用のカスタムホットワード。現在は Transducer と Qwen3 モデルで対応しています。 |
| `vad_buffer_size` | Optional | `0` より大きい数値 | `5.0` | VAD バッファサイズを秒単位で指定します。 |
| `gpu_acceleration` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | GPU アクセラレーションを無効化する場合は `cpu` を指定します。 |
| `format` | Optional | `json`, `txt`, `srt`, `vtt`, `md` | `stdout` またはディレクトリモードでは `json`、それ以外は `--output` から推定 | 出力拡張子からの推定を上書きします。 |

### `serve` 設定キー

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `host` | Optional | バインドアドレス | `0.0.0.0` | ローカル限定にする場合は `127.0.0.1` を使います。 |
| `port` | Optional | TCP ポート `0` から `65535` | `14200` | API サーバーのポートです。 |
| `api_key` | Optional | 文字列 | 空 | 空の場合、Bearer 認証で保護されません。 |
| `models_dir` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | インストール済みモデルの解決に使います。 |
| `ip_whitelist` | Optional | カンマ区切りルール | `localhost` | `localhost`、完全一致 IP、CIDR、`*`、`192.168.*` などの IPv4 ワイルドカードに対応します。 |
| `max_streaming` | Optional | 非負整数 | `2` | 最大同時ストリーミング WebSocket 接続数。 |
| `max_concurrent` | Optional | 非負整数 | `2` | 最大同時バッチジョブ数。 |
| `max_queue_size` | Optional | 非負整数 | `100` | `0` は実質的にキュー無制限を意味します。 |
| `max_upload_size_mb` | Optional | 非負整数 | `50` | `0` はアップロードサイズ制限を無効化します。 |
| `job_ttl_minutes` | Optional | 非負整数 | `60` | `0` は完了 / 失敗ジョブのクリーンアップを無効化します。 |
| `gpu_acceleration` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | ローカルバッチジョブとストリーミングジョブに使うサーバーレベルの既定値です。 |
| `vad_model_id` | Optional | プリセットモデル ID | `silero-vad` | API サーバージョブ用の既定 VAD 関連モデルです。 |
| `punctuation_model_id` | Optional | プリセットモデル ID | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | API サーバージョブ用の既定句読点関連モデルです。 |

## パラメータ

### Global

```text
sona
  -V, --version
  -v, --verbose
  -h, --help
  help
```

Sona のバージョンを表示するには `-V` または `--version` を使います。詳細な診断ログを有効にするには、サブコマンドの前に `-v` または `--verbose` を置きます。コマンドヘルプは `-h`、`--help`、または `help` で表示できます。

```bash
sona --version
sona -V
sona -v models list
sona --verbose transcribe ./sample.mp4 --config ./sona-cli.toml
sona transcribe --help
```

詳細診断は `stderr` に出力されます。`models list` の JSON や、`--output` なしの `transcribe` 結果などのコマンド出力は `stdout` に残るため、他のツールへ安全にパイプできます。

高度なラッパーやテストでは `SONA_FORCE_CLI=1` を設定すると、実行ファイルが認識済みの CLI サブコマンドなしで起動された場合でも CLI モードを強制できます。

### `transcribe`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `<input>` | `--input-dir` を渡さない場合は必須 | ローカル音声または動画ファイルパス | None | 文字起こしする単一ファイル。 |
| `--input-dir <dir>` | ディレクトリモードでは必須 | ディレクトリパス | None | ディレクトリ内の対応メディアファイルを文字起こしします。 |
| `--config <path>` | Optional | TOML ファイルパス | None | 設定ファイルから既定値を読み込みます。 |
| `--output <path>` | Optional | ファイルシステムパス | `stdout` | 出力ファイルパス。 |
| `--output-dir <dir>` | `--input-dir` と併用する場合は必須 | ディレクトリパス | None | 入力ファイルごとに文字起こしファイルを書き出します。 |
| `--recursive` | Optional | フラグ | Off | サブディレクトリを走査し、相対出力パスを保ちます。 |
| `--jobs <n>` | Optional | `0` より大きい整数 | `1` | ディレクトリモードの最大同時ファイルジョブ数。 |
| `--format <format>` | Optional | `json`, `txt`, `srt`, `vtt`, `md` | `stdout` またはディレクトリモードでは `json`、それ以外は `--output` から推定 | 設定と出力拡張子による推定を上書きします。 |
| `--language <code>` | Optional | `auto` またはモデル言語コード | `auto` | 設定を上書きします。 |
| `--model-id <id>` | `model_id` が設定されていない場合は必須 | オフラインプリセットモデル ID | None | メインの文字起こしモデル。 |
| `--models-dir <path>` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | 設定を上書きします。 |
| `--vad-model-id <id>` | Optional | プリセットモデル ID | 必要な場合は `silero-vad` | 既定の VAD 関連モデルを上書きします。 |
| `--punctuation-model-id <id>` | Optional | プリセットモデル ID | 必要な場合は `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | 既定の句読点関連モデルを上書きします。 |
| `--threads <n>` | Optional | `0` より大きい整数 | `4` | 設定を上書きします。 |
| `--enable-itn` | Optional | フラグ | `false` | `--disable-itn` と同時には使えません。 |
| `--disable-itn` | Optional | フラグ | `false` | `enable_itn = true` を上書きします。`--enable-itn` と同時には使えません。 |
| `--hotwords <words>` | Optional | カンマ区切り語句 | None | `hotwords` を上書きします。現在は Transducer と Qwen3 モデルで対応しています。 |
| `--gpu-acceleration <provider>` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | 設定を上書きします。 |
| `--vad-buffer <seconds>` | Optional | `0` より大きい数値 | `5.0` | `vad_buffer_size` の CLI 引数名です。 |
| `--save-wav <path>` | Optional | ファイルシステムパス | None | CLI 専用。中間のリサンプリング WAV を保存します。`--input-dir` とは併用できません。 |
| `--quiet` | Optional | フラグ | Off | CLI 専用。文字起こし進捗を非表示にします。 |

### `models list`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `--models-dir <path>` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | インストール済みプリセットの検出に使います。 |
| `--mode <mode>` | Optional | `streaming`, `offline` | すべてのモード | 対応モードで絞り込みます。 |
| `--type <type>` | Optional | `whisper`、`vad`、`punctuation` などのプリセットモデル種別 | すべての種別 | モデル種別で絞り込みます。 |
| `--language <code>` | Optional | `zh`、`en`、`ja`、`yue` などの言語トークン | すべての言語 | 対応言語トークンで絞り込みます。 |
| `--installed` | Optional | フラグ | Off | `models_dir` に存在するモデルだけを表示します。 |
| Output | Always | JSON | JSON | `stdout` に出力されます。 |

### `models download`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `<model_id>` | Required | 既知のプリセットモデル ID | None | ダウンロードするメインモデル。 |
| `--models-dir <path>` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | 保存先モデルディレクトリ。 |
| `--quiet` | Optional | フラグ | Off | ダウンロードごとの進捗を非表示にします。 |
| Companion downloads | Automatic | 必須の VAD と句読点プリセット | Automatic | メインモデルのダウンロード時に必須関連モデルもダウンロードします。 |

### `models delete`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `<model_id>` | Required | 既知のプリセットモデル ID | None | 削除するモデル。 |
| `--models-dir <path>` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | 対象モデルディレクトリ。 |
| `--yes` | Optional | フラグ | Off | 対話確認を省略します。 |
| Missing install path | No | 既知だが未インストールのプリセット | 成功 no-op | `stderr` に通知して終了コード 0 で終了します。 |
| Companion deletion | No | 必須の VAD と句読点プリセット | 削除しない | 不要な関連モデルは明示的に削除してください。 |

### `serve`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `--config <path>` | Optional | TOML ファイルパス | None | 設定ファイルから既定値を読み込みます。 |
| `--host <ip>` | Optional | バインドアドレス | `0.0.0.0` | 設定を上書きします。 |
| `--port <port>` | Optional | TCP ポート `0` から `65535` | `14200` | 設定を上書きします。 |
| `--api-key <key>` | Optional | 文字列 | 空 | 空の場合は Bearer 認証なしです。 |
| `--models-dir <path>` | Optional | ファイルシステムパス | 推定できる場合はデスクトップアプリのモデルディレクトリ | 設定を上書きします。 |
| `--ip-whitelist <rules>` | Optional | カンマ区切りルール | `localhost` | `localhost`、完全一致 IP、CIDR、`*`、`192.168.*` などの IPv4 ワイルドカードに対応します。 |
| `--max-streaming <n>` | Optional | 非負整数 | `2` | 最大同時ストリーミング接続数。 |
| `--max-concurrent <n>` | Optional | 非負整数 | `2` | 最大同時バッチジョブ数。 |
| `--max-queue-size <n>` | Optional | 非負整数 | `100` | `0` は実質的なキュー無制限を意味します。 |
| `--max-upload-size-mb <n>` | Optional | 非負整数 | `50` | `0` はアップロードサイズ制限を無効化します。 |
| `--job-ttl-minutes <n>` | Optional | 非負整数 | `60` | `0` は完了 / 失敗ジョブのクリーンアップを無効化します。 |
| `--gpu-acceleration <provider>` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | HTTP API リクエストごとの GPU 上書きは受け付けません。 |
| `--vad-model-id <id>` | Optional | プリセットモデル ID | `silero-vad` | API サーバージョブの既定 VAD 関連モデル。 |
| `--punctuation-model-id <id>` | Optional | プリセットモデル ID | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | API サーバージョブの既定句読点関連モデル。 |

完全な clap 生成ヘルプは `sona <command> --help` で確認できます。
