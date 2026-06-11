# Sona

[English](README.md) | [簡體中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md)

**Sona** 是一款功能強大的離線轉錄（字幕）編輯器，由 [Tauri](https://tauri.app)、[React](https://react.dev) 和 [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 建構。它使用高效能的 Rust 後端，直接在您的本機上提供快速、準確且私密的語音轉文字能力。

## ✨ 特性

- **🔒 離線與隱私**：所有語音處理都在您的裝置本機進行。沒有任何資料會離開您的機器。
- **🎙️ 即時轉錄**：以低延遲即時錄製和轉錄音訊。
- **📁 批次處理**：匯入多個音訊或影片檔案，在背景進行批次轉錄。
- **🗂️ 工作區組織**：透過 `工作區`、`專案` 與 `Inbox` 整理已儲存的錄音和匯入內容。
- **📝 互動式編輯器**：與音訊播放同步的富文字編輯器，支援文字校對、說話人標籤和版本快照。
- **👥 說話人檔案與校對**：建立本機說話人檔案，逐段修正說話人標籤，並在匯出前集中檢查候選或匿名說話人分組。
- **✨ LLM 助手**：使用 OpenAI、Anthropic、Gemini 或 Ollama 對轉錄文字進行潤色、翻譯和摘要。
- **🗣️ 即時字幕與語音輸入**：複用同一套離線即時轉錄能力，既可顯示懸浮字幕，也可向其他應用直接輸入文字。
- **📤 智慧匯出**：支援多種格式（TXT、SRT、VTT、JSON）和雙語字幕的匯出。
- **🛟 復原、備份與診斷**：復原中斷任務、匯出包含設定、工作區和文字歷史的輕量備份，並在應用內檢查模型與執行時健康狀態。
- **🔔 通知與自動化**：透過頂部通知中心檢視更新、復原和自動化結果，並在設定中設定資料夾自動化規則。
- **🤖 強大的語音辨識模型**：由 **SenseVoice**、**Whisper** 和 **Paraformer** 等最先進的模型驅動。

## 🚀 快速開始

### 從 GitHub Releases 下載

安裝 Sona 最簡單的方法是從 [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) 頁面下載適合您平台的預編譯二進位檔案。

### 使用者指南

如果您想檢視面向終端使用者的完整使用說明，請閱讀[使用者指南](docs/user-guide.zh-TW.md)。其中包含首次設定、`Live Record`、`Batch Import`、`工作區` / `專案` / `Inbox`、轉錄編輯、說話人校對、版本快照、LLM 功能、`語音輸入法`、匯出、`儀表板` / 備份 / 復原入口，以及常見問題。

### CLI

Sona 現在透過桌面主程式直接提供離線批次轉錄指令。安裝包裡的 CLI 子指令由主程式二進位承載，但預設不會幫您寫入系統 `PATH`。

安裝包內的常見位置：

- Windows：在安裝目錄執行 `Sona.exe transcribe ...`
- macOS：執行 `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux：從安裝位置執行 `Sona` 主程式並附帶 CLI 子指令
- AppImage：執行掛載後的 AppImage 可執行檔並附帶 CLI 子指令

如果您是從原始碼建構，也仍然可以直接透過 Cargo 執行 CLI：

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- \
  transcribe ./sample.mp4 --config ./sona-cli.toml --output ./sample.srt
```

目前 CLI 範圍刻意保持精簡：

- 單檔與目錄離線轉錄
- 預設模型清單檢視、模型下載與模型刪除
- 透過 `sona serve` 啟動無外介面 (headless) HTTP API 服務；詳情請檢視 [docs/api.zh-TW.md](docs/api.zh-TW.md)
- 匯出到 `json`、`txt`、`srt`、`vtt` 或 `md`
- 透過桌面主程式提供，但不會註冊到 `PATH`

完整 CLI 說明和最小 TOML 範例請檢視 [docs/cli.zh-TW.md](docs/cli.zh-TW.md)。

### 從原始碼建構

#### 前置條件

*   **Node.js**: v20 或更高版本 (用於建構前端)。
*   **Rust**: 穩定版 (用於 Tauri 後端)。
*   **套件管理器**: 透過 Corepack 使用 `pnpm` (推薦)。

##### Linux 依賴
如果您使用的是 Linux (Ubuntu/Debian)，請確保您已安裝必要的系統依賴：

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

#### 安裝步驟

1.  **克隆倉庫**
    ```bash
    git clone https://github.com/AirSodaz/sona.git
    cd sona
    ```

2.  **安裝依賴**
    ```bash
    corepack enable
    pnpm install
    ```

3.  **執行應用**
    ```bash
    pnpm run tauri dev
    ```

4.  **執行前端測試**
    ```bash
    pnpm test
    ```

## 📦 模型管理

Sona 允許您選擇最適合您需求的 AI 模型，無論是離線轉錄還是線上助手。

### 離線轉錄
1.  進入 **Settings > Model Settings**（設定 > 模型設定）。
2.  從精心挑選的高效能模型清單中進行選擇：
    *   **SenseVoice**：多語言支援和情感辨識的最佳選擇。
    *   **Whisper (Tiny)**：OpenAI Whisper 模型的輕量級版本。
    *   **Paraformer**：專為流式辨識最佳化。
3.  點擊 **Download**（下載）。模型將自動儲存在本機。

### LLM 助手（潤色、翻譯與摘要）
1.  進入 **Settings > LLM Service**（設定 > LLM 服務）。
2.  選擇您的服務提供者（OpenAI、Anthropic、Gemini 或 Ollama）。
3.  輸入您的 API 金鑰和 Base URL（如果適用）。
4.  選擇為潤色、翻譯和摘要產生提供支援的模型。

## 🏗️ 建構

要建構生產環境用的應用程式：

```bash
pnpm run tauri build
```

可執行檔將產生在 `src-tauri/target/release/bundle` 目錄中。
