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

test('desktop api server invokes local offline ASR through the core transcriber port', () => {
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const apiServer = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'server.rs'), 'utf8');

  assert.match(tauriCargo, /^sona-local-asr\s*=\s*\{ path = "\.\.\/adapters\/local_asr" \}/mu);
  assert.match(apiServer, /use sona_core::ports::asr::OfflineTranscriber;/u);
  assert.match(apiServer, /sona_local_asr::offline::LocalOfflineAsrAdapter/u);
  assert.match(apiServer, /\.transcribe\(plan\)/u);
  assert.doesNotMatch(apiServer, /run_offline_transcription/u);
  assert.doesNotMatch(apiServer, /use crate::integrations::asr::transcribe_batch_with_progress;/u);
  assert.doesNotMatch(apiServer, /LocalSherpaAdapter::offline_plan_to_batch_request/u);
});

test('pr guardrails run ASR adapter tests with core bindings and standalone CLI', () => {
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.match(
    prWorkflow,
    /cargo test -p sona-core -p sona-local-asr -p sona-online-asr -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
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

test('online ASR provider manifest is owned by core and re-exported by desktop', () => {
  const coreAsr = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'ports', 'asr.rs'), 'utf8');
  const coreManifestPath = path.join(repoRoot, 'core', 'src', 'ports', 'online-asr-providers.json');
  const legacySharedManifestPath = path.join(repoRoot, 'src', 'shared', 'online-asr-providers.json');
  const desktopProviders = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr_providers.rs'),
    'utf8',
  );
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

  assert.match(desktopProviders, /pub use sona_core::ports::asr::\{/u);
  assert.doesNotMatch(desktopProviders, /include_str!/u);
  assert.doesNotMatch(desktopProviders, /struct OnlineAsrProviderManifest/u);
  assert.doesNotMatch(desktopProviders, /static ONLINE_ASR_PROVIDER_MANIFEST/u);

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

test('app config migration and LLM provider manifest are owned by core', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreConfig = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'config', 'mod.rs'), 'utf8');
  const coreDefaults = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'config', 'defaults.rs'), 'utf8');
  const coreMigration = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'config', 'migration.rs'), 'utf8');
  const desktopConfig = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'mod.rs'), 'utf8');
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
  assert.match(desktopConfig, /pub use sona_core::config::\{/u);
  assert.match(desktopConfig, /pub use sona_sqlite::config_store as sqlite_store;/u);
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
  const desktopDatabase = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'mod.rs'), 'utf8');
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
  assert.match(desktopDatabase, /pub use sona_sqlite::\{[\s\S]*legacy_migration[\s\S]*\};/u);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'legacy_migration.rs')),
    false,
  );
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'error.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'ports.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'database', 'schema.rs')), false);
  assert.doesNotMatch(desktopDatabase, /pub mod legacy_migration;/u);
  assert.doesNotMatch(desktopDatabase, /rusqlite::Connection/u);
  assert.doesNotMatch(desktopDatabase, /struct ConnectionPool/u);
  assert.doesNotMatch(desktopDatabase, /pub struct Database/u);
});

test('SQLite config and task ledger stores are owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopCore = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'mod.rs'), 'utf8');
  const desktopConfig = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'mod.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'config_store.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'task_ledger.rs')));
  assert.match(sqliteLib, /^pub mod config_store;/mu);
  assert.match(sqliteLib, /^pub mod task_ledger;/mu);
  assert.match(sqliteLib, /^pub use config_store::SqliteConfigStore;/mu);
  assert.match(sqliteLib, /^pub use task_ledger::SqliteLedgerRepository;/mu);
  assert.match(desktopConfig, /pub use sona_sqlite::config_store as sqlite_store;/u);
  assert.match(desktopCore, /pub use sona_sqlite::task_ledger as task_ledger_sqlite;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'config', 'sqlite_store.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'task_ledger', 'sqlite_repository.rs')), false);
});

test('SQLite automation repository is owned by sqlite adapter', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');
  const coreAutomationPath = path.join(repoRoot, 'core', 'src', 'automation.rs');
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopAutomation = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation.rs'), 'utf8');
  const desktopAutomationRepository = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation', 'repository.rs'),
    'utf8',
  );
  const desktopAutomationTypes = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation', 'types.rs'),
    'utf8',
  );

  assert.ok(fs.existsSync(coreAutomationPath));
  assert.match(coreLib, /^pub mod automation;/mu);
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'automation.rs')));
  assert.match(sqliteLib, /^pub mod automation;/mu);
  assert.match(sqliteLib, /^pub use automation::\{AutomationRepositoryState, SqliteAutomationRepository\};/mu);
  assert.match(desktopAutomation, /pub use sona_sqlite::automation as sqlite_repository;/u);
  assert.match(desktopAutomationTypes, /pub use sona_core::automation::\{/u);
  assert.match(desktopAutomationTypes, /pub use sona_sqlite::automation::AutomationRepositoryState;/u);
  assert.match(desktopAutomationRepository, /validate_rule_activation/u);
  assert.doesNotMatch(desktopAutomationRepository, /is_feature_llm_config_complete/u);
  assert.doesNotMatch(desktopAutomationRepository, /fn is_feature_llm_config_complete/u);
  assert.doesNotMatch(desktopAutomationRepository, /fn is_batch_asr_configured/u);
  assert.doesNotMatch(desktopAutomationRepository, /online_asr_providers/u);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'automation', 'sqlite_repository.rs')),
    false,
  );
});

test('automation runtime path rules are owned by core and adapted by desktop', () => {
  const coreAutomation = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'automation.rs'), 'utf8');
  const desktopCoreAutomation = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'core', 'automation.rs'),
    'utf8',
  );

  assert.match(coreAutomation, /pub struct AutomationRuntimeRuleConfig/u);
  assert.match(coreAutomation, /pub struct AutomationRuntimeCandidatePayload/u);
  assert.match(coreAutomation, /pub enum AutomationRuntimePathCollectionOutcome/u);
  assert.match(coreAutomation, /pub struct AutomationRuntimePathMetadata/u);
  assert.match(coreAutomation, /pub fn should_consider_runtime_candidate_path/u);
  assert.match(coreAutomation, /pub fn collect_runtime_rule_path_result/u);
  assert.match(desktopCoreAutomation, /sona_core::automation::\{/u);
  assert.match(desktopCoreAutomation, /collect_runtime_rule_path_result/u);
  assert.match(desktopCoreAutomation, /should_consider_runtime_candidate_path/u);
  assert.doesNotMatch(desktopCoreAutomation, /const SUPPORTED_MEDIA_EXTENSIONS/u);
  assert.doesNotMatch(desktopCoreAutomation, /pub struct AutomationRuntimeRuleConfig/u);
  assert.doesNotMatch(desktopCoreAutomation, /pub struct AutomationRuntimeCandidatePayload/u);
  assert.doesNotMatch(desktopCoreAutomation, /pub enum AutomationRuntimePathCollectionOutcome/u);
  assert.doesNotMatch(desktopCoreAutomation, /fn is_supported_media_path/u);
  assert.doesNotMatch(desktopCoreAutomation, /fn is_path_within_watch_scope/u);
});

test('SQLite project repository is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopProject = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project.rs'), 'utf8');
  const desktopProjectTypes = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'project', 'types.rs'),
    'utf8',
  );

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'project.rs')));
  assert.match(sqliteLib, /^pub mod project;/mu);
  assert.match(sqliteLib, /^pub use project::SqliteProjectRepository;/mu);
  assert.match(desktopProject, /pub use sona_sqlite::project as sqlite_repository;/u);
  assert.match(desktopProjectTypes, /pub use crate::core::project::\*;/u);
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
  const desktopAnalytics = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'analytics.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'core', 'src', 'llm_usage.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'llm_usage.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'analytics.rs')));
  assert.match(coreLib, /^pub mod llm_usage;/mu);
  assert.match(sqliteLib, /^pub mod llm_usage;/mu);
  assert.match(sqliteLib, /^pub mod analytics;/mu);
  assert.match(tauriLlm, /pub\(crate\) use sona_core::llm_usage;/u);
  assert.match(tauriIntegrations, /pub use sona_sqlite::llm_usage as llm_usage_sqlite;/u);
  assert.match(tauriLlmTypes, /pub use sona_core::llm_usage::\{LlmGenerateSource, LlmUsageCategory, TokenUsage\};/u);
  assert.match(desktopAnalytics, /pub use sona_sqlite::analytics::SqliteAnalyticsRepository;/u);
  assert.doesNotMatch(desktopAnalytics, /struct AnalyticsRepositoryImpl/u);
  assert.doesNotMatch(desktopAnalytics, /impl crate::core::dashboard::ports::AnalyticsRepository/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm_usage.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'llm_usage_sqlite.rs')), false);
});

test('storage usage SQLite and filesystem scanner is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopCore = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'mod.rs'), 'utf8');
  const desktopStorageCommand = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'commands', 'storage.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'storage_usage.rs')));
  assert.match(sqliteLib, /^pub mod storage_usage;/mu);
  assert.match(desktopCore, /pub use sona_sqlite::storage_usage;/u);
  assert.match(desktopStorageCommand, /crate::core::storage_usage::\{/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'storage_usage.rs')), false);
  assert.doesNotMatch(desktopCore, /^pub mod storage_usage;/mu);
});

test('history filesystem helpers are owned by core history module', () => {
  const coreHistory = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'history', 'mod.rs'), 'utf8');
  const desktopHistory = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'core', 'src', 'history', 'fs_utils.rs')));
  assert.match(coreHistory, /^pub mod fs_utils;/mu);
  assert.match(desktopHistory, /pub\(crate\) use sona_core::history::fs_utils;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'fs_utils.rs')), false);
  assert.doesNotMatch(desktopHistory, /^pub\(crate\) mod fs_utils;/mu);
});

test('SQLite history store is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopHistory = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_store.rs')));
  assert.match(sqliteLib, /^pub mod history_store;/mu);
  assert.match(sqliteLib, /^pub use history_store::SqliteHistoryStore;/mu);
  assert.match(desktopHistory, /pub use sona_sqlite::history_store as sqlite_store;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'sqlite_store.rs')), false);
  assert.doesNotMatch(desktopHistory, /^pub mod sqlite_store;/mu);
});

test('history backup archive persistence is owned by sqlite adapter', () => {
  const sqliteLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs'), 'utf8');
  const desktopHistory = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history.rs'), 'utf8');

  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_backup.rs')));
  assert.ok(fs.existsSync(path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_archive.rs')));
  assert.match(sqliteLib, /^pub mod history_backup;/mu);
  assert.match(sqliteLib, /^pub mod history_archive;/mu);
  assert.match(desktopHistory, /pub use sona_sqlite::history_backup as backup;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'backup.rs')), false);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'repositories', 'history', 'repository.rs')),
    false,
  );
  assert.doesNotMatch(desktopHistory, /^pub mod backup;/mu);
  assert.doesNotMatch(desktopHistory, /^pub\(crate\) mod repository;/mu);
});

test('standalone CLI invokes local offline ASR through the core transcriber port', () => {
  const cliTranscribeRs = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'cli', 'src', 'transcribe.rs'),
    'utf8',
  );

  assert.match(cliTranscribeRs, /use sona_core::ports::asr::OfflineTranscriber;/u);
  assert.match(cliTranscribeRs, /sona_local_asr::offline::LocalOfflineAsrAdapter/u);
  assert.match(cliTranscribeRs, /\.transcribe\(plan\)/u);
  assert.doesNotMatch(cliTranscribeRs, /run_offline_transcription/u);
});

test('recognizer transcript utilities are owned by core and reused by adapters', () => {
  const coreTranscript = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'transcript.rs'), 'utf8');
  const tauriTranscript = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const localOffline = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'offline.rs'),
    'utf8',
  );

  assert.match(coreTranscript, /pub fn normalize_recognizer_text\(/u);
  assert.match(coreTranscript, /pub fn synthesize_durations\(/u);
  assert.match(
    tauriTranscript,
    /pub\(crate\) use sona_core::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.match(
    localOffline,
    /use sona_core::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn normalize_recognizer_text/u);
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn synthesize_durations/u);
  assert.doesNotMatch(localOffline, /^fn normalize_recognizer_text/mu);
  assert.doesNotMatch(localOffline, /^fn synthesize_durations/mu);
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
  const coreMod = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'mod.rs'), 'utf8');
  const batchRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'), 'utf8');
  const speakerRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'speaker.rs'), 'utf8');
  const runtimeStatusRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'runtime_status.rs'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'pipeline.rs')), false);
  assert.doesNotMatch(coreMod, /^pub mod pipeline;/mu);
  assert.match(batchRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(batchRs, /sona_local_asr::audio::save_wav_file/u);
  assert.match(speakerRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(speakerRs, /sona_local_asr::audio::save_wav_file/u);
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

test('desktop speaker processing delegates sherpa speaker primitives to local ASR adapter', () => {
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const localAsrLib = fs.readFileSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'lib.rs'), 'utf8');
  const speakerRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'speaker.rs'), 'utf8');

  assert.match(localAsrLib, /^pub mod speaker;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker.rs')), true);
  assert.doesNotMatch(tauriCargo, /^sherpa-onnx\s*=/mu);
  assert.doesNotMatch(speakerRs, /use sherpa_onnx::/u);
  assert.match(speakerRs, /sona_local_asr::speaker::run_speaker_diarization/u);
  assert.match(speakerRs, /sona_local_asr::speaker::SpeakerEmbeddingIndex/u);
  assert.doesNotMatch(speakerRs, /OfflineSpeakerDiarization/u);
  assert.doesNotMatch(speakerRs, /SpeakerEmbeddingExtractor/u);
  assert.doesNotMatch(speakerRs, /SpeakerEmbeddingManager/u);
});

test('desktop live VAD creation is delegated to local ASR adapter', () => {
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );

  assert.match(modelConfigRs, /pub use sona_local_asr::audio::\{[^}]*SafeVad/u);
  assert.match(modelConfigRs, /pub use sona_local_asr::audio::\{[^}]*load_vad/u);
  assert.doesNotMatch(modelConfigRs, /create_vad_detector/u);
  assert.doesNotMatch(modelConfigRs, /pub struct SafeVad/u);
  assert.doesNotMatch(modelConfigRs, /SileroVadModelConfig/u);
  assert.doesNotMatch(modelConfigRs, /VadModelConfig/u);
  assert.doesNotMatch(modelConfigRs, /VoiceActivityDetector/u);
});

test('desktop punctuation loading is delegated to local ASR adapter', () => {
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );

  assert.match(modelConfigRs, /pub use sona_local_asr::punctuation::\{Punctuation, load_punctuation\}/u);
  assert.doesNotMatch(modelConfigRs, /OfflinePunctuation/u);
  assert.doesNotMatch(modelConfigRs, /OfflinePunctuationConfig/u);
  assert.doesNotMatch(modelConfigRs, /OfflinePunctuationModelConfig/u);
});

test('desktop recognizer construction is delegated to local ASR adapter', () => {
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );

  assert.match(modelConfigRs, /pub use sona_local_asr::recognizer::/u);
  assert.doesNotMatch(modelConfigRs, /use sherpa_onnx::/u);
  assert.doesNotMatch(modelConfigRs, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(modelConfigRs, /OnlineRecognizerConfig/u);
  assert.doesNotMatch(modelConfigRs, /pub enum ModelType/u);
  assert.doesNotMatch(modelConfigRs, /impl Recognizer/u);
});

test('local offline transcription reuses local ASR recognizer model construction', () => {
  const offlineRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'offline.rs'),
    'utf8',
  );
  const recognizerRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'),
    'utf8',
  );

  assert.match(offlineRs, /pub struct LocalOfflineAsrAdapter/u);
  assert.match(offlineRs, /impl OfflineTranscriber for LocalOfflineAsrAdapter/u);
  assert.match(offlineRs, /use crate::recognizer::\{[^}]*build_offline_model_config/u);
  assert.match(offlineRs, /decode_offline_samples/u);
  assert.match(offlineRs, /create_offline_recognizer/u);
  assert.match(recognizerRs, /pub fn create_offline_recognizer\([\s\S]*\) -> Result<SafeOfflineRecognizer, String>/u);
  assert.doesNotMatch(offlineRs, /pub async fn run_offline_transcription/u);
  assert.doesNotMatch(offlineRs, /enum ModelType/u);
  assert.doesNotMatch(offlineRs, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(recognizerRs, /\) -> Result<OfflineRecognizer, String>/u);
  assert.doesNotMatch(offlineRs, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(offlineRs, /fn build_model_config/u);
  assert.doesNotMatch(offlineRs, /fn create_recognizer/u);
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
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );
  const batchRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );
  const sherpaRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'sherpa_onnx.rs'),
    'utf8',
  );
  const desktopOnlineRs = `${batchRs}\n${sherpaRs}`;

  assert.match(modelConfigRs, /create_online_stream/u);
  assert.match(modelConfigRs, /accept_online_samples/u);
  assert.match(modelConfigRs, /decode_online_ready/u);
  assert.match(modelConfigRs, /online_stream_result/u);
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
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
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

  assert.match(modelConfigRs, /accept_vad_samples/u);
  assert.match(modelConfigRs, /reset_vad/u);
  assert.match(modelConfigRs, /vad_detected/u);
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
