# Sona CLI

`sona` 透過桌面主程式提供離線轉錄指令。安裝包不會把 `sona` 寫入 shell `PATH`，因此需要直接執行已安裝的應用二進位檔案並附帶 CLI 子指令。從原始碼建構時，也可以用 Cargo 執行同一組命令。

CLI 範圍刻意保持精簡：單檔和目錄離線轉錄、預設模型清單/下載/刪除、無外介面 (headless) HTTP API 服務啟動。它不包含即時錄音、LLM 潤色或 LLM 翻譯。

## 執行方式

- Windows：在安裝目錄執行 `Sona.exe transcribe ...`
- macOS：執行 `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux 安裝包：從安裝位置執行 `Sona` 主程式並附帶 CLI 子指令
- AppImage：執行掛載後的 AppImage 可執行檔並附帶 CLI 子指令
- 原始碼：`cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 --config ./sona-cli.toml`

## 常用指令

### 轉錄檔案

```bash
sona transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

不指定 `--output` 時，轉錄結果會以 JSON 寫入 `stdout`。指定 `--output` 時，格式會從副檔名推斷，除非同時傳入 `--format`。

### 轉錄目錄

```bash
sona transcribe \
  --input-dir ./media \
  --output-dir ./transcripts \
  --format srt \
  --recursive \
  --jobs 1 \
  --config ./sona-cli.toml
```

目錄模式會為每個受支援媒體檔案在 `--output-dir` 中寫出一個轉錄檔案。預設只掃描目錄直屬檔案；加入 `--recursive` 後會包含子目錄，並保留相對輸出路徑。轉錄正文寫入檔案，`stdout` 會輸出 JSON 成功/失敗彙總。

### 列出、下載或刪除模型

```bash
sona models list --mode offline --type whisper
sona models list --language zh --installed
sona models download sherpa-onnx-whisper-turbo
sona models delete sherpa-onnx-whisper-turbo
```

當所選預設模型需要伴生模型時，`models download` 會自動下載所需模型，例如 `silero-vad` 或預設標點模型。
`models delete` 只會刪除指定模型，不會自動刪除伴生模型。

### 啟動 API 服務

```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key
```

HTTP API 端點和請求範例見 [api.zh-TW.md](api.zh-TW.md)。

## 設定檔

透過 `--config` 傳入 TOML 檔案。命令列參數會覆寫設定檔中的值。

最小 `transcribe` 範例：

```toml
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
gpu_acceleration = "auto"
format = "srt"
```

### `transcribe` 設定鍵

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `models_dir` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | CLI 找不到桌面模型目錄時請明確傳入。 |
| `model_id` | 必選，除非傳入 `--model-id` | 離線預設模型 ID | 無 | 用 `sona models list --mode offline` 檢視可用 ID。 |
| `vad_model_id` | 條件必選 | 預設模型 ID | 無 | 所選模型需要 VAD 時必選。 |
| `punctuation_model_id` | 條件必選 | 預設模型 ID | 無 | 所選模型需要標點時必選。 |
| `language` | 可選 | `auto` 或模型語言代碼，如 `zh`、`en`、`ja` | `auto` | 覆寫自動語言偵測。 |
| `threads` | 可選 | 大於 `0` 的整數 | `4` | 辨識執行緒數。 |
| `enable_itn` | 可選 | `true` 或 `false` | `false` | 啟用逆文字歸一化。 |
| `vad_buffer_size` | 可選 | 大於 `0` 的數字 | `5.0` | VAD 緩衝秒數。 |
| `gpu_acceleration` | 可選 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | 使用 `cpu` 可明確關閉 GPU 加速。 |
| `format` | 可選 | `json`、`txt`、`srt`、`vtt`、`md` | 寫入 stdout 或目錄模式時為 `json`，否則從 `--output` 推斷 | 覆寫輸出副檔名推斷。 |

### `serve` 設定鍵

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `host` | 可選 | 監聽地址 | `0.0.0.0` | 本機存取可用 `127.0.0.1`。 |
| `port` | 可選 | TCP 連接埠 `0` 到 `65535` | `14200` | API 服務連接埠。 |
| `api_key` | 可選 | 字串 | 空 | 為空時請求不需要 Bearer 認證。 |
| `models_dir` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | 用於解析已安裝模型。 |
| `ip_whitelist` | 可選 | 逗號分隔規則 | `localhost` | 支援 `localhost`、精確 IP、CIDR、`*`，以及 `192.168.*` 這類 IPv4 萬用字元。 |
| `max_streaming` | 可選 | 非負整數 | `2` | 最大併發流式 WebSocket 連線數。 |
| `max_concurrent` | 可選 | 非負整數 | `2` | 最大併發批次任務數。 |
| `max_queue_size` | 可選 | 非負整數 | `100` | `0` 表示佇列基本不限。 |
| `max_upload_size_mb` | 可選 | 非負整數 | `50` | `0` 表示關閉上傳大小限制。 |
| `job_ttl_minutes` | 可選 | 非負整數 | `60` | `0` 表示關閉完成/失敗任務清理。 |
| `gpu_acceleration` | 可選 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | 本機批次和流式任務的服務級預設值。 |
| `vad_model_id` | 可選 | 預設模型 ID | `silero-vad` | API 服務任務的預設 VAD 伴生模型。 |
| `punctuation_model_id` | 可選 | 預設模型 ID | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | API 服務任務的預設標點伴生模型。 |

## 參數

### 全域

```text
sona
  -V, --version
  -v, --verbose
  -h, --help
  help
```

使用 `-V` 或 `--version` 列印 Sona 版本號。使用 `-v` 或 `--verbose` 放在子指令前啟用詳細診斷日誌。使用 `-h`、`--help` 或 `help` 列印指令說明：

```bash
sona --version
sona -V
sona -v models list
sona --verbose transcribe ./sample.mp4 --config ./sona-cli.toml
sona transcribe --help
```

詳細診斷日誌會寫入 `stderr`。指令結果仍寫入 `stdout`，包括 `models list` 的 JSON 輸出，以及 `transcribe` 未指定 `--output` 時的輸出，因此仍可安全透過管線傳給其他工具。

### `transcribe`

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `<input>` | 必選，除非傳入 `--input-dir` | 本機音訊或影片檔案路徑 | 無 | 要轉錄的單一檔案。 |
| `--input-dir <dir>` | 目錄模式必選 | 目錄路徑 | 無 | 轉錄目錄中的受支援媒體檔案。 |
| `--config <path>` | 可選 | TOML 檔案路徑 | 無 | 從設定檔載入預設值。 |
| `--output <path>` | 可選 | 檔案系統路徑 | `stdout` | 輸出檔案路徑。 |
| `--output-dir <dir>` | 與 `--input-dir` 同用時必選 | 目錄路徑 | 無 | 為每個輸入檔案寫出一個轉錄檔案。 |
| `--recursive` | 可選 | 旗標 | 關閉 | 掃描子目錄並保留相對輸出路徑。 |
| `--jobs <n>` | 可選 | 大於 `0` 的整數 | `1` | 目錄模式下最大併發檔案任務數。 |
| `--format <format>` | 可選 | `json`、`txt`、`srt`、`vtt`、`md` | 寫入 stdout 或目錄模式時為 `json`，否則從 `--output` 推斷 | 覆寫設定和輸出副檔名推斷。 |
| `--language <code>` | 可選 | `auto` 或模型語言代碼 | `auto` | 覆寫設定。 |
| `--model-id <id>` | 必選，除非設定了 `model_id` | 離線預設模型 ID | 無 | 主轉錄模型。 |
| `--models-dir <path>` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | 覆寫設定。 |
| `--vad-model-id <id>` | 條件必選 | 預設模型 ID | 無 | 所選模型需要 VAD 時必選。 |
| `--punctuation-model-id <id>` | 條件必選 | 預設模型 ID | 無 | 所選模型需要標點時必選。 |
| `--threads <n>` | 可選 | 大於 `0` 的整數 | `4` | 覆寫設定。 |
| `--enable-itn` | 可選 | 旗標 | `false` | 與 `--disable-itn` 互斥。 |
| `--disable-itn` | 可選 | 旗標 | `false` | 覆寫 `enable_itn = true`；與 `--enable-itn` 互斥。 |
| `--hotwords <words>` | 可選 | 逗號分隔片語 | 無 | 僅 CLI 參數；目前支援 Transducer 和 Qwen3 模型。 |
| `--gpu-acceleration <provider>` | 可選 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | 覆寫設定。 |
| `--vad-buffer <seconds>` | 可選 | 大於 `0` 的數字 | `5.0` | `vad_buffer_size` 的 CLI 參數名。 |
| `--save-wav <path>` | 可選 | 檔案系統路徑 | 無 | 僅 CLI 參數；儲存中間重取樣 WAV。與 `--input-dir` 不相容。 |
| `--quiet` | 可選 | 旗標 | 關閉 | 僅 CLI 參數；隱藏轉錄進度。 |

### `models list`

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `--models-dir <path>` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | 用於偵測已安裝預設模型。 |
| `--mode <mode>` | 可選 | `streaming`、`offline` | 所有模式 | 按支援模式過濾。 |
| `--type <type>` | 可選 | 預設模型類型，如 `whisper`、`vad`、`punctuation` | 所有類型 | 按模型類型過濾。 |
| `--language <code>` | 可選 | 語言代碼，如 `zh`、`en`、`ja`、`yue` | 所有語言 | 依支援的語言代碼過濾。 |
| `--installed` | 可選 | 旗標 | 關閉 | 只顯示 `models_dir` 中已存在的模型。 |
| 輸出 | 總是 | JSON | JSON | 寫入 `stdout`。 |

### `models download`

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `<model_id>` | 必選 | 已知預設模型 ID | 無 | 要下載的主模型。 |
| `--models-dir <path>` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | 目標模型目錄。 |
| `--quiet` | 可選 | 旗標 | 關閉 | 隱藏單個下載進度。 |
| 伴生模型下載 | 自動 | 所需 VAD 和標點預設模型 | 自動 | 下載主模型時會同時下載必需伴生模型。 |

### `models delete`

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `<model_id>` | 必選 | 已知預設模型 ID | 無 | 要刪除的模型。 |
| `--models-dir <path>` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | 目標模型目錄。 |
| `--yes` | 可選 | 旗標 | 關閉 | 跳過互動確認提示。 |
| 安裝路徑缺失 | 否 | 已知但未安裝的預設模型 | 成功 no-op | 向 `stderr` 輸出提示並以狀態碼 0 退出。 |
| 伴生模型刪除 | 否 | 所需 VAD 和標點預設模型 | 不刪除 | 如果不再需要伴生模型，請明確刪除對應模型。 |

### `serve`

| 參數 / 設定鍵 | 必要性 | 取值範圍 | 預設值 | 說明 |
| --- | --- | --- | --- | --- |
| `--config <path>` | 可選 | TOML 檔案路徑 | 無 | 從設定檔載入預設值。 |
| `--host <ip>` | 可選 | 監聽地址 | `0.0.0.0` | 覆寫設定。 |
| `--port <port>` | 可選 | TCP 連接埠 `0` 到 `65535` | `14200` | 覆寫設定。 |
| `--api-key <key>` | 可選 | 字串 | 空 | 為空時不啟用 Bearer 認證。 |
| `--models-dir <path>` | 可選 | 檔案系統路徑 | 可推斷時使用桌面應用模型目錄 | 覆寫設定。 |
| `--ip-whitelist <rules>` | 可選 | 逗號分隔規則 | `localhost` | 支援 `localhost`、精確 IP、CIDR、`*`，以及 `192.168.*` 這類 IPv4 萬用字元。 |
| `--max-streaming <n>` | 可選 | 非負整數 | `2` | 最大併發流式連線數。 |
| `--max-concurrent <n>` | 可選 | 非負整數 | `2` | 最大併發批次任務數。 |
| `--max-queue-size <n>` | 可選 | 非負整數 | `100` | `0` 表示佇列基本不限。 |
| `--max-upload-size-mb <n>` | 可選 | 非負整數 | `50` | `0` 表示關閉上傳大小限制。 |
| `--job-ttl-minutes <n>` | 可選 | 非負整數 | `60` | `0` 表示關閉完成/失敗任務清理。 |
| `--gpu-acceleration <provider>` | 可選 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | HTTP API 請求不支援依個別請求覆寫 GPU 設定。 |
| `--vad-model-id <id>` | 可選 | 預設模型 ID | `silero-vad` | API 服務任務的預設 VAD 伴生模型。 |
| `--punctuation-model-id <id>` | 可選 | 預設模型 ID | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | API 服務任務的預設標點伴生模型。 |

執行 `sona <command> --help` 可檢視 clap 產生的完整幫助文字。
