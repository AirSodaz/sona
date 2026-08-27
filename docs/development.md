# Developing Sona

[English](development.md) | [简体中文](development.zh-CN.md) | [Project README](../README.md) | [Contributing](../CONTRIBUTING.md)

This guide covers local setup, development, testing, and source builds. See the [CLI guide](cli.md) for command usage and the [Android guide](../platforms/android/README.md) for Android-specific builds.

## Prerequisites

- Node.js 20 or later
- Corepack with the repository-pinned pnpm version
- The stable Rust toolchain
- CMake and a C/C++ compiler for the llama.cpp local ASR adapter (the Clang toolchain is recommended, e.g. `clang`/`clang++` on Linux and `clang-cl` + Ninja on Windows). Desktop and CLI builds dynamically link its generated ggml and llama libraries; release bundles stage those libraries beside the host runtime. Build llama.cpp from a recent master — the Granite Speech GGUF requires b9045 or newer (verified against b10615); older local builds silently degrade Granite transcription quality.
- Optional (GPU builds): for Windows/Linux Vulkan builds, the LunarG Vulkan SDK (providing `glslc` and `SPIRV-Headers`); for macOS, the built-in Metal framework from Xcode Command Line Tools is used automatically. Local development builds stay CPU-only by default; bundled release builds enable `llama-vulkan` on Windows x64 and Linux (env `LLAMA_ENABLE_VULKAN=1`) and `llama-metal` on macOS (env `LLAMA_ENABLE_METAL=1`).
- The platform dependencies required by Tauri

On Ubuntu or Debian, install the desktop system dependencies with:

```bash
sudo apt-get update
sudo apt-get install libwebkit2gtk-4.1-dev \
    build-essential \
    clang \
    llvm-dev \
    libclang-dev \
    cmake \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libasound2-dev

## Install

```bash
git clone https://github.com/AirSodaz/sona.git
cd sona
corepack enable
pnpm install
```

## Develop

Run the desktop application through the repository Tauri wrapper:

```bash
pnpm run tauri dev
```

Run only the frontend development server when the native host is not needed:

```bash
pnpm run dev
```

## Test And Check

Use the smallest command that covers the area you changed:

```bash
pnpm test
pnpm run test:scripts
pnpm run lint:ci
pnpm run build:ci
```

Rust packages should be tested with focused Cargo package or test selectors. Contributor-specific validation guidance lives in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Build The Desktop Application

```bash
pnpm run tauri build
```

Desktop bundles are written under `target/release/bundle` or `target/<triple>/release/bundle`, depending on the build target.

## Build The CLI

```bash
pnpm run build:sona-cli
cargo run -p sona-cli -- --help
```

The release binary is written under `target/release` or `target/<triple>/release`. Packaged desktop builds include the matching standalone `sona-cli` resource; the desktop executable itself does not parse CLI subcommands.
