# Developing Sona

[English](development.md) | [简体中文](development.zh-CN.md) | [Project README](../README.md) | [Contributing](../CONTRIBUTING.md)

This guide covers local setup, development, testing, and source builds. See the [CLI guide](cli.md) for command usage and the [Android guide](../platforms/android/README.md) for Android-specific builds.

## Prerequisites

- Node.js 20 or later
- Corepack with the repository-pinned pnpm version
- The stable Rust toolchain
- CMake and a C/C++ compiler for the llama.cpp local ASR adapter (the Clang toolchain is recommended, e.g. `clang`/`clang++` on Linux and `clang-cl` + Ninja on Windows). Desktop and CLI builds dynamically link its generated ggml and llama libraries; release bundles stage those libraries beside the host runtime.
- Optional (GPU builds): for Windows/Linux Vulkan builds, the LunarG Vulkan SDK (providing `glslc` and `SPIRV-Headers`); for macOS, the built-in Metal framework from Xcode Command Line Tools is used automatically. Local development builds stay CPU-only by default; bundled release builds enable `llama-vulkan` on Windows x64 and Linux (env `LLAMA_ENABLE_VULKAN=1`) and `llama-metal` on macOS (env `LLAMA_ENABLE_METAL=1`).
- System graphics and font libraries for GPUI (Vulkan, Direct3D 11/12, or Metal depending on OS)

On Ubuntu or Debian, install the desktop system dependencies with:

```bash
sudo apt-get update
sudo apt-get install \
    build-essential \
    clang \
    llvm-dev \
    libclang-dev \
    cmake \
    curl \
    wget \
    file \
    libssl-dev \
    libfontconfig1-dev \
    libfreetype6-dev \
    libasound2-dev \
    libxkbcommon-x11-dev \
    libvulkan-dev
```

## Install

```bash
git clone https://github.com/AirSodaz/sona.git
cd sona
corepack enable
pnpm install
```

## Develop

Run the pure Rust GPUI desktop application:

```bash
cargo run -p sona
# or using pnpm script alias
pnpm run dev
```

## Test and Validate

Pick the narrowest commands that cover your changes:

```bash
pnpm test
pnpm run test:scripts
pnpm run verify:android-client
pnpm run verify:android-uniffi
```

If you modified cross-platform contracts exposed to UniFFI or Android, run `pnpm run generate:sona-context` to regenerate the composition root.
Rust crates should be tested with focused Cargo package or test selectors. See [CONTRIBUTING.md](../CONTRIBUTING.md) for contributor verification expectations.

## Build The Desktop Application

```bash
cargo build -p sona --release
# or using pnpm script alias
pnpm run build
```

The release binary is written to `target/release/sona` (or `target/release/sona.exe` on Windows).

## Build The CLI

```bash
pnpm run build:sona-cli
cargo run -p sona-cli -- --help
```

The release binary is written under `target/release` or `target/<triple>/release`. Packaged desktop builds include the matching standalone `sona-cli` resource; the desktop executable itself does not parse CLI subcommands.

## Build The MCP Server

```bash
pnpm run build:sona-mcp
cargo run -p sona-mcp -- --help
```

The release binary is written under `target/release/sona-mcp` or `target/<triple>/release/sona-mcp`.

## Build Channels & Metadata

Sona distinguishes four build channels across Desktop, CLI, and Android: `dev`, `ci`, `nightly`, and `stable`. Compile-time SemVer 2.0 build metadata is resolved from git state via `scripts/get-build-info.mjs`. Development and CI builds guard automatic update checks while displaying channel badges in Desktop Settings > About.
