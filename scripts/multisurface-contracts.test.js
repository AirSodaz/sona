import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8').replace(/\r\n/gu, '\n');
}

function rustSources(...roots) {
  const sources = [];
  const visit = (absolutePath) => {
    for (const entry of fs.readdirSync(absolutePath, { withFileTypes: true })) {
      const child = path.join(absolutePath, entry.name);
      if (entry.isDirectory()) {
        visit(child);
      } else if (entry.isFile() && entry.name.endsWith('.rs')) {
        sources.push({
          relativePath: path.relative(repoRoot, child).split(path.sep).join('/'),
          source: fs.readFileSync(child, 'utf8'),
        });
      }
    }
  };

  for (const root of roots) {
    visit(path.join(repoRoot, root));
  }
  return sources;
}

function withoutInlineRustTests(source) {
  const testModule = source.search(/\n#\[cfg\(test\)\]\s*\nmod tests\s*\{/u);
  return testModule < 0 ? source : source.slice(0, testModule);
}

const coreBackup = read('core', 'src', 'backup', 'model.rs');
const coreSync = read('core', 'src', 'sync', 'model.rs');
const desktopBindings = read(
  'platforms',
  'desktop',
  'frontend',
  'src',
  'bindings.ts',
);
const frontendBackup = read(
  'platforms',
  'desktop',
  'frontend',
  'src',
  'types',
  'backup.ts',
);
const syncConflictCenter = read(
  'platforms',
  'desktop',
  'frontend',
  'src',
  'components',
  'settings',
  'sync',
  'SyncConflictCenter.tsx',
);
const prGuardrails = read('.github', 'workflows', 'pr-guardrails.yml');

test('desktop backup contract follows Rust schema v3 and tags', () => {
  assert.match(coreBackup, /BACKUP_SCHEMA_VERSION:\s*u64\s*=\s*3/u);
  assert.match(frontendBackup, /BACKUP_SCHEMA_VERSION\s*=\s*3\s+as const/u);
  assert.match(
    desktopBindings,
    /export type BackupManifestCounts_Serialize = \{[\s\S]*?\btags: number,/u,
  );
  assert.doesNotMatch(
    desktopBindings,
    /export type BackupManifestCounts_Serialize = \{[\s\S]*?\bprojects: number,/u,
  );
});

test('generated desktop sync entity kinds include the canonical tag variant', () => {
  assert.match(coreSync, /enum SyncEntityKind\s*\{[\s\S]*?\bTag,/u);
  assert.match(
    desktopBindings,
    /export type SyncEntityKind = [^;]*"tag"/u,
  );
});

test('sync conflict UI reads the persisted HLC snake_case timestamp', () => {
  assert.match(syncConflictCenter, /version\.clock\.physical_ms\b/u);
  assert.doesNotMatch(syncConflictCenter, /version\.clock\.physicalMs\b/u);
});

test('PR guardrails reference the renamed WebDAV sync adapter crate', () => {
  assert.match(prGuardrails, /-p sona-sync-webdav\b/u);
  assert.doesNotMatch(prGuardrails, /-p sona-webdav\b/u);
});

test('desktop AppConfig is constrained by the generated Rust contract', () => {
  const configTypes = read(
    'platforms',
    'desktop',
    'frontend',
    'src',
    'types',
    'config.ts',
  );

  assert.match(
    desktopBindings,
    /export type AppConfig = AppConfig_Serialize \| AppConfig_Deserialize/u,
  );
  assert.match(configTypes, /AppConfig as GeneratedAppConfig/u);
  assert.match(configTypes, /GeneratedAppConfig\s*&/u);
  assert.doesNotMatch(configTypes, /Record<string, any>/u);
});

test('Rust-owned Tauri command contracts stay generated and complete', () => {
  const rustRegistry = read('adapters', 'ts_bind', 'src', 'tauri_contracts.rs');
  const tsBind = read('adapters', 'ts_bind', 'src', 'lib.rs');
  const commands = read(
    'platforms',
    'desktop',
    'frontend',
    'src',
    'services',
    'tauri',
    'commands.ts',
  );
  const contracts = read(
    'platforms',
    'desktop',
    'frontend',
    'src',
    'services',
    'tauri',
    'contracts.ts',
  );

  const registryCommands = Array.from(
    rustRegistry.matchAll(/TauriCommandContract::new\(\s*"([^"]+)"/gu),
    (match) => match[1],
  );
  assert.equal(registryCommands.length, 54);
  assert.equal(new Set(registryCommands).size, registryCommands.length);

  const commandGroups = [
    'tag',
    'taskLedger',
    'recovery',
    'automationRepository',
    'automation',
    'history',
  ];
  const frontendCommands = commandGroups.flatMap((group) => {
    const body = new RegExp(`\\n  ${group}: \\{([\\s\\S]*?)\\n  \\},`, 'u')
      .exec(commands)?.[1];
    assert.ok(body, `missing TauriCommand.${group}`);
    return Array.from(body.matchAll(/:\s*'([^']+)'/gu), (match) => match[1]);
  });
  assert.deepEqual(
    [...registryCommands].sort(),
    [...frontendCommands].sort(),
    'Rust metadata and frontend command groups must own the same commands',
  );

  const generatedMap = /export type RustTauriCommandContractMap = \{([\s\S]*?)\n\};/u
    .exec(desktopBindings)?.[1];
  assert.ok(generatedMap, 'generated bindings must contain the Rust-owned map');
  const generatedCommands = Array.from(
    generatedMap.matchAll(/^\s*"([^"]+)":\s*\{/gmu),
    (match) => match[1],
  );
  assert.deepEqual(
    [...registryCommands].sort(),
    [...generatedCommands].sort(),
    'generated bindings must contain every registry command exactly once',
  );

  const manualMap = /type ManualTauriCommandContractMap = \{([\s\S]*?)\n\};\n\nexport type TauriCommandContractMap/u
    .exec(contracts)?.[1];
  assert.ok(manualMap, 'frontend contracts must retain a bounded manual map');
  assert.doesNotMatch(
    manualMap,
    /\[TauriCommand\.(?:project|tag|taskLedger|recovery|automationRepository|automation|history)\./u,
  );
  assert.match(
    contracts,
    /export type TauriCommandContractMap = RustTauriCommandContractMap\s*&\s*ManualTauriCommandContractMap;/u,
  );
  assert.match(
    tsBind,
    /output\.push_str\(&render_rust_tauri_command_contract_map\(\)\);/u,
  );

  for (const [command, args, result] of [
    ['tag_update', '{ tagId: string; updates: TagUpdateInput }', 'TagRecord | null'],
    ['task_ledger_patch_task', '{ id: string; patch: TaskLedgerPatch_Deserialize }', 'TaskLedgerSnapshot_Serialize'],
    ['recovery_save_snapshot', '{ items: RecoveryItemInput_Deserialize[] }', 'RecoverySnapshot_Serialize'],
    ['automation_persist_repository_state', '{ profiles: AutomationProfileInput_Deserialize[]; rules: AutomationRuleInput_Deserialize[]; processedEntries: AutomationProcessedInput_Deserialize[] }', 'void'],
    ['history_update_transcript', 'HistoryUpdateTranscriptRequest_Deserialize', 'HistoryItemRecord'],
  ]) {
    const escapedCommand = command.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
    const entry = new RegExp(
      `TauriCommandContract::new\\(\\s*"${escapedCommand}",\\s*"([^"]+)",\\s*"([^"]+)"`,
      'u',
    ).exec(rustRegistry);
    assert.ok(entry, `missing Rust contract for ${command}`);
    assert.deepEqual(
      { args: entry[1], result: entry[2] },
      { args, result },
      `${command} transport phases drifted`,
    );
  }
});

test('core domain and host ports expose structured errors', () => {
  const structuredErrorFiles = [
    ['core', 'src', 'config', 'repository.rs'],
    ['core', 'src', 'config', 'service.rs'],
    ['core', 'src', 'tag', 'repository.rs'],
    ['core', 'src', 'tag', 'service.rs'],
    ['core', 'src', 'automation', 'repository.rs'],
    ['core', 'src', 'automation', 'service.rs'],
    ['core', 'src', 'recovery', 'repository.rs'],
    ['core', 'src', 'recovery', 'service.rs'],
    ['core', 'src', 'task_ledger', 'repository.rs'],
    ['core', 'src', 'task_ledger', 'service.rs'],
    ['core', 'src', 'ports', 'fs.rs'],
    ['core', 'src', 'ports', 'path.rs'],
    ['core', 'src', 'ports', 'time.rs'],
  ];

  for (const file of structuredErrorFiles) {
    assert.doesNotMatch(
      read(...file),
      /Result\s*<[\s\S]*?,\s*String\s*>/u,
      `${file.join('/')} must use a structured error`,
    );
  }
});

test('ASR, LLM, Event, and Automation system ports preserve structured failures', () => {
  for (const file of ['asr.rs', 'llm.rs', 'event.rs']) {
    assert.doesNotMatch(
      read('core', 'src', 'ports', file),
      /Result\s*<[\s\S]*?,\s*String\s*>/u,
      `core/src/ports/${file} must use a structured error`,
    );
  }

  const automationService = read('core', 'src', 'automation', 'service.rs');
  assert.match(
    automationService,
    /fn\s+path_exists\s*\([^)]*\)\s*->\s*Result\s*<\s*bool\s*,\s*FileSystemError\s*>/u,
  );
  assert.match(
    automationService,
    /fn\s+create_dir_all\s*\([^)]*\)\s*->\s*Result\s*<\s*\(\)\s*,\s*FileSystemError\s*>/u,
  );
});

test('Core runtime and runtime filesystem public APIs preserve structured failures', () => {
  for (const file of ['config.rs', 'file_utils.rs', 'gpu.rs', 'serve.rs']) {
    assert.doesNotMatch(
      read('core', 'src', 'runtime', file),
      /pub\s+(?:async\s+)?fn\s+[^\{;]*?Result\s*<[^\{;]*?,\s*String\s*>/u,
      `core/src/runtime/${file} must use a structured error`,
    );
  }

  assert.doesNotMatch(
    read('adapters', 'runtime_fs', 'src', 'lib.rs'),
    /pub\s+(?:async\s+)?fn\s+[^\{;]*?Result\s*<[^\{;]*?,\s*String\s*>/u,
    'runtime-fs public functions must use structured errors',
  );
});

test('History uses structured failures and injected production time and IDs', () => {
  for (const file of [
    ['core', 'src', 'history', 'item_factory.rs'],
    ['core', 'src', 'history', 'transcript_payload.rs'],
    ['core', 'src', 'history', 'mutation_repository.rs'],
    ['core', 'src', 'history', 'query_repository.rs'],
    ['adapters', 'sqlite', 'src', 'history_fs_utils.rs'],
  ]) {
    assert.doesNotMatch(
      read(...file),
      /pub\s+(?:async\s+)?fn\s+[^\{;]*?Result\s*<[^\{;]*?,\s*String\s*>/u,
      `${file.join('/')} public functions must use structured errors`,
    );
  }

  const historyStore = read('adapters', 'sqlite', 'src', 'history_store.rs');
  assert.doesNotMatch(
    historyStore,
    /(?:Utc|Local)::now\s*\(|\bsync_now_ms\s*\(|(?<!:)\bUuid::new_v4\s*\(/u,
    'SQLite History production paths must use injected clocks and IDs',
  );
  assert.match(
    historyStore,
    /pub\s+fn\s+with_environment\s*\([\s\S]*?Arc<dyn\s+UnixMillisClock>[\s\S]*?Arc<dyn\s+HistoryIdGenerator>/u,
  );
});

test('SQLite Sync receives its clock through the repository factory', () => {
  const syncRepository = read('adapters', 'sqlite', 'src', 'sync_repository.rs');
  assert.doesNotMatch(
    syncRepository,
    /SystemTime|UNIX_EPOCH|\bsync_now_ms\s*\(/u,
    'SQLite Sync must not read the system clock directly',
  );
  assert.match(syncRepository, /clock:\s*Arc<dyn\s+UnixMillisClock>/u);
  assert.match(
    syncRepository,
    /pub\s+fn\s+new\s*\(db:\s*Arc<Database>,\s*clock:\s*Arc<dyn\s+UnixMillisClock>/u,
  );

  assert.match(
    read('platforms', 'desktop', 'src', 'platform', 'sync.rs'),
    /sync_repository_factory\(Arc::new\(SystemClock\)\)/u,
  );
  assert.match(
    read('adapters', 'uniffi_bind', 'src', 'sync_bridge.rs'),
    /sync_repository_factory\(Arc::new\(SystemClock\)\)/u,
  );
});

test('API server preserves typed failures and receives local ASR through the Core port', () => {
  const apiServer = withoutInlineRustTests(
    read('adapters', 'api_server', 'src', 'lib.rs'),
  );

  assert.doesNotMatch(
    apiServer,
    /pub\s+(?:async\s+)?fn\s+[^\{;]*?Result\s*<[^\{;]*?,\s*String\s*>/u,
    'API server public functions must use typed errors',
  );
  assert.doesNotMatch(
    apiServer,
    /sona_local_asr::batch::LocalBatchAsrAdapter/u,
    'API server orchestration must receive the Core BatchTranscriber port',
  );
  assert.match(apiServer, /Arc<dyn\s+BatchTranscriber>/u);
  for (const errorType of [
    'ApiServerPlatformError',
    'ApiServerRuntimeError',
    'ApiServerStartError',
    'ApiServerStopError',
    'ApiServerDashboardError',
  ]) {
    assert.match(
      read('adapters', 'api_server', 'src', 'error.rs'),
      new RegExp(`(?:enum|struct)\\s+${errorType}`, 'u'),
    );
  }
});

test('API server consumes runtime capability ports from host composition roots', () => {
  const desktop = read('platforms', 'desktop', 'src', 'app', 'server.rs');
  const cli = read('platforms', 'cli', 'src', 'serve.rs');
  const adapters = [
    ['media_validator', 'sona_media_detector::MagicNumberMediaFileValidator'],
    ['gpu_availability', 'sona_local_asr::gpu::LocalGpuAvailabilityProvider'],
    ['model_catalog', 'sona_runtime_fs::RuntimeModelCatalogProvider'],
    ['batch_plan_resolver', 'sona_runtime_fs::RuntimeBatchTranscribePlanResolver'],
  ];

  for (const [field, adapter] of adapters) {
    const composition = new RegExp(
      `${field}:\\s*Arc::new\\(\\s*${adapter.replaceAll('.', '\\.') }\\s*,?\\s*\\)`,
      'gu',
    );
    assert.equal(
      Array.from(desktop.matchAll(composition)).length,
      2,
      `Desktop must compose ${field} in both API server start paths`,
    );
    assert.equal(
      Array.from(cli.matchAll(composition)).length,
      1,
      `CLI must compose ${field} in its API server start path`,
    );
  }

  assert.doesNotMatch(desktop, /default_info_response/u);
});

test('API server depends only on Core runtime capability ports', () => {
  const runtimePorts = read('core', 'src', 'ports', 'runtime.rs');
  const apiServerManifest = read('adapters', 'api_server', 'Cargo.toml');
  const apiServerSource = withoutInlineRustTests(
    read('adapters', 'api_server', 'src', 'lib.rs'),
  );

  for (const port of [
    'MediaFileValidator',
    'GpuAvailabilityProvider',
    'ModelCatalogProvider',
    'BatchTranscribePlanResolver',
  ]) {
    assert.match(runtimePorts, new RegExp(`pub\\s+trait\\s+${port}\\b`, 'u'));
    assert.match(apiServerSource, new RegExp(`Arc<dyn\\s+${port}>`, 'u'));
  }

  for (const dependency of [
    'sona-local-asr',
    'sona-media-detector',
    'sona-runtime-fs',
  ]) {
    assert.doesNotMatch(apiServerManifest, new RegExp(dependency, 'u'));
  }
  for (const moduleName of [
    'sona_local_asr',
    'sona_media_detector',
    'sona_runtime_fs',
  ]) {
    assert.doesNotMatch(apiServerSource, new RegExp(moduleName, 'u'));
  }
});

test('desktop and UniFFI host sync through the shared application layer', () => {
  const hosts = [
    read('platforms', 'desktop', 'src', 'platform', 'sync.rs'),
    read('adapters', 'uniffi_bind', 'src', 'sync_bridge.rs'),
  ];
  const lowLevelCalls = [
    'create_remote_vault',
    'open_remote_vault_with_password',
    'open_remote_vault_with_recovery_key',
    'open_remote_vault_with_vault_key',
    'run_sync_cycle',
    'load_remote_state_for_join',
  ];

  for (const source of hosts) {
    assert.match(source, /\bSyncApplication\b/u);
    for (const call of lowLevelCalls) {
      assert.doesNotMatch(source, new RegExp(`\\b${call}\\b`, 'u'));
    }
    assert.doesNotMatch(
      source,
      /\bstruct\s+(?:UnlockedSession|Session|PersistedSyncConfig|PersistedConfig)\b/u,
    );
  }
});

test('Desktop Sync lifecycle requests are provider-neutral behind WebDAV compatibility', () => {
  const syncApplication = read('adapters', 'sync', 'src', 'application.rs');
  const desktopSync = read('platforms', 'desktop', 'src', 'platform', 'sync.rs');
  const desktopCommands = read('platforms', 'desktop', 'src', 'commands', 'sync.rs');
  const frontendCommands = read(
    'platforms',
    'desktop',
    'frontend',
    'src',
    'services',
    'tauri',
    'commands.ts',
  );
  const frontendSync = read(
    'platforms',
    'desktop',
    'frontend',
    'src',
    'services',
    'tauri',
    'sync.ts',
  );

  assert.match(
    syncApplication,
    /#\[derive\([^\]]*Deserialize[^\]]*Serialize[^\]]*\)\]\s*#\[serde\(rename_all = "camelCase"\)\]\s*pub struct SyncProviderInput/u,
  );
  for (const request of [
    'SyncCreateRequest',
    'SyncPreviewJoinRequest',
    'SyncJoinRequest',
  ]) {
    assert.match(
      desktopSync,
      new RegExp(`pub struct ${request}\\s*\\{\\s*pub provider: SyncProviderInput,`, 'u'),
    );
  }
  assert.match(
    desktopSync,
    /pub async fn test_provider<[^>]+>\([^)]*provider:\s*SyncProviderInput/su,
  );
  assert.doesNotMatch(desktopSync, /pub async fn test_webdav_provider\b/u);

  assert.match(desktopCommands, /pub async fn sync_test_provider\b/u);
  assert.match(desktopCommands, /pub async fn sync_test_webdav_provider\b/u);
  assert.match(desktopCommands, /webdav_provider_input\(config\)/u);
  assert.match(frontendCommands, /testProvider:\s*'sync_test_provider'/u);
  assert.match(frontendCommands, /testWebDavProvider:\s*'sync_test_webdav_provider'/u);
  assert.match(frontendSync, /TauriCommand\.sync\.testProvider/u);
  assert.doesNotMatch(frontendSync, /TauriCommand\.sync\.testWebDavProvider/u);
  assert.match(
    frontendSync,
    /providerId:\s*'webdav',[\s\S]*configuration/u,
  );
});

test('UniFFI Sync lifecycle JSON is provider-neutral behind WebDAV compatibility', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const syncBridge = read('adapters', 'uniffi_bind', 'src', 'sync_bridge.rs');

  assert.match(
    syncBridge,
    /#\[serde\(untagged\)\]\s*enum ProviderInputWire\s*\{[\s\S]*Canonical\(SyncProviderInput\),[\s\S]*LegacyWebDav\(WebDavObjectStoreConfig\),[\s\S]*\}/u,
  );
  for (const request of ['CreateRequest', 'JoinRequest']) {
    assert.match(
      syncBridge,
      new RegExp(`struct ${request}\\s*\\{\\s*provider: ProviderInputWire,`, 'u'),
    );
  }
  assert.match(
    syncBridge,
    /fn parse_provider_input_json\([^)]*\)\s*->\s*SonaCoreBindingResult<SyncProviderInput>/su,
  );
  assert.match(
    syncBridge,
    /pub\(crate\) async fn test_provider_json\([^)]*\)[\s\S]*parse_provider_input_json\(&config_json/u,
  );
  assert.doesNotMatch(
    syncBridge,
    /provider_input\(request\.provider\)/u,
  );
  assert.match(
    binding,
    /pub async fn sync_test_provider_json\(config_json: String\)\s*->\s*SonaCoreBindingResult<String>/u,
  );
});

test('Android registers its secure sync secret store with the UniFFI binding', () => {
  const container = read(
    'platforms',
    'android',
    'client',
    'app',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'android',
    'app',
    'composition',
    'SonaAppContainer.kt',
  );

  assert.match(container, /AndroidSyncSecretStore\.create\(appContext\)/u);
  assert.match(container, /UniffiSyncSecretStoreRegistrar\(\)/u);
  assert.match(container, /register\(appDataDir, syncSecretStore\)/u);
});

test('UniFFI owns Sync secrets and cache lifetime per canonical application context', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const applicationContext = read('adapters', 'uniffi_bind', 'src', 'application_context.rs');
  const syncBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'sync_bridge.rs'),
  );
  const secretStoreBridge = read(
    'adapters',
    'uniffi_bind',
    'src',
    'sync_secret_store_bridge.rs',
  );
  const androidRegistrar = read(
    'platforms',
    'android',
    'client',
    'adapters',
    'uniffi',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'android',
    'adapters',
    'uniffi',
    'sync',
    'UniffiSyncSecretStoreAdapter.kt',
  );
  const syncApplicationFactory = syncBridge.match(
    /fn application\(app_data_dir: &str\)[\s\S]*?(?=\nfn provider_registry)/u,
  )?.[0];

  assert.match(applicationContext, /const DEFAULT_CONTEXT_CACHE_CAPACITY: usize = 8/u);
  assert.match(
    applicationContext,
    /sync_secret_store: Arc<HostSyncSecretStore>/u,
  );
  assert.match(
    applicationContext,
    /sync_secret_store_overrides: HashMap<PathBuf, Arc<dyn FfiSyncSecretStore>>/u,
  );
  assert.match(
    applicationContext,
    /sync_secret_store_overrides[\s\S]*\.get\(&key\)[\s\S]*\.or_else\(\|\| self\.default_sync_secret_store\.clone\(\)\)/u,
  );
  assert.match(
    applicationContext,
    /if !self\.sync_secret_store_overrides\.contains_key\(path\)[\s\S]*register_sync_secret_store/u,
  );
  assert.match(
    applicationContext,
    /SqliteApplicationContext::normalize_writable_app_data_dir\(app_data_dir\)/u,
  );
  assert.match(
    applicationContext,
    /pub\(crate\) fn release_application_context\([\s\S]*Result<bool, DatabaseError>[\s\S]*\.release\(app_data_dir\.as_ref\(\)\)/u,
  );
  assert.match(
    applicationContext,
    /normalize_existing_app_data_dir\(app_data_dir\)\?[\s\S]*entries\.remove\(&key\)/u,
  );
  assert.match(
    applicationContext,
    /sync_secret_store_overrides\.remove\(&key\)/u,
  );
  assert.match(
    applicationContext,
    /!cached\.context\.has_active_sync_handle\(\)/u,
  );
  assert.match(secretStoreBridge, /registration: RwLock<Option<Arc<dyn FfiSyncSecretStore>>>/u);
  assert.doesNotMatch(secretStoreBridge, /\b(?:static|OnceLock)\b/u);
  assert.match(syncBridge, /let secret_store: Arc<dyn SyncSecretStore> = context\.sync_secret_store\(\)/u);
  assert.ok(syncApplicationFactory, 'canonical Sync application factory must remain explicit');
  assert.doesNotMatch(syncApplicationFactory, /\bWebDav\w*\b/u);

  assert.match(binding, /pub fn register_sync_secret_store_for_app_data_dir\b/u);
  assert.match(
    binding,
    /#\[uniffi::export\]\s*pub fn release_application_context\(app_data_dir: String\) -> SonaCoreBindingResult<bool>/u,
  );
  assert.match(androidRegistrar, /registerSyncSecretStoreForAppDataDir/u);
  assert.match(
    androidRegistrar,
    /fun register\(appDataDir: String, store: SyncSecretStorePort\)/u,
  );
  assert.doesNotMatch(androidRegistrar, /\bregisterSyncSecretStore\b/u);
});

test('UniFFI tests own application context and History environments', () => {
  const applicationContext = read(
    'adapters',
    'uniffi_bind',
    'src',
    'application_context.rs',
  );
  const syncBridge = read('adapters', 'uniffi_bind', 'src', 'sync_bridge.rs');
  const historyFixtures = [
    read('adapters', 'uniffi_bind', 'src', 'backup_bridge.rs'),
    read('adapters', 'uniffi_bind', 'src', 'dashboard_bridge.rs'),
  ].join('\n');

  assert.doesNotMatch(applicationContext, /clear_application_contexts_for_tests/u);
  assert.doesNotMatch(syncBridge, /clear_application_contexts_for_tests/u);
  assert.doesNotMatch(historyFixtures, /SqliteHistoryStore::new\s*\(/u);
});

test('new production code cannot consume the removed Project API', () => {
  const compatibilityUse = /\b(?:sona_core::project|SqliteProject(?:Adapter|Repository)|Project(?:Store|RepositoryService|Record))\b/u;

  for (const { relativePath, source } of rustSources('adapters', 'platforms')) {
    if (!relativePath.includes('/tests/')) {
      assert.doesNotMatch(
        source,
        compatibilityUse,
        `${relativePath} must use the canonical Tag API`,
      );
    }
  }
});

test('hosts reuse the shared SQLite application context', () => {
  const uniffiBridges = [
    'app_config_repository_bridge.rs',
    'automation_bridge.rs',
    'backup_bridge.rs',
    'history_mutation_bridge.rs',
    'history_query_bridge.rs',
    'sync_bridge.rs',
    'tag_bridge.rs',
    'task_ledger_bridge.rs',
  ];
  for (const file of uniffiBridges) {
    const source = withoutInlineRustTests(read('adapters', 'uniffi_bind', 'src', file));
    assert.match(source, /\bapplication_context\b/u, `${file} must use the host context`);
    assert.doesNotMatch(source, /\bDatabase::open(?:_read_only)?\b/u);
    assert.doesNotMatch(source, /\bLazySqlite\w+\b/u);
  }

  const desktopSetup = read('platforms', 'desktop', 'src', 'app', 'setup.rs');
  const desktopDatabase = read('platforms', 'desktop', 'src', 'platform', 'database.rs');
  assert.match(desktopSetup, /\bSqliteApplicationContext::from_database\b/u);
  assert.match(desktopSetup, /manage\(sqlite_context\)/u);
  assert.doesNotMatch(desktopSetup, /manage\(db\)/u);
  assert.doesNotMatch(desktopDatabase, /\bDatabase::(?:global|set_global)\b/u);

  for (const file of ['app_config.rs', 'automation.rs', 'backup.rs', 'history.rs', 'task_ledger.rs']) {
    const source = withoutInlineRustTests(read('platforms', 'cli', 'src', file));
    assert.match(source, /\bSqliteApplicationContext\b/u, `${file} must use the CLI context`);
    assert.doesNotMatch(source, /\bDatabase::open(?:_read_only)?\b/u);
    assert.doesNotMatch(source, /\bLazySqlite\w+\b/u);
  }
});

test('UniFFI exposes versioned typed Tag contracts without extending legacy Project', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const facade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const tagBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'tag_bridge.rs'),
  );
  const tagMapper = read(
    'adapters',
    'uniffi_bind',
    'src',
    'mapper',
    'tag_mapper.rs',
  );

  for (const typeName of [
    'FfiTagCreateInputV1',
    'FfiTagRecordV1',
    'FfiTagRepositorySnapshotV1',
    'FfiTagUpdateInputV1',
  ]) {
    assert.match(tagMapper, new RegExp(`struct\\s+${typeName}\\b`, 'u'));
  }
  assert.doesNotMatch(tagMapper, /\bFfiTagDefaults\w*V1\b|\bdefaults\s*:/u);
  assert.doesNotMatch(tagMapper, /serde_json|Value/u);

  for (const functionName of [
    'load_tag_repository_v1',
    'replace_tags_v1',
    'create_tag_v1',
    'update_tag_v1',
    'delete_tag_v1',
    'reorder_tags_v1',
    'set_active_tag_id_v1',
  ]) {
    const exportedFunction = new RegExp(
      `#\\[uniffi::export\\]\\s*pub\\s+fn\\s+${functionName}\\b`,
      'u',
    );
    assert.match(binding, exportedFunction);
    assert.match(facade, new RegExp(`pub\\s+fn\\s+${functionName}\\b`, 'u'));
    assert.match(tagBridge, new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'));
  }

  const projectSurface = `${binding}\n${facade}`;
  assert.doesNotMatch(projectSurface, /FfiProject\w*V1|\b\w*project\w*_v1\b/iu);
});

test('UniFFI exposes typed History V1 contracts and Android consumes them without JSON', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const facade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const queryBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'history_query_bridge.rs'),
  );
  const mutationBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'history_mutation_bridge.rs'),
  );
  const historyMapper = read(
    'adapters',
    'uniffi_bind',
    'src',
    'mapper',
    'history_mapper.rs',
  );
  const androidBindings = read(
    'platforms',
    'android',
    'client',
    'adapters',
    'uniffi',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'android',
    'adapters',
    'uniffi',
    'recording',
    'UniffiRecordingBindings.kt',
  );
  const androidHistory = read(
    'platforms',
    'android',
    'client',
    'adapters',
    'uniffi',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'android',
    'adapters',
    'uniffi',
    'recording',
    'UniffiRecordingHistoryAdapter.kt',
  );

  for (const enumName of [
    'FfiHistoryItemKindV1',
    'FfiHistoryItemStatusV1',
    'FfiHistoryAudioStatusV1',
    'FfiHistoryDraftSourceV1',
    'FfiHistoryDraftSourcePatchV1',
    'FfiTranscriptSnapshotReasonV1',
    'FfiHistoryWorkspaceScopeV1',
    'FfiHistoryWorkspaceFilterTypeV1',
    'FfiHistoryWorkspaceDateFilterV1',
    'FfiHistoryWorkspaceSortOrderV1',
  ]) {
    assert.match(historyMapper, new RegExp(`enum\\s+${enumName}\\b`, 'u'));
  }
  for (const typeName of [
    'FfiHistoryItemRecordV1',
    'FfiHistoryCreateLiveDraftRequestV1',
    'FfiHistoryCompleteLiveDraftRequestV1',
    'FfiHistoryUpdateTranscriptRequestV1',
    'FfiHistoryDeleteItemsRequestV1',
    'FfiHistorySaveRecordingRequestV1',
    'FfiHistorySaveImportedFileRequestV1',
    'FfiHistoryTrashItemsRequestV1',
    'FfiHistoryCreateTranscriptSnapshotRequestV1',
    'FfiHistoryItemMetaPatchV1',
    'FfiHistoryUpdateItemMetaRequestV1',
    'FfiHistoryUpdateTagAssignmentsRequestV1',
    'FfiHistoryReplaceTagAssignmentsRequestV1',
    'FfiTranscriptSnapshotMetadataV1',
    'FfiTranscriptSnapshotRecordV1',
    'FfiHistoryWorkspaceQueryRequestV1',
    'FfiHistoryWorkspaceSearchRangeV1',
    'FfiHistoryWorkspaceSearchSnippetV1',
    'FfiHistoryWorkspaceItemSearchMatchV1',
    'FfiHistorySearchMatchEntryV1',
    'FfiHistoryWorkspaceSummaryV1',
    'FfiHistoryTagCountEntryV1',
    'FfiHistoryWorkspaceItemCountsV1',
    'FfiHistoryWorkspaceQueryResultV1',
    'FfiLiveRecordingDraftResultV1',
  ]) {
    assert.match(historyMapper, new RegExp(`struct\\s+${typeName}\\b`, 'u'));
  }
  assert.doesNotMatch(historyMapper, /serde_json|\bValue\b/u);

  const queryFunctions = [
    'list_history_items_v1',
    'query_history_workspace_v1',
    'load_history_transcript_v1',
    'list_history_transcript_snapshots_v1',
    'load_history_transcript_snapshot_v1',
  ];
  const mutationFunctions = [
    'create_history_live_draft_v1',
    'complete_history_live_draft_v1',
    'save_history_recording_v1',
    'save_history_imported_file_v1',
    'trash_history_items_v1',
    'restore_history_items_v1',
    'purge_history_items_v1',
    'update_history_transcript_v1',
    'create_history_transcript_snapshot_v1',
    'update_history_item_meta_v1',
    'update_history_tag_assignments_v1',
    'replace_history_tag_assignments_v1',
  ];
  for (const functionName of [...queryFunctions, ...mutationFunctions]) {
    assert.match(
      binding,
      new RegExp(`#\\[uniffi::export[^\\]]*\\]\\s*pub\\s+async\\s+fn\\s+${functionName}\\b`, 'u'),
    );
    assert.match(facade, new RegExp(`pub\\s+async\\s+fn\\s+${functionName}\\b`, 'u'));
  }
  for (const functionName of queryFunctions) {
    assert.match(
      queryBridge,
      new RegExp(`pub\\(crate\\)\\s+async\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
  for (const functionName of mutationFunctions) {
    assert.match(
      mutationBridge,
      new RegExp(`pub\\(crate\\)\\s+async\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }

  for (const kotlinFunction of [
    'createHistoryLiveDraftV1',
    'updateHistoryTranscriptV1',
    'completeHistoryLiveDraftV1',
    'purgeHistoryItemsV1',
    'queryHistoryWorkspaceV1',
    'loadHistoryTranscriptV1',
  ]) {
    assert.match(androidBindings, new RegExp(`\\b${kotlinFunction}\\b`, 'u'));
  }
  assert.doesNotMatch(
    `${androidBindings}\n${androidHistory}`,
    /(?:createHistoryLiveDraft|updateHistoryTranscript|completeHistoryLiveDraft|purgeHistoryItems|queryHistoryWorkspace|loadHistoryTranscript)Json/u,
  );
  assert.doesNotMatch(androidHistory, /kotlinx\.serialization\.json|buildJsonObject|parseJson/u);
  assert.doesNotMatch(
    `${binding}\n${facade}\n${historyMapper}`,
    /FfiProject\w*V1|\b\w*project\w*_v1\b|\bdelete_history_items_v1\b/iu,
  );
});

test('UniFFI exposes versioned typed Task Ledger contracts with tri-state patches', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const facade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const taskLedgerBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'task_ledger_bridge.rs'),
  );
  const taskLedgerMapper = read(
    'adapters',
    'uniffi_bind',
    'src',
    'mapper',
    'task_ledger_mapper.rs',
  );

  for (const enumName of [
    'FfiTaskLedgerKindV1',
    'FfiTaskLedgerStatusV1',
    'FfiStringPatchV1',
  ]) {
    assert.match(taskLedgerMapper, new RegExp(`enum\\s+${enumName}\\b`, 'u'));
  }
  for (const typeName of [
    'FfiTaskLedgerRecordV1',
    'FfiTaskLedgerPatchV1',
    'FfiTaskLedgerSnapshotV1',
  ]) {
    assert.match(taskLedgerMapper, new RegExp(`struct\\s+${typeName}\\b`, 'u'));
  }
  assert.match(
    taskLedgerMapper,
    /enum\s+FfiStringPatchV1\s*\{[^}]*Unchanged[^}]*Clear[^}]*Set\s*\{\s*value:\s*String\s*\}/su,
  );
  assert.doesNotMatch(taskLedgerMapper, /serde_json|\bValue\b/u);

  for (const functionName of [
    'load_task_ledger_snapshot_v1',
    'upsert_task_ledger_record_v1',
    'patch_task_ledger_record_v1',
    'remove_task_ledger_record_v1',
    'clear_resolved_task_ledger_records_v1',
  ]) {
    const exportedFunction = new RegExp(
      `#\\[uniffi::export\\]\\s*pub\\s+fn\\s+${functionName}\\b`,
      'u',
    );
    assert.match(binding, exportedFunction);
    assert.match(facade, new RegExp(`pub\\s+fn\\s+${functionName}\\b`, 'u'));
    assert.match(
      taskLedgerBridge,
      new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
});

test('UniFFI exposes typed Recovery V1 records with JSON limited to dynamic leaves', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const facade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const recoveryBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'recovery_bridge.rs'),
  );
  const recoveryMapper = read(
    'adapters',
    'uniffi_bind',
    'src',
    'mapper',
    'recovery_mapper.rs',
  );

  for (const enumName of [
    'FfiRecoverySourceV1',
    'FfiRecoveryResolutionV1',
    'FfiRecoveryItemStageV1',
    'FfiRecoveryQueueStatusV1',
  ]) {
    assert.match(recoveryMapper, new RegExp(`enum\\s+${enumName}\\b`, 'u'));
  }
  for (const typeName of [
    'FfiRecoveryFileStatV1',
    'FfiRecoveredTranscriptTimingUnitV1',
    'FfiRecoveredTranscriptTimingV1',
    'FfiRecoveredTranscriptSegmentV1',
    'FfiRecoveryItemInputV1',
    'FfiRecoveredQueueItemV1',
    'FfiRecoverySnapshotV1',
  ]) {
    assert.match(recoveryMapper, new RegExp(`struct\\s+${typeName}\\b`, 'u'));
  }

  for (const dynamicLeaf of [
    'resolved_config_snapshot_json',
    'export_config_json',
    'stage_config_json',
  ]) {
    assert.match(recoveryMapper, new RegExp(`pub\\s+${dynamicLeaf}:`, 'u'));
  }
  assert.doesNotMatch(
    recoveryMapper,
    /pub\s+(?:snapshot|items|queue_items|segments)_json\s*:/u,
  );

  for (const functionName of [
    'load_recovery_snapshot_v1',
    'save_recovery_snapshot_v1',
    'persist_recovery_queue_snapshot_v1',
  ]) {
    const exportedFunction = new RegExp(
      `#\\[uniffi::export\\]\\s*pub\\s+fn\\s+${functionName}\\b`,
      'u',
    );
    assert.match(binding, exportedFunction);
    assert.match(facade, new RegExp(`pub\\s+fn\\s+${functionName}\\b`, 'u'));
    assert.match(
      recoveryBridge,
      new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
  assert.match(
    binding,
    /pub\s+fn\s+save_recovery_snapshot_v1\s*\([^)]*items:\s*Vec<FfiRecoveryItemInputV1>/su,
  );
  assert.match(
    binding,
    /pub\s+fn\s+persist_recovery_queue_snapshot_v1\s*\([^)]*queue_items:\s*Vec<FfiRecoveryItemInputV1>/su,
  );
});

test('UniFFI exposes typed Automation V1 repository and Tag-based validation contracts', () => {
  const binding = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const facade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const automationBridge = withoutInlineRustTests(
    read('adapters', 'uniffi_bind', 'src', 'automation_bridge.rs'),
  );
  const automationMapper = read(
    'adapters',
    'uniffi_bind',
    'src',
    'mapper',
    'automation_mapper.rs',
  );

  for (const typeName of [
    'FfiAutomationStageConfigV1',
    'FfiAutomationExportConfigV1',
    'FfiAutomationRuleInputV1',
    'FfiAutomationProcessedInputV1',
    'FfiAutomationRepositoryInputV1',
    'FfiAutomationRuleRecordV1',
    'FfiAutomationProcessedRecordV1',
    'FfiAutomationRepositoryStateV1',
    'FfiAutomationValidationStageConfigV1',
    'FfiAutomationValidationExportConfigV1',
    'FfiAutomationValidationRuleV1',
    'FfiAutomationTagReferenceV1',
    'FfiAutomationRuleValidationResultV1',
  ]) {
    assert.match(automationMapper, new RegExp(`struct\\s+${typeName}\\b`, 'u'));
  }
  assert.doesNotMatch(automationMapper, /serde_json|\bValue\b|_json\s*:/u);

  for (const functionName of [
    'load_automation_repository_state_v1',
    'replace_automation_rules_v1',
    'replace_automation_processed_entries_v1',
    'replace_automation_repository_state_v1',
    'validate_automation_rule_activation_v1',
  ]) {
    const exportedFunction = new RegExp(
      `#\\[uniffi::export\\]\\s*pub\\s+fn\\s+${functionName}\\b`,
      'u',
    );
    assert.match(binding, exportedFunction);
    assert.match(facade, new RegExp(`pub\\s+fn\\s+${functionName}\\b`, 'u'));
    assert.match(
      automationBridge,
      new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
  assert.match(
    binding,
    /pub\s+fn\s+validate_automation_rule_activation_v1\s*\([^)]*rule:\s*FfiAutomationValidationRuleV1[^)]*global_config_json:\s*String[^)]*tags:\s*Vec<FfiAutomationTagReferenceV1>/su,
  );
  assert.doesNotMatch(
    `${binding}\n${facade}`,
    /FfiProject\w*V1|\b\w*project\w*_v1\b/iu,
  );
});
