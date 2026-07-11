# Nightly 预发布构建工作流 (Nightly Release Workflow)

Nightly Release 工作流用于每天自动构建、签名、打包并发布 Sona 的每日预发布版本（pre-release）。同时，它还会生成并更新适用于 Tauri 自动更新器的升级元数据（`updater.json`）。

工作流配置文件: [.github/workflows/nightly.yml](file:///c:/Users/asoda/projects/sona/.github/workflows/nightly.yml)

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
  2. 生成 nightly 版本号字符串：`${base_version}-${github.run_number}`。
  3. 计算 UTC 时间下的构建日期，格式为 `YYYY-MM-DD`。

### 3. `build-tauri`
* **运行环境**: 矩阵平台（`macos-latest`, `ubuntu-22.04`, `windows-latest`）
* **目的**: 编译 Sona 应用程序、对安装包进行数字签名，并生成签名校验文件。
* **构建矩阵目标**:
  - **macOS**: `macos-latest`（构建 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 目标）。
  - **Linux**: `ubuntu-22.04`（默认目标，打包为 `.deb`、`.rpm`、`.AppImage`）。
  - **Windows**: `windows-latest`（默认 x64 目标和 `aarch64-pc-windows-msvc` ARM64 目标）。
* **关键步骤**:
  1. **系统依赖**: 安装目标平台所需的系统级依赖（例如 Linux 下的 GTK/Webkit2gtk）。
  2. **Rust & Node 环境配置**: 配置 Rust 工具链、启用 `corepack`，并通过 `pnpm` 安装依赖。
  3. **渠道与版本修改**: 调用 `node platforms/desktop/scripts/patch-channel.js --channel nightly --version <version>` 来更新 Tauri 配置文件中的渠道和版本号。
  4. **下载 Sherpa-Onnx 库**: 针对每个目标平台/架构，下载并解压匹配的 `sherpa-onnx` 动态链接库。
  5. **Tauri 构建**: 运行 `node platforms/desktop/scripts/tauri.js build` 构建应用。使用配置的签名密钥对安装包进行签名。
  6. **重命名构建产物**: 重命名 macOS 的构建产物，在文件名中附加架构标识（`aarch64` 或 `x64`），以防在发布上传时发生冲突。
  7. **上传构建产物**: 将生成的安装包和升级签名文件（`.sig` 文件）作为工作流构件（Artifacts）上传。

### 4. `publish-nightly`
* **运行环境**: `ubuntu-latest`
* **目的**: 更新 Git 标签、生成自动更新元数据，并发布 Release。
* **关键步骤**:
  1. **下载构件**: 下载 `build-tauri` 任务中上传的所有构建产物。
  2. **生成变更日志 (Changelog)**: 生成自上一个 nightly 标签 SHA 到当前提交之间的 Git 提交日志。
  3. **移动 Tag**: 通过 GitHub API 将 `nightly` Git 标签指向当前的提交。
  4. **构建 `updater.json`**: 读取下载的签名文件（`.sig` 文件），构建 Tauri 格式的 `updater.json`，其中包含所有新上传的 nightly 构件下载地址及其对应的签名信息。
  5. **创建或更新 Release**: 使用 `ncipollo/release-action` 更新 GitHub 上现有的 `nightly` 预发布（pre-release）版本，上传所有新编译的安装包和生成的 `updater.json`。

---

## 依赖的 GitHub Secrets 密钥

要使此工作流顺利运行，必须在 GitHub 仓库的 Secrets 中配置以下凭据：

1. **`TAURI_SIGNING_PRIVATE_KEY`**: 由 Tauri CLI 生成的私钥，用于对应用程序安装包进行签名。
2. **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`**: 用于保护 Tauri 签名私钥的密码短语。
3. **`github.token` / `GH_TOKEN`**: GitHub Actions 自动提供的内置 Token，用于执行 API 请求（如移动标签、读取文件）和上传发布资源。
