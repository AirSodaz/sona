# Nightly Release Workflow

The Nightly Release workflow automatically builds, signs, packages, and publishes pre-release builds of Sona every day. It also generates and updates the update metadata (`updater.json`) for the Tauri auto-updater.

Workflow File: [.github/workflows/nightly.yml](../.github/workflows/nightly.yml)

---

## Triggers

1. **Schedule (Cron)**: Runs automatically every day at `18:00 UTC` (2:00 AM Beijing Time / 10:00 AM PST).
2. **Manual Dispatch (`workflow_dispatch`)**: Can be manually triggered from the GitHub Actions tab.
   - **Inputs**:
     - `force` (boolean, default: `false`): Force a build and release even if there are no new commits since the last nightly release.

---

## Concurrency

The workflow runs under the concurrency group `nightly-release` with `cancel-in-progress: true`. If a new run is triggered while a previous run is still active, the previous run is cancelled to save resources.

---

## Workflow Jobs

### 1. `check-commits`
* **Runs on**: `ubuntu-latest`
* **Purpose**: Avoid redundant builds if the codebase hasn't changed.
* **Flow**:
  1. Checks the current commit SHA (`github.sha`).
  2. Resolves the commit SHA of the existing `nightly` tag using `git ls-remote`.
  3. If the current SHA is the same as the nightly tag SHA (and `force` is not set to `true`), the workflow exits early with `should_build=false`.

### 2. `prepare`
* **Runs on**: `ubuntu-latest`
* **Purpose**: Compute nightly version identifiers.
* **Flow**:
  1. Reads the base version from `platforms/desktop/frontend/package.json`.
  2. Generates the nightly version string: `${base_version}-nightly.${github.run_number}`.
  3. Computes the build date in `YYYY-MM-DD` format (UTC).

### 3. `android-client`
* **Calls**: [.github/workflows/android-client.yml](../.github/workflows/android-client.yml) (`ubuntu-22.04`)
* **Purpose**: Compiles and verifies the Android client APKs for `arm64-v8a` and `x86_64`.
* **Key Steps**:
  1. Sets up Java 17, Android SDK (`platforms;android-37.0`, `ndk;29.0.14206865`), Rust Android targets (`aarch64-linux-android`, `x86_64-linux-android`), `sccache`, and `sherpa-onnx` libraries.
  2. Runs `pnpm run verify:android-client` with `SONA_ANDROID_CHANNEL=nightly`, `SONA_ANDROID_VERSION_NAME=${{ needs.prepare.outputs.version }}`, and `SONA_ANDROID_VERSION_CODE=${{ github.run_number }}`.
  3. Uploads `app-arm64-v8a-debug.apk`, `app-x86_64-debug.apk`, `app-arm64-v8a-release-unsigned.apk`, and `app-x86_64-release-unsigned.apk`.

### 4. `build-tauri`
* **Calls**: [.github/workflows/desktop-client.yml](../.github/workflows/desktop-client.yml) (`macos-latest`, `ubuntu-22.04`, `windows-latest`)
* **Purpose**: Compiles the Sona desktop application, verifies bundles, signs installers, and generates update signatures.
* **Matrix Targets**:
  - **macOS**: `macos-latest` (`aarch64-apple-darwin` and `x86_64-apple-darwin` with `--features llama-metal`).
  - **Linux**: `ubuntu-22.04` (`x86_64-unknown-linux-gnu` with `--features llama-vulkan`, compiling `.deb` and `.AppImage`; `.rpm` is excluded for pre-release versions by `patch-channel.js`).
  - **Windows**: `windows-latest` (`x86_64-pc-windows-msvc` with `--features llama-vulkan`, and CPU-only `aarch64-pc-windows-msvc`; `.msi` is excluded for pre-release versions by `patch-channel.js`).
* **Key Steps**:
  1. **Dependencies & Toolchains**: Installs native dependencies (Vulkan SDK when `llama-vulkan` is enabled, GTK/WebKit2GTK + `clang`/`mold` on Linux), Rust toolchain, `sccache`, Node.js/pnpm, and Windows MSVC build environment.
  2. **Channel Patching**: Invokes `node platforms/desktop/scripts/patch-channel.js --channel nightly --version <version>` to update Tauri configuration, Cargo workspace version, and nightly icons.
  3. **Native Libs**: Sets up prebuilt `sherpa-onnx` (`setup-sherpa-onnx`) and `llama.cpp` (`setup-llama-cpp-prebuilt`) libraries for each target platform.
  4. **Tauri Build & Verify**: Builds Sona via `node platforms/desktop/scripts/tauri.js build`, signs updater bundles, and runs `verify-tauri-bundle.js`.
  5. **Asset Renaming & Upload**: Renames macOS `.app.tar.gz` / `.sig` artifacts with architecture suffixes (`aarch64` or `x64`) and uploads release artifacts.

### 5. `publish-nightly`
* **Runs on**: `ubuntu-latest`
* **Purpose**: Verify artifacts, update the git tag, generate desktop and Android update metadata, and publish the release.
* **Key Steps**:
  1. **Download & Verify Artifacts**: Pulls all files built by `build-tauri` and `android-client`, verifying all four expected Android APKs are present.
  2. **Changelog Generation**: Generates a git commit log between the previous nightly tag SHA and the current commit.
  3. **Move Tag**: Updates the `nightly` Git tag via the GitHub API to point to the current commit.
  4. **Build Update Manifests**: Reads the desktop `.sig` files to construct `updater.json`, and invokes `scripts/android-update-manifest.js generate --channel nightly` to construct `android-update.json`.
  5. **Create/Update Release**: Uses `ncipollo/release-action` to update the existing pre-release named `nightly` with the new desktop/Android binaries, `updater.json`, and `android-update.json`.

---

## Required Secrets

The following credentials must be set in GitHub repository secrets for the workflow to complete successfully:

1. **`TAURI_SIGNING_PRIVATE_KEY`**: The private key generated by Tauri CLI, used to sign the application bundles.
2. **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`**: The passphrase protecting the Tauri signing private key.
3. **`github.token` / `GH_TOKEN`**: The built-in GitHub Actions token, used to perform API calls (moving tags, reading files) and uploading release assets.
