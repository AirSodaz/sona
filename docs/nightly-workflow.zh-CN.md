# Nightly 预发布构建工作流 (Nightly Release Workflow)

Nightly Release 工作流用于每天自动构建、签名、打包并发布 Sona 的每日预发布版本（pre-release）。同时，它还会生成并更新适用于 Tauri 自动更新器的升级元数据（`updater.json`）。

工作流配置文件: [.github/workflows/nightly.yml](../.github/workflows/nightly.yml)

---

## 触发条件

1. **定时触发 (Cron)**: 每天在 `18:00 UTC`（北京时间 凌晨 2:00 / 太平洋时间 上午 10:00）自动运行。
2. **手动触发 (`workflow_dispatch`)**: 可以从 GitHub Actions 标签页手动运行。
   - **输入参数**:
     - `force` (布尔值, 默认: `false`): 强制进行构建和发布，即使自上次 nightly 发布以来没有新的提交。

---

## 并发控制

该工作流在并发组 `nightly-release` 下运行，并设置了 `cancel-in-progress: true`。如果在前一次运行尚未结束时触发了新的运行，则会取消前一次运行以节省资源。

---

## 工作流任务 (Workflow Jobs)

### 1. `check-commits`
* **运行环境**: `ubuntu-latest`
* **目的**: 避免在代码库未发生变更时进行冗余构建。
* **流程**:
  1. 检查当前提交的 SHA (`github.sha`)。
  2. 使用 `git ls-remote` 获取现有 `nightly` 标签的提交 SHA。
  3. 如果当前 SHA 与已有的 nightly 标签 SHA 相同（且未指定 `force` 为 `true`），则工作流提前结束，并输出 `should_build=false`。

### 2. `prepare`
* **运行环境**: `ubuntu-latest`
* **目的**: 计算 nightly 版本的标识符。
* **流程**:
  1. 从 `platforms/desktop/frontend/package.json` 中读取基础版本号（Base Version）。
  2. 生成 nightly 版本号字符串：`${base_version}-nightly.${github.run_number}`。
  3. 计算 UTC 时间下的构建日期，格式为 `YYYY-MM-DD`。

### 3. `android-client`
* **调用可复用工作流**: [.github/workflows/android-client.yml](../.github/workflows/android-client.yml)（`ubuntu-22.04`）
* **目的**: 编译并验证 `arm64-v8a` 与 `x86_64` 架构的 Android 客户端 APK。
* **关键步骤**:
  1. 配置 Java 17、Android SDK（`platforms;android-37.0`、`ndk;29.0.14206865`）、Rust Android 目标（`aarch64-linux-android`、`x86_64-linux-android`）、`sccache` 与 `sherpa-onnx` 库。
  2. 传入 `SONA_ANDROID_CHANNEL=nightly`、`SONA_ANDROID_VERSION_NAME=${{ needs.prepare.outputs.version }}` 与 `SONA_ANDROID_VERSION_CODE=${{ github.run_number }}` 执行 `pnpm run verify:android-client`。
  3. 上传 `app-arm64-v8a-debug.apk`、`app-x86_64-debug.apk`、`app-arm64-v8a-release-unsigned.apk` 与 `app-x86_64-release-unsigned.apk`。

### 4. `build-tauri`
* **调用可复用工作流**: [.github/workflows/desktop-client.yml](../.github/workflows/desktop-client.yml)（`macos-latest`, `ubuntu-22.04`, `windows-latest`）
* **目的**: 编译 Sona 桌面端应用程序、校验打包产物、对安装包进行数字签名并生成签名校验文件。
* **构建矩阵目标**:
  - **macOS**: `macos-latest`（`aarch64-apple-darwin` 与 `x86_64-apple-darwin`，启用 `--features llama-metal`）。
  - **Linux**: `ubuntu-22.04`（`x86_64-unknown-linux-gnu`，启用 `--features llama-vulkan`，构建 `.deb` 与 `.AppImage`；预发布版本由 `patch-channel.js` 自动排除 `.rpm` 格式）。
  - **Windows**: `windows-latest`（`x86_64-pc-windows-msvc` 启用 `--features llama-vulkan`，以及纯 CPU 版本的 `aarch64-pc-windows-msvc`；预发布版本由 `patch-channel.js` 自动排除 `.msi`）。
* **关键步骤**:
  1. **系统依赖与工具链**: 安装各平台所需依赖（启用 `llama-vulkan` 时安装 Vulkan SDK，Linux 安装 GTK/WebKit2GTK 与 `clang`/`mold`）、Rust 工具链、`sccache`、Node.js/pnpm 及 Windows 构建环境。
  2. **渠道与版本修改**: 调用 `node platforms/desktop/scripts/patch-channel.js --channel nightly --version <version>` 更新 Tauri 配置、Cargo workspace 版本号与 Nightly 图标。
  3. **准备原生库**: 通过 `setup-sherpa-onnx` 与 `setup-llama-cpp-prebuilt` 为各目标平台拉取预编译共享库。
  4. **Tauri 构建与校验**: 运行 `node platforms/desktop/scripts/tauri.js build` 编译并签名，随后执行 `verify-tauri-bundle.js` 校验安装包与动态库完整性。
  5. **重命名与上传产物**: 为 macOS `.app.tar.gz` / `.sig` 产物附加架构标识（`aarch64` 或 `x64`）后上传工作流构件。

### 5. `publish-nightly`
* **运行环境**: `ubuntu-latest`
* **目的**: 校验产物、更新 Git 标签、生成桌面端与 Android 自动更新元数据，并发布 Release。
* **关键步骤**:
  1. **下载并校验构件**: 下载 `build-tauri` 与 `android-client` 上传的所有产物，并校验 4 个 Android APK 均已产出。
  2. **生成变更日志 (Changelog)**: 生成自上一个 nightly 标签 SHA 到当前提交之间的 Git 提交日志。
  3. **移动 Tag**: 通过 GitHub API 将 `nightly` Git 标签指向当前的提交。
  4. **构建更新清单**: 读取桌面端签名文件（`.sig`）生成 `updater.json`，并运行 `scripts/android-update-manifest.js generate --channel nightly` 生成 `android-update.json`。
  5. **创建或更新 Release**: 使用 `ncipollo/release-action` 更新 GitHub 上现有的 `nightly` 预发布版本，上传桌面端与 Android 安装包以及 `updater.json`、`android-update.json`。

---

## 依赖的 GitHub Secrets 密钥

要使此工作流顺利运行，必须在 GitHub 仓库的 Secrets 中配置以下凭据：

1. **`TAURI_SIGNING_PRIVATE_KEY`**: 由 Tauri CLI 生成的私钥，用于对应用程序安装包进行签名。
2. **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`**: 用于保护 Tauri 签名私钥的密码短语。
3. **`github.token` / `GH_TOKEN`**: GitHub Actions 自动提供的内置 Token，用于执行 API 请求（如移动标签、读取文件）和上传发布资源。
