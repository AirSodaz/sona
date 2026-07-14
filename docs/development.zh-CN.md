# Sona 开发指南

[English](development.md) | [简体中文](development.zh-CN.md) | [项目 README](../README.zh-CN.md) | [参与贡献](../CONTRIBUTING.md)

本指南介绍本地环境、开发、测试和源码构建。命令用法请查看 [CLI 指南](cli.zh-CN.md)，Android 专用构建说明请查看 [Android 指南](../platforms/android/README.md)。

## 前置条件

- Node.js 20 或更高版本
- Corepack，以及仓库锁定版本的 pnpm
- Rust stable 工具链
- Tauri 在当前平台所需的系统依赖

在 Ubuntu 或 Debian 上，可通过以下命令安装桌面端系统依赖：

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

## 安装

```bash
git clone https://github.com/AirSodaz/sona.git
cd sona
corepack enable
pnpm install
```

## 开发

通过仓库的 Tauri 包装器运行桌面应用：

```bash
pnpm run tauri dev
```

不需要原生宿主时，可以只运行前端开发服务器：

```bash
pnpm run dev
```

## 测试与检查

请选择能覆盖改动范围的最小命令：

```bash
pnpm test
pnpm run test:scripts
pnpm run lint:ci
pnpm run build:ci
```

Rust 包应使用聚焦的 Cargo package 或 test selector 进行测试。贡献者相关的验证要求请查看 [CONTRIBUTING.md](../CONTRIBUTING.md)。

## 构建桌面应用

```bash
pnpm run tauri build
```

桌面安装包会根据构建目标生成在 `target/release/bundle` 或 `target/<triple>/release/bundle` 目录中。

## 构建 CLI

```bash
pnpm run build:sona-cli
cargo run -p sona-cli -- --help
```

release 二进制会生成在 `target/release` 或 `target/<triple>/release`。打包后的桌面应用包含同平台的独立 `sona-cli` 资源；桌面可执行文件本身不解析 CLI 子命令。
