# Sona 开发指南

[English](development.md) | [简体中文](development.zh-CN.md) | [项目 README](../README.zh-CN.md) | [参与贡献](../CONTRIBUTING.md)

本指南介绍本地环境、开发、测试和源码构建。命令用法请查看 [CLI 指南 (English)](cli.md)，Android 专用构建说明请查看 [Android 指南](../platforms/android/README.md)。

## 前置条件

- Node.js 20 或更高版本
- Corepack，以及仓库锁定版本的 pnpm
- Rust stable 工具链
- 用于构建 llama.cpp 本地 ASR 适配器的 CMake 与 C/C++ 编译器（推荐 Clang 工具链，如 Linux 下 `clang`/`clang++`、Windows 下 `clang-cl` + Ninja）。桌面端与 CLI 构建会动态链接生成的 ggml 与 llama 共享库，发布安装包会将这些库暂存到宿主运行时目录。
- 可选（GPU 加速构建）：Windows / Linux Vulkan 构建需要 LunarG Vulkan SDK（提供 `glslc` 与 `SPIRV-Headers`）；macOS 自动使用 Xcode Command Line Tools 内置的 Metal 工具链。本地开发构建默认采用纯 CPU 模式；发布的桌面安装包中，Windows x64 和 Linux 启用 `llama-vulkan` 特性（由环境变量 `LLAMA_ENABLE_VULKAN=1` 驱动打包），macOS 启用 `llama-metal` 特性（由环境变量 `LLAMA_ENABLE_METAL=1` 驱动打包）。
- GPUI 依赖的系统图形与字体库（根据系统不同为 Vulkan、Direct3D 11/12 或 Metal）

在 Ubuntu 或 Debian 上，可通过以下命令安装桌面端系统依赖：

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

## 安装

```bash
git clone https://github.com/AirSodaz/sona.git
cd sona
corepack enable
pnpm install
```

## 开发

运行纯 Rust GPUI 桌面应用：

```bash
cargo run -p sona
# 或使用 pnpm 脚本别名
pnpm run dev
```

## 测试与检查

请选择能覆盖改动范围的最小命令：

```bash
pnpm test
pnpm run test:scripts
pnpm run verify:android-client
pnpm run verify:android-uniffi
```

如果修改了面向 UniFFI 或移动端的接口契约，请运行 `pnpm run generate:sona-context` 重新生成组合根代码。
Rust 包应使用聚焦的 Cargo package 或 test selector 进行测试。贡献者相关的验证要求请查看 [CONTRIBUTING.md](../CONTRIBUTING.md)。

## 构建桌面应用

```bash
cargo build -p sona --release
# 或使用 pnpm 脚本别名
pnpm run build
```

Release 二进制文件会输出在 `target/release/sona`（Windows 下为 `target/release/sona.exe`）。

## 构建 CLI

```bash
pnpm run build:sona-cli
cargo run -p sona-cli -- --help
```

release 二进制会生成在 `target/release` 或 `target/<triple>/release`。打包后的桌面应用包含同平台的独立 `sona-cli` 资源；桌面可执行文件本身不解析 CLI 子命令。

## 构建 MCP 服务端

```bash
pnpm run build:sona-mcp
cargo run -p sona-mcp -- --help
```

Release 二进制文件会输出在 `target/release/sona-mcp` 或 `target/<triple>/release/sona-mcp`。

## 构建渠道与版本元数据

Sona 在桌面端、CLI 与 Android 端统一区分 4 种构建渠道：`dev`、`ci`、`nightly` 和 `stable`。编译期 SemVer 2.0 版本元数据由 `scripts/get-build-info.mjs` 基于 Git 状态解析。开发与 CI 构建会自动阻断自动更新检查，并在桌面端 `设置 > 关于` 中显示专属渠道徽标。
