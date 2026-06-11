# Sona

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md)

**Sona** は、[Tauri](https://tauri.app)、[React](https://react.dev)、[Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) で作られた、オフライン中心の文字起こしエディタです。高性能な Rust バックエンドを使い、音声からテキストへの変換をできるだけ手元のマシン上で高速かつプライベートに実行します。

## Features

- **オフラインとプライバシー**: 音声処理は基本的にローカルデバイス上で実行されます。
- **Live Record**: 低遅延で録音しながら、リアルタイムに文字起こしを確認できます。
- **Batch Import**: 複数の音声・動画ファイルを取り込み、バックグラウンドでまとめて文字起こしできます。
- **Workspace Organization**: `Workspace`、`Projects`、`Inbox` で保存済みの録音やインポートを整理できます。
- **Interactive Editor**: 音声再生と同期したエディタで、セグメント単位の修正、話者ラベル、Version Snapshots を扱えます。
- **Speaker Profiles & Review**: ローカルの Speaker Profiles を作成し、候補や匿名話者グループをエクスポート前に確認できます。
- **LLM Assistant**: OpenAI、Anthropic、Gemini、Ollama などを設定して、AI Polish、Translate、AI Summary を任意で使えます。
- **Live Caption & Voice Typing**: 同じオフラインのリアルタイム文字起こし基盤を使い、フローティング字幕や他アプリへの入力を行えます。
- **Smart Export**: TXT、SRT、VTT、JSON などの形式へ、原文・翻訳・二言語形式で書き出せます。
- **Recovery, Backup & Diagnostics**: 中断した作業の復元、軽量バックアップ、モデルや実行環境の診断に対応しています。
- **Notifications & Automation**: ヘッダー通知センターで更新、復元、自動化結果を確認し、Settings からフォルダー監視ルールを設定できます。
- **Advanced ASR Models**: SenseVoice、Whisper、Paraformer などのモデルを利用できます。

## Getting Started

### GitHub Releases からダウンロードする

Sona を使い始める一番簡単な方法は、[GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) から自分の OS に合うビルドをダウンロードすることです。

### User Guide

初回セットアップや日常的な使い方は、[ユーザーガイド](docs/user-guide.ja.md) を参照してください。`Live Record`、`Batch Import`、`Workspace` / `Projects` / `Inbox`、編集、Speaker Review、Version Snapshots、LLM 機能、`Voice Typing`、エクスポート、`Dashboard` / バックアップ / 復元、トラブルシューティングをまとめています。

### CLI

Sona は、デスクトップアプリ本体の実行ファイルを通じて、オフライン文字起こし用の CLI サブコマンドを提供します。インストーラー版は通常 `PATH` に登録しないため、インストール済みアプリのバイナリに CLI サブコマンドを付けて実行してください。

インストール済みパッケージの代表的な実行場所:

- Windows: インストールディレクトリで `Sona.exe transcribe ...` を実行
- macOS: `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...` を実行
- Linux: インストール先の `Sona` バイナリに CLI サブコマンドを付けて実行
- AppImage: マウントした AppImage の実行ファイルに CLI サブコマンドを付けて実行

ソースからビルドしている場合は、Cargo から同じ CLI を実行できます。

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 --config ./sona-cli.toml --output ./sample.srt
```

現在の CLI 範囲は意図的に絞っています。

- 単一ファイルとディレクトリのオフライン文字起こし
- プリセットモデルの一覧表示、ダウンロード、削除
- `sona serve` によるヘッドレス HTTP API サーバー起動。詳細は [docs/api.ja.md](docs/api.ja.md) を参照
- `json`、`txt`、`srt`、`vtt`、`md` への書き出し
- デスクトップアプリ本体から提供し、既定では `PATH` へ登録しない

完全な CLI ガイドと最小 TOML 例は [docs/cli.ja.md](docs/cli.ja.md) を参照してください。

### ソースからビルドする

#### 前提条件

- **Node.js**: v20 以降。フロントエンドのビルドに使います。
- **Rust**: 安定版。Tauri バックエンドに必要です。
- **Package Manager**: Corepack 経由の `pnpm` を推奨します。

##### Linux の依存関係

Linux (Ubuntu / Debian) でビルドする場合は、必要なシステム依存関係をインストールしてください。

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

#### セットアップ手順

1. **リポジトリを clone する**

   ```bash
   git clone https://github.com/AirSodaz/sona.git
   cd sona
   ```

2. **依存関係をインストールする**

   ```bash
   corepack enable
   pnpm install
   ```

3. **アプリを起動する**

   ```bash
   pnpm run tauri dev
   ```

4. **フロントエンドテストを実行する**

   ```bash
   pnpm test
   ```

## Model Management

Sona では、オフライン文字起こしと任意のオンライン AI 機能に使うモデルを用途ごとに選べます。

### Offline Transcription

1. `Settings > Model Settings` を開きます。
2. 用途に合うモデルを選びます。
   - **SenseVoice**: 多言語対応と感情認識に向いたモデル。
   - **Whisper (Tiny)**: OpenAI Whisper の軽量版。
   - **Paraformer**: ストリーミング向けに最適化されたモデル。
3. `Download` をクリックします。モデルはローカルに保存されます。

### LLM Assistant (Polish, Translate, Summary)

1. `Settings > LLM Service` を開きます。
2. OpenAI、Anthropic、Gemini、Ollama などのプロバイダーを選びます。
3. 必要に応じて API Key、Base URL、Endpoint、Deployment Name などを入力します。
4. Polish、Translation、Summary に使うモデルを割り当てます。

## Building

本番用にアプリをビルドするには、次のコマンドを実行します。

```bash
pnpm run tauri build
```

実行ファイルは `src-tauri/target/release/bundle` に生成されます。
