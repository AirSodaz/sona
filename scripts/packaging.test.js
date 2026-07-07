import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const node = process.execPath;

function makeTempRepo() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-packaging-'));
  fs.mkdirSync(path.join(root, 'src-tauri', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'src-tauri', 'resources', 'cli'), { recursive: true });
  fs.mkdirSync(path.join(root, 'src-tauri', 'resources', 'shared_libs'), { recursive: true });
  return root;
}

function writeTauriConfig(root) {
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'tauri.conf.json'),
    JSON.stringify(
      {
        bundle: {
          resources: ['resources/shared_libs/*', 'resources/cli/*'],
          externalBin: ['binaries/ffmpeg'],
        },
      },
      null,
      2,
    ),
  );
}

function rustFilesUnder(root) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      return rustFilesUnder(fullPath);
    }
    return entry.isFile() && entry.name.endsWith('.rs') ? [fullPath] : [];
  });
}

test('tauri bundle verification requires ffmpeg sidecar and shared libraries', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  fs.mkdirSync(path.join(root, 'src-tauri', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'src-tauri', 'resources', 'shared_libs'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'binaries', 'ffmpeg-x86_64-pc-windows-msvc.exe'),
    'ffmpeg',
  );
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'resources', 'shared_libs', 'sherpa-onnx-c-api.dll'),
    'sherpa',
  );
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'resources', 'shared_libs', 'onnxruntime.dll'),
    'onnxruntime',
  );
  fs.writeFileSync(path.join(root, 'src-tauri', 'resources', 'cli', 'sona-cli.exe'), 'cli');
  writeTauriConfig(root);
  const bundleRoot = path.join(root, 'target', target, 'release', 'bundle', 'nsis');
  fs.mkdirSync(bundleRoot, { recursive: true });
  fs.writeFileSync(path.join(bundleRoot, 'Sona_0.8.0_x64-setup.exe'), 'installer');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--bundle-root',
      bundleRoot,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified packaged ffmpeg sidecar/);
  assert.match(result.stdout, /Verified standalone CLI resource/);
  assert.match(result.stdout, /Verified shared libraries/);
});

test('desktop tauri crate no longer bundles sona-cli sidecar artifacts', () => {
  const libRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const cargoToml = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const tauriConfig = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'tauri.conf.json'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const tauriScript = fs.readFileSync(path.join(repoRoot, 'scripts', 'tauri.js'), 'utf8');
  const oldCliSidecarScript = ['prepare', 'cli', 'sidecar'].join('-');
  const oldCliBundleScript = ['verify', 'cli', 'bundle'].join('-');

  assert.doesNotMatch(libRs, /\bmod cli;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'cli')), false);
  assert.doesNotMatch(cargoToml, /^clap\s*=/mu);
  assert.doesNotMatch(cargoToml, /^clap_complete\s*=/mu);
  assert.doesNotMatch(tauriConfig, /binaries\/sona-cli/u);
  assert.doesNotMatch(tauriScript, new RegExp(oldCliSidecarScript, 'u'));
  assert.doesNotMatch(prWorkflow, new RegExp(`${oldCliSidecarScript}|${oldCliBundleScript}`, 'u'));

  const desktopCliCoreReferences = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /sona_core::cli_|OfflineTranscribeCliOptions/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopCliCoreReferences, []);
});

test('standalone CLI resource staging copies the sona-cli binary into the desktop bundle resources', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  const releaseDir = path.join(root, 'target', target, 'release');
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, 'sona-cli.exe'), 'cli');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'setup-sona-cli-resource.js'),
      '--repo-root',
      root,
      '--target',
      target,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Staged standalone CLI resource/);

  const resourceDir = path.join(root, 'src-tauri', 'resources', 'cli');
  assert.equal(fs.existsSync(path.join(resourceDir, 'sona-cli.exe')), true);
  assert.equal(fs.existsSync(path.join(resourceDir, 'sherpa-onnx-c-api.dll')), false);
  assert.equal(fs.existsSync(path.join(resourceDir, 'onnxruntime.dll')), false);
});

test('release workflows stage standalone CLI into the same-platform desktop installer resources', () => {
  assert.equal(fs.existsSync(path.join(repoRoot, 'scripts', 'package-sona-cli.js')), false);

  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const workflow = fs.readFileSync(path.join(repoRoot, '.github', 'workflows', workflowName), 'utf8');

    assert.match(workflow, /cargo build -p sona-cli --release \$\{\{ matrix\.args \}\}/u);
    assert.match(workflow, /node scripts\/setup-sona-cli-resource\.js \$\{\{ matrix\.args \}\}/u);
    assert.match(workflow, /matrix\.args != '--target universal-apple-darwin'/u);
    assert.doesNotMatch(workflow, /node scripts\/package-sona-cli\.js/u);
    assert.doesNotMatch(workflow, /target\/\*\*\/release\/sona-cli-\*\.tar\.gz/u);
  }
});

test('core crate does not keep sona-cli config template surface', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');

  assert.doesNotMatch(coreLib, /^pub mod cli_config;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'src', 'cli_config.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'tests', 'cli_config.rs')), false);
});

test('core crate exposes serve runtime without cli runtime surface', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');

  assert.match(coreLib, /^pub mod serve_runtime;/mu);
  assert.doesNotMatch(coreLib, /^pub mod cli_runtime;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'src', 'cli_runtime.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'tests', 'cli_runtime.rs')), false);
});

test('desktop api server invokes local batch ASR through the core transcriber port', () => {
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const apiServer = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'server.rs'), 'utf8');

  assert.match(tauriCargo, /^sona-local-asr\s*=\s*\{ path = "\.\.\/adapters\/local_asr" \}/mu);
  assert.match(apiServer, /use sona_core::ports::asr::\{[\s\S]*BatchTranscriber/u);
  assert.match(apiServer, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.match(apiServer, /\.transcribe\(plan\)/u);
  assert.doesNotMatch(apiServer, /run_offline_transcription/u);
  assert.doesNotMatch(apiServer, /use crate::integrations::asr::transcribe_batch_with_progress;/u);
  assert.doesNotMatch(apiServer, /LocalSherpaAdapter::offline_plan_to_batch_request/u);
});

test('pr guardrails run adapter tests with core bindings and standalone CLI', () => {
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.match(
    prWorkflow,
    /cargo test -p sona-core -p sona-local-asr -p sona-online-llm -p sona-online-asr -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
  );
});

test('core ASR request contract is exposed through TS and UniFFI binding crates', () => {
  const coreCargo = fs.readFileSync(path.join(repoRoot, 'core', 'Cargo.toml'), 'utf8');
  const tsBindLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'ts_bind', 'src', 'lib.rs'), 'utf8');
  const uniffiLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'lib.rs'), 'utf8');
  const uniffiMapper = fs.readFileSync(path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper.rs'), 'utf8');

  assert.match(coreCargo, /specta = \{[^}]*features = \["derive", "serde_json"\]/u);

  for (const typeName of [
    'AsrTranscriptionRequest',
    'AsrEngineConfig',
    'OnlineAsrProviderRequest',
    'VolcengineDoubaoAsrConfig',
    'TranscriptPostprocessOptions',
    'SpeakerProcessingConfig',
    'ModelFileConfig',
  ]) {
    assert.match(tsBindLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(uniffiLib, /default_batch_segmentation_mode/u);
  assert.match(uniffiLib, /online_asr_provider_request/u);
  assert.match(uniffiLib, /volcengine_doubao_asr_config_from_json/u);
  assert.match(uniffiMapper, /pub enum FfiAsrEngine/u);
  assert.match(uniffiMapper, /pub enum FfiAsrMode/u);
  assert.match(uniffiMapper, /pub enum FfiBatchSegmentationMode/u);
  assert.match(uniffiMapper, /pub struct FfiOnlineAsrProviderRequest/u);
  assert.match(uniffiMapper, /pub struct FfiVolcengineDoubaoAsrConfig/u);
});

test('online ASR provider manifest is owned by core and used directly by desktop', () => {
  const coreAsr = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'ports', 'asr.rs'), 'utf8');
  const coreManifestPath = path.join(repoRoot, 'core', 'src', 'ports', 'online-asr-providers.json');
  const legacySharedManifestPath = path.join(repoRoot, 'src', 'shared', 'online-asr-providers.json');
  const desktopIntegrations = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'mod.rs'), 'utf8');
  const apiServer = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'server.rs'), 'utf8');
  const streamingRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'streaming.rs'), 'utf8');
  const onlineAdapterRs = ['groq.rs', 'mistral.rs', 'volcengine.rs']
    .map((file) => fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', file), 'utf8'))
    .join('\n');
  const tsBindLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'ts_bind', 'src', 'lib.rs'), 'utf8');
  const onlineProvidersTs = fs.readFileSync(path.join(repoRoot, 'src', 'services', 'onlineAsrProviders.ts'), 'utf8');
  const asrConfigServiceTest = fs.readFileSync(
    path.join(repoRoot, 'src', 'services', '__tests__', 'asrConfigService.test.ts'),
    'utf8',
  );

  assert.ok(fs.existsSync(coreManifestPath));
  assert.equal(fs.existsSync(legacySharedManifestPath), false);
  assert.match(coreAsr, /ONLINE_ASR_PROVIDERS_JSON/u);
  assert.match(coreAsr, /include_str!\("online-asr-providers\.json"\)/u);
  assert.doesNotMatch(coreAsr, /src\/shared\/online-asr-providers\.json/u);
  assert.match(coreAsr, /pub const VOLCENGINE_DOUBAO_PROVIDER_ID/u);
  assert.match(coreAsr, /pub fn online_asr_providers\(\)/u);
  assert.match(coreAsr, /pub fn find_online_asr_provider/u);
  assert.match(coreAsr, /pub struct OnlineAsrProvider/u);
  assert.match(coreAsr, /pub struct OnlineAsrBatchCapability/u);
  assert.match(coreAsr, /pub fn provider_id\(&self\) -> &str/u);

  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr_providers.rs')), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod asr_providers;/mu);
  assert.match(apiServer, /sona_core::ports::asr::\{[^}]*find_online_asr_provider[^}]*online_asr_providers/u);
  assert.match(streamingRs, /sona_core::ports::asr::find_online_asr_provider/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::GROQ_WHISPER_PROVIDER_ID/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::MISTRAL_VOXTRAL_PROVIDER_ID/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::VOLCENGINE_DOUBAO_PROVIDER_ID/u);

  assert.match(tsBindLib, /OnlineAsrProvider/u);
  assert.match(tsBindLib, /OnlineAsrCapability/u);
  assert.match(tsBindLib, /OnlineAsrBatchCapability/u);
  assert.match(tsBindLib, /OnlineAsrLocalFileBatchMode/u);
  assert.match(onlineProvidersTs, /\.\.\/\.\.\/core\/src\/ports\/online-asr-providers\.json/u);
  assert.match(asrConfigServiceTest, /\.\.\/\.\.\/\.\.\/core\/src\/ports\/online-asr-providers\.json/u);
});

test('preset model catalog data is owned by core and reused by frontend', () => {
  const corePresetModels = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'preset_models.rs'), 'utf8');
  const corePresetModelsPath = path.join(repoRoot, 'core', 'src', 'preset-models.json');
  const legacySharedPresetModelsPath = path.join(repoRoot, 'src', 'shared', 'preset-models.json');
  const modelServiceTs = fs.readFileSync(path.join(repoRoot, 'src', 'services', 'modelService.ts'), 'utf8');

  assert.ok(fs.existsSync(corePresetModelsPath));
  assert.equal(fs.existsSync(legacySharedPresetModelsPath), false);
  assert.match(corePresetModels, /include_str!\("preset-models\.json"\)/u);
  assert.doesNotMatch(corePresetModels, /src\/shared\/preset-models\.json/u);
  assert.match(modelServiceTs, /\.\.\/\.\.\/core\/src\/preset-models\.json/u);
});

test('desktop platform adapters own Tauri path event diagnostics and preset model bridges', () => {
  const desktopPlatform = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'mod.rs'), 'utf8');
  const platformPaths = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'paths.rs'), 'utf8');
  const platformEvent = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'event.rs'), 'utf8');
  const platformPresetModels = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'preset_models.rs'),
    'utf8',
  );
  const platformDiagnostics = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'diagnostics.rs'),
    'utf8',
  );

  assert.match(desktopPlatform, /^pub mod paths;/mu);
  assert.match(desktopPlatform, /^pub mod event;/mu);
  assert.match(desktopPlatform, /^pub mod preset_models;/mu);
  assert.match(desktopPlatform, /^pub mod diagnostics;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'paths.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'event.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'preset_models.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'diagnostics.rs')), false);
  assert.match(platformPaths, /pub struct TauriPathProvider/u);
  assert.match(platformPaths, /impl<R: Runtime> PathProvider for TauriPathProvider<R>/u);
  assert.match(platformEvent, /pub struct TauriEventEmitter<R: Runtime>\(pub AppHandle<R>\)/u);
  assert.match(platformEvent, /impl<R: Runtime> EventEmitter for TauriEventEmitter<R>/u);
  assert.match(platformPresetModels, /pub use sona_core::preset_models::\*/u);
  assert.match(platformPresetModels, /tauri::async_runtime::spawn_blocking/u);
  assert.match(platformDiagnostics, /pub use sona_core::diagnostics::\{/u);
  assert.match(platformDiagnostics, /crate::platform::paths::\{PathKind, PathProvider\}/u);
});

test('desktop tauri crate imports core crates directly without a local core shim', () => {
  const desktopLib = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const desktopRust = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src'))
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');

  assert.doesNotMatch(desktopLib, /^pub mod core;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core')), false);
  assert.doesNotMatch(desktopRust, /crate::core::/u);
  assert.match(desktopRust, /sona_core::/u);
  assert.match(desktopRust, /sona_sqlite::/u);
});

test('desktop filesystem adapters live in platform rather than repositories', () => {
  const desktopPlatform = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'mod.rs'), 'utf8');
  const platformStorage = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'file_storage.rs'), 'utf8');
  const platformRecovery = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'recovery_repository.rs'),
    'utf8',
  );
  const exportCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'export.rs'), 'utf8');
  const systemCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'system.rs'), 'utf8');

  assert.doesNotMatch(desktopPlatform, /^pub mod export_files;/mu);
  assert.match(desktopPlatform, /^pub mod file_storage;/mu);
  assert.match(desktopPlatform, /^pub mod recovery_repository;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'export_files.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'export.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'storage.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'recovery.rs')), false);
  assert.match(platformStorage, /pub use sona_core::file_utils::\{/u);
  assert.match(platformRecovery, /pub struct FsRecoveryRepository/u);
  assert.match(exportCommand, /use sona_core::export::\{[\s\S]*export_transcript_file as core_export_transcript_file/u);
  assert.match(exportCommand, /core_export_transcript_file\(ExportTranscriptFileRequest/u);
  assert.doesNotMatch(exportCommand, /crate::platform::export_files/u);
  assert.match(systemCommand, /crate::platform::recovery_repository::FsRecoveryRepository/u);
});

test('desktop automation and project repository task adapters live in platform', () => {
  const desktopPlatform = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'mod.rs'), 'utf8');
  const platformAutomation = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'automation_repository.rs'),
    'utf8',
  );
  const platformProject = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'project_repository.rs'),
    'utf8',
  );
  const automationCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'automation.rs'), 'utf8');
  const projectCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'project.rs'), 'utf8');

  assert.match(desktopPlatform, /^pub mod automation_repository;/mu);
  assert.match(desktopPlatform, /^pub mod project_repository;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project')), false);
  assert.match(platformAutomation, /pub fn validate_rule_activation_inner/u);
  assert.match(platformAutomation, /sona_sqlite::automation::SqliteAutomationRepository/u);
  assert.match(platformProject, /sona_sqlite::project::SqliteProjectRepository/u);
  assert.match(automationCommand, /crate::platform::automation_repository::\{/u);
  assert.match(projectCommand, /crate::platform::project_repository::\{/u);
  assert.match(automationCommand, /sona_core::automation::\{/u);
  assert.match(projectCommand, /sona_core::project::\{/u);
});

test('desktop history repository facade lives in platform without repositories module', () => {
  const desktopLib = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const desktopPlatform = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'mod.rs'), 'utf8');
  const platformHistory = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository.rs'),
    'utf8',
  );
  const dashboardApp = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'dashboard.rs'), 'utf8');
  const setupApp = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'setup.rs'), 'utf8');
  const desktopRust = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /crate::repositories|repositories::history/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));
  const historyTypeShimReferences = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /super::types|history::types/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.doesNotMatch(desktopLib, /^pub mod repositories;/mu);
  assert.match(desktopPlatform, /^pub mod history_repository;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories')), false);
  assert.ok(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository', 'llm_helpers.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository', 'state.rs')));
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository', 'types.rs')), false);
  assert.doesNotMatch(platformHistory, /^mod types;/mu);
  assert.deepEqual(historyTypeShimReferences, []);
  assert.deepEqual(desktopRust, []);
  assert.match(platformHistory, /pub use sona_core::history::\{/u);
  assert.match(dashboardApp, /use crate::platform::history_repository::SqliteHistoryStore;/u);
  assert.match(dashboardApp, /use sona_sqlite::analytics::SqliteAnalyticsRepository;/u);
  assert.match(setupApp, /crate::platform::history_repository::SqliteHistoryStore::new/u);
  assert.match(setupApp, /sona_sqlite::analytics::SqliteAnalyticsRepository::new/u);
});

test('app config migration and LLM provider manifest are owned by core', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreConfig = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'config', 'mod.rs'), 'utf8');
  const coreDefaults = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'config', 'defaults.rs'), 'utf8');
  const coreMigration = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'config', 'migration.rs'), 'utf8');
  const desktopSystemCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'system.rs'), 'utf8');
  const desktopServer = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'server.rs'), 'utf8');
  const desktopIntegrations = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'mod.rs'), 'utf8');
  const llmProvidersTs = fs.readFileSync(path.join(repoRoot, 'src', 'services', 'llm', 'providers.ts'), 'utf8');

  assert.match(coreLib, /^pub mod config;/mu);
  assert.match(coreLib, /^pub mod llm_providers;/mu);
  assert.ok(fs.existsSync(path.join(repoRoot, 'core', 'src', 'llm-providers.json')));
  assert.equal(fs.existsSync(path.join(repoRoot, 'src', 'shared', 'llm-providers.json')), false);
  assert.match(coreConfig, /^pub mod defaults;/mu);
  assert.match(coreConfig, /pub fn migrate_app_config/u);
  assert.match(coreDefaults, /crate::ports::asr::online_asr_providers/u);
  assert.match(coreMigration, /crate::llm_providers::find_llm_provider_by_id_or_alias/u);
  assert.match(desktopSystemCommand, /sona_core::config::migrate_app_config/u);
  assert.match(desktopServer, /sona_sqlite::config_store::SqliteConfigStore/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'defaults.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'migration.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'types.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'error.rs')), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod llm_providers;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm_providers.rs')), false);
  assert.match(llmProvidersTs, /\.\.\/\.\.\/\.\.\/core\/src\/llm-providers\.json/u);
});

test('SQLite database handle and schema are owned by sqlite adapter', () => {
  const workspaceCargo = fs.readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8');
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const desktopSetup = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'setup.rs'), 'utf8');
  const sqliteCargoPath = path.join(repoRoot, 'adapters', 'sqlite', 'Cargo.toml');
  const sqliteLibPath = path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs');
  const sqliteMigrationPath = path.join(repoRoot, 'adapters', 'sqlite', 'src', 'legacy_migration.rs');
  const sqliteLib = fs.readFileSync(sqliteLibPath, 'utf8');

  assert.match(workspaceCargo, /"adapters\/sqlite"/u);
  assert.ok(fs.existsSync(sqliteCargoPath));
  assert.ok(fs.existsSync(sqliteLibPath));
  assert.ok(fs.existsSync(sqliteMigrationPath));
  assert.match(sqliteLib, /^pub mod legacy_migration;/mu);
  assert.match(tauriCargo, /sona-sqlite\s*=\s*\{\s*path\s*=\s*"..\/adapters\/sqlite"/u);
  assert.match(desktopSetup, /sona_sqlite::Database::open/u);
  assert.match(desktopSetup, /sona_sqlite::legacy_migration::migrate_legacy_to_sqlite/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database')), false);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'legacy_migration.rs')),
    false,
  );
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'error.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'ports.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'schema.rs')), false);
  assert.doesNotMatch(desktopSetup, /crate::core::database/u);
  assert.doesNotMatch(desktopSetup, /rusqlite::Connection/u);
  assert.doesNotMatch(desktopSetup, /struct ConnectionPool/u);
  assert.doesNotMatch(desktopSetup, /pub struct Database/u);
});

test('SQLite config and task ledger stores are owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopSystemCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'system.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'config_store.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'task_ledger.rs')));
  assert.match(sqliteLib, /^pub mod config_store;/mu);
  assert.match(sqliteLib, /^pub mod task_ledger;/mu);
  assert.match(sqliteLib, /^pub use config_store::SqliteConfigStore;/mu);
  assert.match(sqliteLib, /^pub use task_ledger::SqliteLedgerRepository;/mu);
  assert.match(desktopSystemCommand, /sona_sqlite::config_store::SqliteConfigStore/u);
  assert.match(desktopSystemCommand, /sona_sqlite::task_ledger::SqliteLedgerRepository/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'sqlite_store.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'task_ledger', 'sqlite_repository.rs')), false);
});

test('SQLite automation repository is owned by sqlite adapter', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreAutomationPath = path.join(repoRoot, 'core', 'src', 'automation.rs');
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const platformAutomationRepository = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'automation_repository.rs'),
    'utf8',
  );
  const desktopAutomationCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'automation.rs'), 'utf8');

  assert.ok(fs.existsSync(coreAutomationPath));
  assert.match(coreLib, /^pub mod automation;/mu);
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'automation.rs')));
  assert.match(sqliteLib, /^pub mod automation;/mu);
  assert.match(sqliteLib, /^pub use automation::\{AutomationRepositoryState, SqliteAutomationRepository\};/mu);
  assert.match(platformAutomationRepository, /sona_sqlite::automation::SqliteAutomationRepository/u);
  assert.match(platformAutomationRepository, /validate_rule_activation/u);
  assert.match(desktopAutomationCommand, /sona_core::automation::\{/u);
  assert.match(desktopAutomationCommand, /sona_sqlite::automation::AutomationRepositoryState/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation')), false);
  assert.doesNotMatch(platformAutomationRepository, /is_feature_llm_config_complete/u);
  assert.doesNotMatch(platformAutomationRepository, /fn is_feature_llm_config_complete/u);
  assert.doesNotMatch(platformAutomationRepository, /fn is_batch_asr_configured/u);
  assert.doesNotMatch(platformAutomationRepository, /online_asr_providers/u);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation', 'sqlite_repository.rs')),
    false,
  );
});

test('automation runtime path rules are owned by core and adapted by desktop', () => {
  const coreAutomation = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'automation.rs'), 'utf8');
  const desktopPlatformRuntime = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'automation_runtime.rs'),
    'utf8',
  );
  const desktopLib = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const desktopCommands = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'automation.rs'), 'utf8');

  assert.match(coreAutomation, /pub struct AutomationRuntimeRuleConfig/u);
  assert.match(coreAutomation, /pub struct AutomationRuntimeCandidatePayload/u);
  assert.match(coreAutomation, /pub enum AutomationRuntimePathCollectionOutcome/u);
  assert.match(coreAutomation, /pub struct AutomationRuntimePathMetadata/u);
  assert.match(coreAutomation, /pub fn should_consider_runtime_candidate_path/u);
  assert.match(coreAutomation, /pub fn collect_runtime_rule_path_result/u);
  assert.match(desktopPlatformRuntime, /sona_core::automation::\{/u);
  assert.match(desktopPlatformRuntime, /collect_runtime_rule_path_result/u);
  assert.match(desktopPlatformRuntime, /should_consider_runtime_candidate_path/u);
  assert.match(desktopLib, /^pub mod platform;/mu);
  assert.match(desktopLib, /crate::platform::automation_runtime::AutomationRuntimeState/u);
  assert.match(desktopCommands, /crate::platform::automation_runtime::\{/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'automation.rs')), false);
  assert.doesNotMatch(desktopPlatformRuntime, /const SUPPORTED_MEDIA_EXTENSIONS/u);
  assert.doesNotMatch(desktopPlatformRuntime, /pub struct AutomationRuntimeRuleConfig/u);
  assert.doesNotMatch(desktopPlatformRuntime, /pub struct AutomationRuntimeCandidatePayload/u);
  assert.doesNotMatch(desktopPlatformRuntime, /pub enum AutomationRuntimePathCollectionOutcome/u);
  assert.doesNotMatch(desktopPlatformRuntime, /fn is_supported_media_path/u);
  assert.doesNotMatch(desktopPlatformRuntime, /fn is_path_within_watch_scope/u);
});

test('SQLite project repository is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const platformProjectRepository = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'project_repository.rs'),
    'utf8',
  );
  const desktopProjectCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'project.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'project.rs')));
  assert.match(sqliteLib, /^pub mod project;/mu);
  assert.match(sqliteLib, /^pub use project::SqliteProjectRepository;/mu);
  assert.match(platformProjectRepository, /sona_sqlite::project::SqliteProjectRepository/u);
  assert.match(desktopProjectCommand, /sona_core::project::\{/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project')), false);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project', 'sqlite_repository.rs')),
    false,
  );
});

test('LLM usage domain and SQLite usage store are owned by core and sqlite adapter', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const tauriLlm = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm.rs'), 'utf8');
  const tauriIntegrations = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'mod.rs'), 'utf8');
  const tauriLlmTypes = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'types.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'core', 'src', 'llm_usage.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'llm_usage.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'analytics.rs')));
  assert.match(coreLib, /^pub mod llm_usage;/mu);
  assert.match(sqliteLib, /^pub mod llm_usage;/mu);
  assert.match(sqliteLib, /^pub mod analytics;/mu);
  assert.match(tauriLlm, /pub\(crate\) use sona_core::llm_usage;/u);
  assert.match(tauriIntegrations, /pub use sona_sqlite::llm_usage as llm_usage_sqlite;/u);
  assert.match(tauriLlmTypes, /pub use sona_core::llm_usage::\{LlmGenerateSource, LlmUsageCategory, TokenUsage\};/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'analytics.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm_usage.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm_usage_sqlite.rs')), false);
});

test('LLM task models and prompt planning are owned by core and reused by desktop', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreLlmTasks = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'llm_tasks.rs'), 'utf8');
  const desktopLlm = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm.rs'), 'utf8');
  const desktopLlmTypes = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'types.rs'), 'utf8');
  const desktopLlmTasks = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'tasks.rs'), 'utf8');
  const tsBindLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'ts_bind', 'src', 'lib.rs'), 'utf8');

  assert.match(coreLib, /^pub mod llm_tasks;/mu);
  assert.match(coreLlmTasks, /pub enum LlmTaskType/u);
  assert.match(coreLlmTasks, /pub struct LlmSegmentInput/u);
  assert.match(coreLlmTasks, /pub fn plan_segment_task_chunks/u);
  assert.match(coreLlmTasks, /pub fn build_polish_prompt/u);
  assert.match(coreLlmTasks, /pub fn build_summary_chunk_prompt/u);
  assert.match(desktopLlm, /pub\(crate\) use sona_core::llm_tasks::\{/u);
  assert.match(desktopLlmTypes, /pub use sona_core::llm_tasks::\{[\s\S]*LlmTaskType[\s\S]*SummarySegmentInput/u);
  assert.match(desktopLlmTasks, /sona_core::llm_tasks::\{/u);
  assert.match(desktopLlmTasks, /plan_segment_task_chunks/u);
  assert.match(desktopLlmTasks, /build_polish_prompt/u);
  assert.match(tsBindLib, /sona_core::llm_tasks::\{/u);
  assert.match(tsBindLib, /LlmTaskType/u);
  assert.match(tsBindLib, /TranscriptSummaryResult/u);
  assert.doesNotMatch(desktopLlmTypes, /pub enum LlmTaskType/u);
  assert.doesNotMatch(desktopLlmTypes, /pub struct LlmSegmentInput/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) fn plan_segment_task_chunks/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) fn build_polish_prompt/u);
});

test('transcript LLM job helpers are owned by core and reused by desktop', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreLlmJobs = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'llm_jobs.rs'), 'utf8');
  const desktopLlmJobs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'jobs.rs'), 'utf8');

  assert.match(coreLib, /^pub mod llm_jobs;/mu);
  assert.match(coreLlmJobs, /pub fn normalized_job_history_id/u);
  assert.match(coreLlmJobs, /pub fn segment_inputs_from_transcript/u);
  assert.match(coreLlmJobs, /pub fn merge_translated_items_into_segments/u);
  assert.match(coreLlmJobs, /pub fn compute_summary_source_fingerprint/u);
  assert.match(desktopLlmJobs, /sona_core::llm_jobs::\{/u);
  assert.doesNotMatch(desktopLlmJobs, /fn normalized_job_history_id/u);
  assert.doesNotMatch(desktopLlmJobs, /fn segment_inputs_from_transcript/u);
  assert.doesNotMatch(desktopLlmJobs, /pub\(crate\) fn merge_translated_items_into_segments/u);
  assert.doesNotMatch(desktopLlmJobs, /pub\(crate\) fn compute_summary_source_fingerprint/u);
});

test('LLM provider protocol mapping is owned by core and reused by desktop', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreProviderProtocol = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'llm_provider_protocol.rs'), 'utf8');
  const desktopLlmTypes = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'types.rs'), 'utf8');
  const desktopLlmProviders = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'providers.rs'), 'utf8');
  const onlineLlmLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'online_llm', 'src', 'lib.rs'), 'utf8');

  assert.match(coreLib, /^pub mod llm_provider_protocol;/mu);
  assert.match(coreProviderProtocol, /pub struct LlmModelSummary/u);
  assert.match(coreProviderProtocol, /pub struct StandardLlmRequest/u);
  assert.match(coreProviderProtocol, /pub fn format_openai_models_urls/u);
  assert.match(coreProviderProtocol, /pub fn extract_text_from_json_response/u);
  assert.match(desktopLlmTypes, /pub use sona_core::llm_provider_protocol::\{/u);
  assert.match(onlineLlmLib, /sona_core::llm_provider_protocol::\{/u);
  assert.match(desktopLlmProviders, /sona_online_llm::\{/u);
  assert.doesNotMatch(desktopLlmProviders, /fn strategy_uses_openai_chat_payload/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn clean_gemini_base_url/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn format_openai_models_urls/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn extract_text_from_json_response/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn build_standard_input/u);
});

test('LLM streaming protocol helpers are owned by core and reused by desktop', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreStreaming = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'llm_streaming_protocol.rs'), 'utf8');
  const desktopLlm = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm.rs'), 'utf8');
  const desktopStreaming = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'streaming.rs'), 'utf8');
  const onlineLlmLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'online_llm', 'src', 'lib.rs'), 'utf8');

  assert.match(coreLib, /^pub mod llm_streaming_protocol;/mu);
  assert.match(coreStreaming, /pub struct StreamTextAccumulator/u);
  assert.match(coreStreaming, /pub struct StreamingLineBuffer/u);
  assert.match(coreStreaming, /pub struct SseEventBuffer/u);
  assert.match(coreStreaming, /pub fn build_openai_chat_payload/u);
  assert.match(desktopLlm, /sona_core::llm_streaming_protocol::\{/u);
  assert.match(onlineLlmLib, /pub async fn try_stream_text_with_provider/u);
  assert.match(desktopStreaming, /try_stream_text_with_provider/u);
  assert.doesNotMatch(desktopStreaming, /pub\(crate\) struct StreamTextAccumulator/u);
  assert.doesNotMatch(desktopStreaming, /pub\(crate\) struct StreamingLineBuffer/u);
  assert.doesNotMatch(desktopStreaming, /struct SseEventBuffer/u);
  assert.doesNotMatch(desktopStreaming, /pub\(crate\) fn build_openai_chat_payload/u);
  assert.doesNotMatch(desktopStreaming, /fn build_openai_stream_url/u);
  assert.doesNotMatch(desktopStreaming, /reqwest::header::CONTENT_TYPE/u);
  assert.doesNotMatch(desktopStreaming, /rig_core::providers/u);
  assert.doesNotMatch(desktopStreaming, /StreamedAssistantContent/u);
});

test('LLM request and config contracts are owned by core and reused by desktop', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreRequestsPath = path.join(repoRoot, 'core', 'src', 'llm_requests.rs');
  const coreRequests = fs.readFileSync(coreRequestsPath, 'utf8');
  const desktopLlmTypes = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'types.rs'), 'utf8');
  const tsBindLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'ts_bind', 'src', 'lib.rs'), 'utf8');
  const requestTypes = [
    'LlmConfig',
    'LlmGenerateRequest',
    'LlmUsageEventPayload',
    'LlmModelsRequest',
    'PolishSegmentsRequest',
    'TranslateSegmentsRequest',
    'SummarizeTranscriptRequest',
    'TranscriptLlmJobRequest',
    'TranscriptSummaryRecordPayload',
    'HistorySummaryPayload',
  ];

  assert.match(coreLib, /^pub mod llm_requests;/mu);
  for (const typeName of requestTypes) {
    assert.match(coreRequests, new RegExp(`pub struct ${typeName}\\b`, 'u'));
    assert.match(desktopLlmTypes, new RegExp(`\\b${typeName}\\b`, 'u'));
    assert.match(tsBindLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(desktopLlmTypes, /pub use sona_core::llm_requests::\{/u);
  assert.match(tsBindLib, /sona_core::llm_requests::\{/u);
  assert.doesNotMatch(desktopLlmTypes, /struct RawLlmConfig/u);
  for (const typeName of requestTypes) {
    assert.doesNotMatch(desktopLlmTypes, new RegExp(`pub struct ${typeName}\\b`, 'u'));
  }
  assert.match(desktopLlmTypes, /pub struct TranscriptLlmJobResult/u);
});

test('LLM generation and model listing use core ports with desktop adapters', () => {
  const corePorts = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'ports', 'mod.rs'), 'utf8');
  const coreLlmPortPath = path.join(repoRoot, 'core', 'src', 'ports', 'llm.rs');
  const coreLlmPort = fs.readFileSync(coreLlmPortPath, 'utf8');
  const desktopCommands = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'commands.rs'), 'utf8');
  const desktopProviders = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'providers.rs'), 'utf8');
  const onlineLlmLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'online_llm', 'src', 'lib.rs'), 'utf8');

  assert.match(corePorts, /^pub mod llm;/mu);
  assert.match(coreLlmPort, /trait LlmTextGenerator/u);
  assert.match(coreLlmPort, /trait LlmModelLister/u);
  assert.match(coreLlmPort, /LlmGenerateRequest/u);
  assert.match(coreLlmPort, /LlmModelsRequest/u);
  assert.match(coreLlmPort, /StandardLlmResponse/u);
  assert.match(coreLlmPort, /LlmModelSummary/u);
  assert.match(onlineLlmLib, /impl LlmTextGenerator for OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /impl LlmModelLister for OnlineLlmAdapter/u);
  assert.match(desktopProviders, /OnlineLlmAdapter as DesktopLlmAdapter/u);
  assert.match(desktopCommands, /DesktopLlmAdapter/u);
  assert.match(desktopCommands, /\.generate_text\(/u);
  assert.match(desktopCommands, /\.list_models\(/u);
  assert.doesNotMatch(desktopCommands, /generate_with_rig/u);
});

test('online LLM provider implementation lives in adapter crate', () => {
  const workspaceCargo = fs.readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8');
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const onlineLlmCargoPath = path.join(repoRoot, 'adapters', 'online_llm', 'Cargo.toml');
  const onlineLlmLibPath = path.join(repoRoot, 'adapters', 'online_llm', 'src', 'lib.rs');
  const onlineLlmLib = fs.readFileSync(onlineLlmLibPath, 'utf8');
  const desktopProviders = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'providers.rs'), 'utf8');
  const desktopNetwork = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'network.rs'), 'utf8');
  const desktopTasks = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm', 'tasks.rs'), 'utf8');

  assert.match(workspaceCargo, /"adapters\/online_llm"/u);
  assert.ok(fs.existsSync(onlineLlmCargoPath));
  assert.match(tauriCargo, /sona-online-llm\s*=\s*\{\s*path\s*=\s*"..\/adapters\/online_llm"/u);
  assert.match(onlineLlmLib, /pub struct OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /impl LlmTextGenerator for OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /impl LlmModelLister for OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /pub struct LlmApiUrl/u);
  assert.match(desktopProviders, /sona_online_llm::\{/u);
  assert.match(desktopNetwork, /sona_online_llm::\{/u);
  assert.match(desktopTasks, /sona_online_llm::\{/u);
  assert.match(onlineLlmLib, /pub async fn execute_google_translate_free_request/u);
  assert.match(onlineLlmLib, /pub async fn fetch_google_translate_free_translation/u);
  assert.doesNotMatch(desktopProviders, /pub(?:\(crate\))? trait LlmAdapter/u);
  assert.doesNotMatch(desktopProviders, /struct AdapterFactory/u);
  assert.doesNotMatch(desktopProviders, /pub\(crate\) async fn get_openai_models/u);
  assert.doesNotMatch(desktopProviders, /pub\(crate\) async fn get_gemini_models/u);
  assert.doesNotMatch(desktopTasks, /enum GoogleTranslateFreeAttemptError/u);
  assert.doesNotMatch(desktopTasks, /fn parse_google_translate_free_retry_after/u);
  assert.doesNotMatch(desktopTasks, /fn extract_google_translate_free_translation/u);
  assert.doesNotMatch(desktopTasks, /fn google_translate_free_retry_delay/u);
});

test('storage usage SQLite and filesystem scanner is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopStorageCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'storage.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'storage_usage.rs')));
  assert.match(sqliteLib, /^pub mod storage_usage;/mu);
  assert.match(desktopStorageCommand, /sona_sqlite::storage_usage::\{/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'storage_usage.rs')), false);
});

test('history filesystem helpers are owned by core history module', () => {
  const coreHistory = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'history', 'mod.rs'), 'utf8');
  const platformHistory = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository.rs'),
    'utf8',
  );

  assert.ok(fs.existsSync(path.join(repoRoot, 'core', 'src', 'history', 'fs_utils.rs')));
  assert.match(coreHistory, /^pub mod fs_utils;/mu);
  assert.match(platformHistory, /pub\(crate\) use sona_core::history::fs_utils;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'fs_utils.rs')), false);
  assert.doesNotMatch(platformHistory, /^pub\(crate\) mod fs_utils;/mu);
});

test('SQLite history store is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const platformHistory = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository.rs'),
    'utf8',
  );

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_store.rs')));
  assert.match(sqliteLib, /^pub mod history_store;/mu);
  assert.match(sqliteLib, /^pub use history_store::SqliteHistoryStore;/mu);
  assert.match(platformHistory, /pub use sona_sqlite::history_store as sqlite_store;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'sqlite_store.rs')), false);
  assert.doesNotMatch(platformHistory, /^pub mod sqlite_store;/mu);
});

test('history backup archive persistence is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const platformHistory = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'history_repository.rs'),
    'utf8',
  );

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_backup.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_archive.rs')));
  assert.match(sqliteLib, /^pub mod history_backup;/mu);
  assert.match(sqliteLib, /^pub mod history_archive;/mu);
  assert.match(platformHistory, /pub use sona_sqlite::history_backup as backup;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'backup.rs')), false);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'repository.rs')),
    false,
  );
  assert.doesNotMatch(platformHistory, /^pub mod backup;/mu);
  assert.doesNotMatch(platformHistory, /^pub\(crate\) mod repository;/mu);
});

test('standalone CLI invokes local batch ASR through the core transcriber port', () => {
  const cliTranscribeRs = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'cli', 'src', 'transcribe.rs'),
    'utf8',
  );

  assert.match(cliTranscribeRs, /use sona_core::ports::asr::BatchTranscriber;/u);
  assert.match(cliTranscribeRs, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.match(cliTranscribeRs, /\.transcribe\(plan\)/u);
  assert.doesNotMatch(cliTranscribeRs, /run_offline_transcription/u);
});

test('recognizer transcript utilities are owned by core and reused by adapters', () => {
  const coreTranscript = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'transcript.rs'), 'utf8');
  const asrMod = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'mod.rs'), 'utf8');
  const tauriTranscript = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const localBatch = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'batch.rs'),
    'utf8',
  );

  assert.match(coreTranscript, /pub fn normalize_recognizer_text\(/u);
  assert.match(coreTranscript, /pub fn synthesize_durations\(/u);
  assert.match(asrMod, /pub use sona_core::transcript_postprocess::TranscriptPostprocessor/u);
  assert.doesNotMatch(asrMod, /^mod postprocess;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'postprocess.rs')), false);
  assert.match(
    tauriTranscript,
    /pub\(crate\) use sona_core::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.match(
    localBatch,
    /use sona_core::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn normalize_recognizer_text/u);
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn synthesize_durations/u);
  assert.doesNotMatch(localBatch, /^fn normalize_recognizer_text/mu);
  assert.doesNotMatch(localBatch, /^fn synthesize_durations/mu);
});

test('timeline transcript normalization is owned by core and reused by desktop', () => {
  const coreTranscript = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'transcript.rs'), 'utf8');
  const tauriTranscript = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );

  assert.match(coreTranscript, /pub fn apply_timeline_normalization\(/u);
  assert.match(coreTranscript, /pub fn build_transcript_update\(/u);
  assert.match(
    tauriTranscript,
    /pub\(crate\) use sona_core::transcript::\{[\s\S]*apply_timeline_normalization[\s\S]*build_transcript_update[\s\S]*\};/u,
  );
  assert.doesNotMatch(tauriTranscript, /struct TokenMap/u);
  assert.doesNotMatch(tauriTranscript, /struct SplitterState/u);
  assert.doesNotMatch(tauriTranscript, /fn split_segment_by_parts/u);
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn apply_timeline_normalization/u);
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn build_transcript_update/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn emit_transcript_update/u);
});

test('desktop Groq and Mistral batch providers delegate HTTP work to online ASR adapter', () => {
  const workspaceCargo = fs.readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8');
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const onlineAsrCargoPath = path.join(repoRoot, 'adapters', 'online_asr', 'Cargo.toml');
  const onlineAsrLibPath = path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs');
  const coreAsr = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'ports', 'asr.rs'), 'utf8');
  const groqRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'groq.rs'), 'utf8');
  const mistralRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'mistral.rs'), 'utf8');

  assert.match(workspaceCargo, /"adapters\/online_asr"/u);
  assert.ok(fs.existsSync(onlineAsrCargoPath));
  assert.ok(fs.existsSync(onlineAsrLibPath));
  assert.match(tauriCargo, /sona-online-asr\s*=\s*\{\s*path\s*=\s*"..\/adapters\/online_asr"/u);
  assert.match(prWorkflow, /sona-online-asr/u);
  assert.match(coreAsr, /trait OnlineBatchTranscriber/u);
  assert.match(coreAsr, /struct OnlineBatchTranscriptionRequest/u);
  assert.match(coreAsr, /struct OnlineBatchTranscriptionOutput/u);

  for (const providerRs of [groqRs, mistralRs]) {
    assert.match(providerRs, /sona_online_asr::/u);
    assert.doesNotMatch(providerRs, /reqwest::multipart/u);
    assert.doesNotMatch(providerRs, /reqwest::Client/u);
    assert.doesNotMatch(providerRs, /\.post\(/u);
  }
});

test('desktop Volcengine batch provider delegates HTTP work to online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /VolcengineDoubaoBatchTranscriber/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config/u);
  assert.match(onlineAsrLib, /build_volcengine_flash_batch_request_body/u);
  assert.match(onlineAsrLib, /segments_from_volcengine_response/u);
  assert.match(volcengineRs, /sona_online_asr::VolcengineDoubaoBatchTranscriber/u);
  assert.doesNotMatch(volcengineRs, /reqwest::Client/u);
  assert.doesNotMatch(volcengineRs, /\.post\(/u);
  assert.doesNotMatch(volcengineRs, /base64::engine::general_purpose::STANDARD\.encode/u);
});

test('desktop Volcengine streaming protocol helpers are owned by online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /build_volcengine_full_client_request_frame/u);
  assert.match(onlineAsrLib, /build_volcengine_audio_frame/u);
  assert.match(onlineAsrLib, /parse_volcengine_server_response_frame/u);
  assert.match(onlineAsrLib, /volcengine_streaming_segments_from_response/u);
  assert.match(onlineAsrLib, /f32_samples_to_i16_pcm_bytes/u);
  assert.match(volcengineRs, /sona_online_asr::build_volcengine_full_client_request_frame/u);
  assert.match(volcengineRs, /sona_online_asr::parse_volcengine_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /pub fn build_audio_frame/u);
  assert.doesNotMatch(volcengineRs, /fn parse_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /fn f32_samples_to_i16_pcm_bytes/u);
});

test('desktop Volcengine config and response helpers are owned by online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /VolcengineConfigError/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config_checked/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config_from_value_checked/u);
  assert.match(onlineAsrLib, /build_volcengine_flash_batch_request_body/u);
  assert.match(onlineAsrLib, /segments_from_volcengine_response/u);
  assert.match(onlineAsrLib, /map_volcengine_status_error/u);
  assert.match(volcengineRs, /sona_online_asr::resolve_volcengine_config_checked/u);
  assert.doesNotMatch(volcengineRs, /pub enum VolcengineMode/u);
  assert.doesNotMatch(volcengineRs, /pub struct VolcengineDoubaoConfigFields/u);
  assert.doesNotMatch(volcengineRs, /fn config_fields/u);
  assert.doesNotMatch(volcengineRs, /pub fn validate_config/u);
  assert.doesNotMatch(volcengineRs, /fn detect_audio_format/u);
  assert.doesNotMatch(volcengineRs, /fn build_flash_batch_request_body/u);
  assert.doesNotMatch(volcengineRs, /pub fn segments_from_response_value/u);
  assert.doesNotMatch(volcengineRs, /fn segment_from_utterance/u);
  assert.doesNotMatch(volcengineRs, /fn ms_value/u);
  assert.doesNotMatch(volcengineRs, /pub fn map_status_error/u);
});

test('desktop hardware module reuses local ASR adapter GPU planning', () => {
  const hardwareRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'hardware.rs'), 'utf8');

  assert.match(hardwareRs, /pub\(crate\) use sona_local_asr::gpu::\{/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::check_gpu_availability\(\)\.await/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::resolve_gpu_acceleration_plan/u);
  assert.doesNotMatch(hardwareRs, /tokio::process::Command/u);
  assert.doesNotMatch(hardwareRs, /struct GpuAccelerationPlan/u);
  assert.doesNotMatch(hardwareRs, /struct GpuFallbackNotice/u);
});

test('desktop local audio helpers come from local ASR adapter without Tauri core pipeline', () => {
  const batchRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'), 'utf8');
  const localAsrSpeakerProcessing = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs'),
    'utf8',
  );
  const runtimeStatusRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'runtime_status.rs'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'pipeline.rs')), false);
  assert.match(batchRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(batchRs, /sona_local_asr::audio::save_wav_file/u);
  assert.match(localAsrSpeakerProcessing, /crate::audio::extract_and_resample_audio/u);
  assert.match(localAsrSpeakerProcessing, /crate::audio::save_wav_file/u);
  assert.match(runtimeStatusRs, /sona_local_asr::audio::resolve_ffmpeg_sidecar_path/u);
  assert.doesNotMatch(prWorkflow, /core::pipeline::tests/u);

  const desktopPipelineReferences = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /crate::core::pipeline|core::pipeline/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopPipelineReferences, []);
});

test('desktop batch ASR delegates audio segmentation to local ASR adapter', () => {
  const batchRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'), 'utf8');

  assert.match(batchRs, /sona_local_asr::audio::segment_batch_audio/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::SileroVadModelConfig/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::VadModelConfig/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::vad_segment_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::fixed_chunk_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::whole_audio_segment/u);
});

test('speaker processing runtime is owned by local ASR adapter and wrapped by desktop platform', () => {
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const localAsrLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'lib.rs'), 'utf8');
  const localAsrProcessing = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs'),
    'utf8',
  );
  const desktopPlatform = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'platform', 'mod.rs'), 'utf8');
  const platformSpeaker = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'platform', 'speaker_processing.rs'),
    'utf8',
  );
  const desktopIntegrations = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'mod.rs'), 'utf8');
  const systemCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'system.rs'), 'utf8');
  const batchRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'), 'utf8');

  assert.match(localAsrLib, /^pub mod speaker;/mu);
  assert.match(localAsrLib, /^pub mod speaker_processing;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker.rs')), true);
  assert.equal(fs.existsSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs')), true);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'speaker.rs')), false);
  assert.match(desktopPlatform, /^pub mod speaker_processing;/mu);
  assert.doesNotMatch(desktopIntegrations, /^pub mod speaker;/mu);
  assert.doesNotMatch(tauriCargo, /^sherpa-onnx\s*=/mu);
  assert.match(localAsrProcessing, /crate::speaker::run_speaker_diarization/u);
  assert.match(localAsrProcessing, /crate::speaker::SpeakerEmbeddingIndex/u);
  assert.match(localAsrProcessing, /crate::audio::extract_and_resample_audio/u);
  assert.doesNotMatch(localAsrProcessing, /use sherpa_onnx::/u);
  assert.doesNotMatch(localAsrProcessing, /OfflineSpeakerDiarization/u);
  assert.doesNotMatch(localAsrProcessing, /SpeakerEmbeddingExtractor/u);
  assert.doesNotMatch(localAsrProcessing, /SpeakerEmbeddingManager/u);
  assert.match(platformSpeaker, /sona_local_asr::speaker_processing::annotate_speaker_segments_from_file/u);
  assert.match(platformSpeaker, /sona_local_asr::speaker_processing::import_speaker_profile_sample/u);
  assert.match(systemCommand, /crate::platform::speaker_processing::annotate_speaker_segments_from_file/u);
  assert.match(systemCommand, /crate::platform::speaker_processing::import_speaker_profile_sample/u);
  assert.match(batchRs, /sona_local_asr::speaker_processing::annotate_segments_with_speakers/u);
  assert.doesNotMatch(batchRs, /crate::integrations::speaker/u);
});

test('media file detection is owned by core and reused by desktop', () => {
  const coreCargo = fs.readFileSync(path.join(repoRoot, 'core', 'Cargo.toml'), 'utf8');
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreMediaDetector = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'media_detector.rs'), 'utf8');
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const desktopIntegrations = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'mod.rs'), 'utf8');
  const systemCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'system.rs'), 'utf8');
  const apiServer = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'server.rs'), 'utf8');

  assert.match(coreCargo, /^infer\s*=/mu);
  assert.match(coreLib, /^pub mod media_detector;/mu);
  assert.match(coreMediaDetector, /pub fn is_valid_media_bytes/u);
  assert.match(coreMediaDetector, /pub async fn is_valid_media_file/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'media_detector.rs')), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod media_detector;/mu);
  assert.doesNotMatch(tauriCargo, /^infer\s*=/mu);
  assert.match(systemCommand, /sona_core::media_detector::check_media_formats/u);
  assert.match(apiServer, /sona_core::media_detector::is_valid_media_file/u);
  assert.doesNotMatch(systemCommand, /crate::integrations::media_detector/u);
  assert.doesNotMatch(apiServer, /crate::integrations::media_detector/u);
});

test('desktop live VAD creation is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const sherpaRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'sherpa_onnx.rs'),
    'utf8',
  );

  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs')), false);
  assert.doesNotMatch(asrMod, /^mod model_config;/mu);
  assert.match(asrMod, /pub\(crate\) use sona_local_asr::audio::\{[\s\S]*load_vad/u);
  assert.match(sherpaRs, /use sona_local_asr::audio::\{[\s\S]*SafeVad/u);
  assert.doesNotMatch(asrMod, /create_vad_detector/u);
  assert.doesNotMatch(asrMod, /pub struct SafeVad/u);
  assert.doesNotMatch(asrMod, /SileroVadModelConfig/u);
  assert.doesNotMatch(asrMod, /VadModelConfig/u);
  assert.doesNotMatch(asrMod, /VoiceActivityDetector/u);
});

test('desktop punctuation loading is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(asrMod, /pub use sona_local_asr::punctuation::\{Punctuation, load_punctuation\}/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuation/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuationConfig/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuationModelConfig/u);
});

test('desktop recognizer construction is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(asrMod, /pub use sona_local_asr::recognizer::/u);
  assert.doesNotMatch(asrMod, /use sherpa_onnx::/u);
  assert.doesNotMatch(asrMod, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(asrMod, /OnlineRecognizerConfig/u);
  assert.doesNotMatch(asrMod, /pub enum ModelType/u);
  assert.doesNotMatch(asrMod, /impl Recognizer/u);
});

test('local batch transcription reuses local ASR recognizer model construction', () => {
  const batchRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'batch.rs'),
    'utf8',
  );
  const recognizerRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'),
    'utf8',
  );

  assert.match(batchRs, /pub struct LocalBatchAsrAdapter/u);
  assert.match(batchRs, /impl BatchTranscriber for LocalBatchAsrAdapter/u);
  assert.match(batchRs, /use crate::recognizer::\{[^}]*build_offline_model_config/u);
  assert.match(batchRs, /decode_offline_samples/u);
  assert.match(batchRs, /create_offline_recognizer/u);
  assert.match(recognizerRs, /pub fn create_offline_recognizer\([\s\S]*\) -> Result<SafeOfflineRecognizer, String>/u);
  assert.doesNotMatch(batchRs, /pub async fn run_offline_transcription/u);
  assert.doesNotMatch(batchRs, /enum ModelType/u);
  assert.doesNotMatch(batchRs, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(recognizerRs, /\) -> Result<OfflineRecognizer, String>/u);
  assert.doesNotMatch(batchRs, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(batchRs, /fn build_model_config/u);
  assert.doesNotMatch(batchRs, /fn create_recognizer/u);
});

test('desktop live offline decode is delegated to local ASR adapter', () => {
  const sherpaRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'sherpa_onnx.rs'),
    'utf8',
  );

  assert.match(sherpaRs, /decode_offline_samples/u);
  assert.doesNotMatch(sherpaRs, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(sherpaRs, /let stream = r\.create_stream\(\)/u);
});

test('desktop batch offline decode is delegated to local ASR adapter', () => {
  const batchRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );

  assert.match(batchRs, /decode_offline_samples/u);
  assert.doesNotMatch(batchRs, /FFI: Calling accept_waveform \(Offline segment\)/u);
  assert.doesNotMatch(batchRs, /let stream = r\.0\.create_stream\(\)/u);
});

test('desktop streaming offline decode is delegated to local ASR adapter', () => {
  const streamingRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'streaming.rs'),
    'utf8',
  );

  assert.match(streamingRs, /decode_offline_samples/u);
  assert.doesNotMatch(streamingRs, /let stream = r\.0\.create_stream\(\)/u);
  assert.doesNotMatch(streamingRs, /stream\.accept_waveform\(16000, &full_audio\)/u);
  assert.doesNotMatch(streamingRs, /r\.0\.decode\(&stream\)/u);
});

test('desktop online stream operations are delegated to local ASR adapter', () => {
  const batchRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );
  const sherpaRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'sherpa_onnx.rs'),
    'utf8',
  );
  const desktopOnlineRs = `${batchRs}\n${sherpaRs}`;

  assert.match(desktopOnlineRs, /use sona_local_asr::recognizer::\{[\s\S]*create_online_stream/u);
  assert.match(desktopOnlineRs, /use sona_local_asr::recognizer::\{[\s\S]*accept_online_samples/u);
  assert.match(desktopOnlineRs, /use sona_local_asr::recognizer::\{[\s\S]*decode_online_ready/u);
  assert.match(desktopOnlineRs, /use sona_local_asr::recognizer::\{[\s\S]*online_stream_result/u);
  assert.doesNotMatch(desktopOnlineRs, /SafeStream\(r\.0\.create_stream\(\)\)/u);
  assert.doesNotMatch(desktopOnlineRs, /\.0\.accept_waveform\(16000/u);
  assert.doesNotMatch(desktopOnlineRs, /r\.0\.is_ready\(&[^)]*\.0\)/u);
  assert.doesNotMatch(desktopOnlineRs, /r\.0\.decode\(&[^)]*\.0\)/u);
  assert.doesNotMatch(desktopOnlineRs, /r\.0\.get_result\(&[^)]*\.0\)/u);
  assert.doesNotMatch(desktopOnlineRs, /r\.0\.reset\(&[^)]*\.0\)/u);
});

test('desktop VAD runtime operations use private local ASR wrappers', () => {
  const audioRs = fs.readFileSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'audio.rs'), 'utf8');
  const recognizerRs = fs.readFileSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'), 'utf8');
  const asrMod = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const sherpaRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'sherpa_onnx.rs'),
    'utf8',
  );
  const streamingRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'streaming.rs'),
    'utf8',
  );
  const desktopVadRs = `${sherpaRs}\n${streamingRs}`;

  assert.match(asrMod, /accept_vad_samples/u);
  assert.match(sherpaRs, /use sona_local_asr::audio::\{[\s\S]*reset_vad/u);
  assert.match(asrMod, /vad_detected/u);
  assert.doesNotMatch(desktopVadRs, /SafeVad\([^)]+\)/u);
  assert.doesNotMatch(desktopVadRs, /\.0\.accept_waveform/u);
  assert.doesNotMatch(desktopVadRs, /\.0\.detected/u);
  assert.doesNotMatch(audioRs, /pub struct SafeVad\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeOnlineRecognizer\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeOfflineRecognizer\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeStream\(pub /u);
});

test('local ASR VAD sherpa primitives are crate-private', () => {
  const audioRs = fs.readFileSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'audio.rs'), 'utf8');

  assert.doesNotMatch(audioRs, /^pub type VadConfig\s*=/mu);
  assert.doesNotMatch(audioRs, /^pub type VadDetector\s*=/mu);
  assert.doesNotMatch(audioRs, /^pub fn create_vad_config/mu);
  assert.doesNotMatch(audioRs, /^pub fn create_vad_detector/mu);
  assert.doesNotMatch(audioRs, /^pub fn vad_segment_audio_with_capacity/mu);
  assert.match(audioRs, /^pub\(crate\) type VadConfig\s*=/mu);
  assert.match(audioRs, /^pub\(crate\) fn create_vad_config/mu);
});
