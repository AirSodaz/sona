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

// The UniFFI binding is a two-layer surface: `#[uniffi::export]` in lib.rs
// delegates straight into a `*_bridge` module. There is no intermediate facade
// type; see the "no intermediate facade layer" test below.
function assertBindingDelegatesToBridge(binding, functionName, bridgeModule) {
  const delegation = new RegExp(
    `#\\[uniffi::export[^\\]]*\\]\\s*pub\\s+(?:async\\s+)?fn\\s+${functionName}\\s*\\([^{]*\\{\\s*${bridgeModule}::`,
    'u',
  );
  assert.match(
    binding,
    delegation,
    `#[uniffi::export] ${functionName} must delegate directly to ${bridgeModule}`,
  );
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

test('PR guardrails execute both sync application and WebDAV adapter tests', () => {
  assert.match(prGuardrails, /-p sona-sync(?=\s|$)/u);
  assert.match(prGuardrails, /-p sona-sync-webdav\b/u);
  assert.doesNotMatch(prGuardrails, /-p sona-webdav\b/u);
});

test('desktop streaming context stays typed across the injected route', () => {
  const apiServerPlatform = read('adapters', 'api_server', 'src', 'platform.rs');
  const desktopServer = read('platforms', 'desktop', 'src', 'app', 'server.rs');
  const desktopStreaming = read('platforms', 'desktop', 'src', 'integrations', 'streaming.rs');

  assert.doesNotMatch(apiServerPlatform, /\bdyn Any\b|std::any::Any/u);
  assert.doesNotMatch(desktopStreaming, /Arc::downcast|unexpected type/u);
  assert.match(desktopServer, /\.layer\(axum::Extension\(streaming_context\)\)/u);
  assert.match(
    desktopStreaming,
    /Extension\(context\): Extension<Arc<TauriStreamingContext>>/u,
  );
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
  assert.equal(registryCommands.length, 55);
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
    ['history_commit_transcript_edit', 'HistoryCommitTranscriptEditRequest_Deserialize', 'HistoryCommitTranscriptEditResult_Serialize'],
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
    ['application', 'src', 'config', 'service.rs'],
    ['core', 'src', 'tag', 'repository.rs'],
    ['application', 'src', 'tag', 'service.rs'],
    ['core', 'src', 'automation', 'repository.rs'],
    ['application', 'src', 'automation', 'service.rs'],
    ['core', 'src', 'recovery', 'repository.rs'],
    ['application', 'src', 'recovery', 'service.rs'],
    ['core', 'src', 'task_ledger', 'repository.rs'],
    ['application', 'src', 'task_ledger', 'service.rs'],
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

  const automationPorts = read('core', 'src', 'automation', 'model.rs');
  assert.match(
    automationPorts,
    /fn\s+path_exists\s*\([^)]*\)\s*->\s*Result\s*<\s*bool\s*,\s*FileSystemError\s*>/u,
  );
  assert.match(
    automationPorts,
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

  // history_store is a directory module; production store lives in store.rs.
  const historyStore = read('adapters', 'sqlite', 'src', 'history_store', 'store.rs');
  assert.doesNotMatch(
    historyStore,
    /(?:Utc|Local)::now\s*\(|\bsync_now_ms\s*\(|(?<!:)\bUuid::new_v4\s*\(/u,
    'SQLite History production paths must use injected clocks and IDs',
  );
  assert.match(
    historyStore,
    /pub\s+fn\s+with_environment\s*\([\s\S]*?Arc<dyn\s+ClockPort>[\s\S]*?Arc<dyn\s+HistoryIdGenerator>/u,
  );
});

test('SQLite Sync receives its clock through the repository factory', () => {
  const syncRepository = read('adapters', 'sqlite', 'src', 'sync_repository.rs');
  assert.doesNotMatch(
    syncRepository,
    /SystemTime|UNIX_EPOCH|\bsync_now_ms\s*\(/u,
    'SQLite Sync must not read the system clock directly',
  );
  assert.match(syncRepository, /clock:\s*Arc<dyn\s+ClockPort>/u);
  assert.match(
    syncRepository,
    /pub\s+fn\s+new\s*\(db:\s*Arc<Database>,\s*clock:\s*Arc<dyn\s+ClockPort>/u,
  );

  assert.match(
    read('platforms', 'desktop', 'src', 'platform', 'sync.rs'),
    /sync_repository_factory\(Arc::new\(SystemClock\)\)/u,
  );
  assert.match(
    read('platforms', 'uniffi', 'src', 'sync_bridge.rs'),
    /sync_repository_factory\(Arc::new\(SystemClock\)\)/u,
  );
});

test('API server preserves typed failures and receives local ASR through the Core port', () => {
  const apiServer = [
    read('adapters', 'api_server', 'src', 'lib.rs'),
    read('adapters', 'api_server', 'src', 'state.rs'),
    read('adapters', 'api_server', 'src', 'worker.rs'),
  ].map(withoutInlineRustTests).join('\n');

  assert.doesNotMatch(
    apiServer,
    /pub\s+(?:async\s+)?fn\s+[^\{;]*?Result\s*<[^\{;]*?,\s*String\s*>/u,
    'API server public functions must use typed errors',
  );
  assert.doesNotMatch(
    apiServer,
    /sona_local_asr::batch::LocalBatchAsrAdapter/u,
    'API server orchestration must receive the Core BatchTranscriberPort port',
  );
  assert.match(apiServer, /Arc<dyn\s+BatchTranscriberPort>/u);
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
  const apiServerSource = [
    read('adapters', 'api_server', 'src', 'lib.rs'),
    read('adapters', 'api_server', 'src', 'state.rs'),
    read('adapters', 'api_server', 'src', 'worker.rs'),
  ].map(withoutInlineRustTests).join('\n');

  for (const port of [
    'MediaValidatorPort',
    'GpuAvailabilityPort',
    'ModelCatalogPort',
    'BatchTranscribePlanPort',
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
    read('platforms', 'uniffi', 'src', 'sync_bridge.rs'),
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
  const syncApplication = read('application', 'sync', 'src', 'application.rs');
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
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const syncBridge = read('platforms', 'uniffi', 'src', 'sync_bridge.rs');

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
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const applicationContext = read('platforms', 'uniffi', 'src', 'application_context.rs');
  const syncBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'sync_bridge.rs'),
  );
  const secretStoreBridge = read(
    'platforms',
    'uniffi',
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
  // The factory takes a `ContextSource` rather than a directory string: bridges
  // resolve an explicit context instead of reaching into the registry.
  const syncApplicationFactory = syncBridge.match(
    /fn application\(context: impl Into<ContextSource>\)[\s\S]*?(?=\nfn provider_registry)/u,
  )?.[0];

  assert.match(applicationContext, /const DEFAULT_CONTEXT_CACHE_CAPACITY: usize = 8/u);
  assert.match(
    applicationContext,
    /sync_secret_store: Arc<HostSyncSecretStore>/u,
  );
  assert.match(
    applicationContext,
    /sync_secret_store_overrides: HashMap<PathBuf, Arc<dyn FfiSecretStore>>/u,
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
  assert.match(secretStoreBridge, /registration: RwLock<Option<Arc<dyn FfiSecretStore>>>/u);
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
    'platforms',
    'uniffi',
    'src',
    'application_context.rs',
  );
  const syncBridge = read('platforms', 'uniffi', 'src', 'sync_bridge.rs');
  const historyFixtures = [
    read('platforms', 'uniffi', 'src', 'backup_bridge.rs'),
    read('platforms', 'uniffi', 'src', 'dashboard_bridge.rs'),
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

test('stateful hosts reuse SQLite while the CLI stays stateless', () => {
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
    const source = withoutInlineRustTests(read('platforms', 'uniffi', 'src', file));
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

  const cliManifest = read('platforms', 'cli', 'Cargo.toml');
  assert.doesNotMatch(cliManifest, /\bsona-sqlite\b/u);
  for (const { relativePath, source } of rustSources('platforms/cli/src')) {
    assert.doesNotMatch(
      withoutInlineRustTests(source),
      /\b(?:SqliteApplicationContext|sona_sqlite|LazySqlite\w+)\b/u,
      `${relativePath} must preserve the stateless CLI boundary`,
    );
  }
});

test('per-call reopening SQLite repositories stay behind the test-support feature', () => {
  // `LazySqlite*` repositories run a full `Database::open` (connection pool,
  // migrations, optimize) on every method call. They exist for focused port
  // tests only; hosts must inject a shared Database/SqliteApplicationContext.
  // The feature gate makes host misuse a compile error rather than a review
  // catch, so guard the gate itself.
  const sqliteManifest = read('adapters', 'sqlite', 'Cargo.toml');
  assert.match(
    sqliteManifest,
    /^\[features\]$/mu,
    'sona-sqlite must declare a [features] table',
  );
  assert.match(
    sqliteManifest,
    /^test-support = \[\]$/mu,
    'sona-sqlite must declare the test-support feature',
  );

  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const libLines = sqliteLib.split('\n');
  const gate = /#\[cfg\(any\(test, feature = "test-support"\)\)\]/u;
  let gatedLazyDeclarations = 0;
  libLines.forEach((line, index) => {
    if (!/\bLazySqlite\w+\b/u.test(line)) {
      return;
    }
    gatedLazyDeclarations += 1;
    assert.match(
      libLines[index - 1] ?? '',
      gate,
      `LazySqlite export must be feature-gated: ${line.trim()}`,
    );
  });
  assert.ok(
    gatedLazyDeclarations > 0,
    'expected at least one gated LazySqlite export to guard',
  );

  // No host may turn the feature on, directly or transitively.
  for (const host of [
    ['platforms', 'desktop'],
    ['platforms', 'cli'],
    ['platforms', 'uniffi'],
  ]) {
    const manifest = read(...host, 'Cargo.toml');
    const production = manifest.split(/^\[dev-dependencies\]$/mu)[0];
    assert.doesNotMatch(
      production,
      /test-support/u,
      `${host.join('/')} must not enable sona-sqlite test-support in production dependencies`,
    );
  }
});

test('UniFFI exposes versioned typed Tag contracts without extending legacy Project', () => {
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const tagBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'tag_bridge.rs'),
  );
  const tagMapper = read(
    'platforms',
    'uniffi',
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
    assertBindingDelegatesToBridge(binding, functionName, 'tag_bridge');
    assert.match(tagBridge, new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'));
  }

  assert.doesNotMatch(binding, /FfiProject\w*V1|\b\w*project\w*_v1\b/iu);
});

test('UniFFI exposes typed History V1 contracts and Android consumes them without JSON', () => {
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const queryBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'history_query_bridge.rs'),
  );
  const mutationBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'history_mutation_bridge.rs'),
  );
  const historyMapper = read(
    'platforms',
    'uniffi',
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
  }
  for (const functionName of queryFunctions) {
    assertBindingDelegatesToBridge(binding, functionName, 'history_query_bridge');
    assert.match(
      queryBridge,
      new RegExp(`pub\\(crate\\)\\s+async\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
  for (const functionName of mutationFunctions) {
    assertBindingDelegatesToBridge(binding, functionName, 'history_mutation_bridge');
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
    `${binding}\n${historyMapper}`,
    /FfiProject\w*V1|\b\w*project\w*_v1\b|\bdelete_history_items_v1\b/iu,
  );
});

test('UniFFI exposes versioned typed Task Ledger contracts with tri-state patches', () => {
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const taskLedgerBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'task_ledger_bridge.rs'),
  );
  const taskLedgerMapper = read(
    'platforms',
    'uniffi',
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
    assertBindingDelegatesToBridge(binding, functionName, 'task_ledger_bridge');
    assert.match(
      taskLedgerBridge,
      new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
});

test('UniFFI exposes typed Recovery V1 records with JSON limited to dynamic leaves', () => {
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const recoveryBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'recovery_bridge.rs'),
  );
  const recoveryMapper = read(
    'platforms',
    'uniffi',
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
    assertBindingDelegatesToBridge(binding, functionName, 'recovery_bridge');
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
  const binding = read('platforms', 'uniffi', 'src', 'lib.rs');
  const automationBridge = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'automation_bridge.rs'),
  );
  const automationMapper = read(
    'platforms',
    'uniffi',
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
    assertBindingDelegatesToBridge(binding, functionName, 'automation_bridge');
    assert.match(
      automationBridge,
      new RegExp(`pub\\(crate\\)\\s+fn\\s+${functionName}\\b`, 'u'),
    );
  }
  assert.match(
    binding,
    /pub\s+fn\s+validate_automation_rule_activation_v1\s*\([^)]*rule:\s*FfiAutomationValidationRuleV1[^)]*global_config_json:\s*String[^)]*tags:\s*Vec<FfiAutomationTagReferenceV1>/su,
  );
  assert.doesNotMatch(binding, /FfiProject\w*V1|\b\w*project\w*_v1\b/iu);
});

// Every `*_json` UniFFI export without a `*_v1` sibling, and why it is still
// JSON. `dynamic-leaf` entries satisfy the typed-contract policy permanently:
// their payload is arbitrary user config or provider extension data whose
// schema this binding does not own. `pending-migration` entries are reviewed
// debt: they transport a complete snapshot, record, request, or result as a
// JSON string and must gain a typed `_v1` sibling.
//
// Adding a new `*_json` export without a `*_v1` sibling fails this test until
// it is classified here. Migrating one means deleting its row.
const UNIFFI_JSON_ONLY_EXPORTS = new Map([
  // Arbitrary application config and settings values.
  ['load_app_config_json', 'dynamic-leaf'],
  ['save_app_config_json', 'dynamic-leaf'],
  ['get_app_setting_json', 'dynamic-leaf'],
  ['set_app_setting_json', 'dynamic-leaf'],
  ['default_config_json', 'dynamic-leaf'],
  ['migrate_app_config_json', 'dynamic-leaf'],
  ['resolve_effective_config_json', 'dynamic-leaf'],
  // Provider-specific ASR configuration; parses into a typed record.
  ['volcengine_doubao_asr_config_from_json', 'dynamic-leaf'],
  // Legacy Project compatibility surface; deliberately gains no typed V1 API.
  ['delete_history_items_json', 'dynamic-leaf'],
  ['update_history_project_assignments_json', 'dynamic-leaf'],
  ['reassign_history_project_json', 'dynamic-leaf'],
  ['load_tag_repository_state_json', 'dynamic-leaf'],
  // Parser entry points: their whole purpose is turning the app's stored JSON
  // into the typed record, which is the *output*. A typed input would make them
  // identity functions, so they stay JSON by design.
  ['llm_config_from_json', 'dynamic-leaf'],
  ['polish_segments_request_from_json', 'dynamic-leaf'],
  ['translate_segments_request_from_json', 'dynamic-leaf'],
  ['summarize_transcript_request_from_json', 'dynamic-leaf'],
]);

function uniffiExports() {
  const binding = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'lib.rs'),
  );
  return [
    ...binding.matchAll(
      /#\[uniffi::export[^\]]*\]\s*pub\s+(?:async\s+)?fn\s+(\w+)\s*\(/gu,
    ),
  ].map(([, name]) => name);
}

test('UniFFI JSON-only exports stay on the reviewed typed-contract inventory', () => {
  const exported = uniffiExports();
  assert.ok(
    exported.length > 100,
    `expected the full exported UniFFI surface, found ${exported.length} functions`,
  );

  const exportedNames = new Set(exported);
  const jsonOnly = exported.filter(
    (name) => /_json\b/u.test(name) && !exportedNames.has(name.replace(/_json\b/u, '_v1')),
  );

  assert.deepEqual(
    [...jsonOnly].sort(),
    [...UNIFFI_JSON_ONLY_EXPORTS.keys()].sort(),
    'every JSON-only UniFFI export must be classified as dynamic-leaf or pending-migration; ' +
      'add a typed _v1 sibling or record the reason here',
  );

  for (const [name, classification] of UNIFFI_JSON_ONLY_EXPORTS) {
    assert.ok(
      ['dynamic-leaf', 'pending-migration'].includes(classification),
      `${name} has an unsupported classification ${classification}`,
    );
  }

  // Domains already migrated must keep both surfaces so existing Kotlin callers
  // keep compiling while new callers use the typed one.
  for (const migrated of [
    'load_storage_usage_snapshot',
    'export_transcript_file',
    'export_backup_archive',
    'inspect_backup_archive',
    'import_backup_archive',
    'load_dashboard_snapshot',
    'load_diagnostics_snapshot',
    'sync_get_status',
    'sync_create_vault',
    'sync_join_vault',
    'sync_run_now',
    'sync_list_conflicts',
    'sync_resolve_conflict',
    'llm_segment_inputs_from_transcript',
    'merge_translated_items_into_transcript',
    'merge_polished_items_into_transcript',
    'plan_summary_prompt_chunks',
    'parse_polish_chunk',
    'run_llm_polish',
    'list_llm_models',
    'describe_llm_model',
    'complete_llm',
  ]) {
    assert.ok(
      exportedNames.has(`${migrated}_json`),
      `${migrated}_json must remain as a compatibility delegate`,
    );
    assert.ok(
      exportedNames.has(`${migrated}_v1`),
      `${migrated}_v1 must expose the typed contract`,
    );
  }
});

// Field names that carry a secret. UniFFI renders a `Record` as a Kotlin
// `data class` whose generated `toString()` prints every field, so a credential
// held as a plain `String` leaks into any log line that formats the record.
// Credentials must be object handles (`FfiSecret`, `FfiOnlineAsrApiKey`),
// whose Kotlin `toString()` is their identity.
// `.` excludes newlines without the `s` flag, so a field type never spans rows.
const CREDENTIAL_FIELD_PATTERN =
  /(\w*(?:password|api_key|recovery_key|secret|token|credential))\s*:\s*(.+)/giu;
const OPAQUE_SECRET_TYPES = /Arc<Ffi\w*(?:Secret|ApiKey)>/u;
// `requires_api_key: bool` and `create_recovery_key: bool` name a credential but
// carry a flag, and a bool cannot leak one.
const NON_SECRET_FIELD_TYPES = /^bool\b/u;
// Fields whose name matches the credential pattern but hold no secret. Keep
// this list narrow and fully qualified; broadening the pattern instead would
// silently drop coverage for real credentials.
const NON_CREDENTIAL_FIELDS = new Set([
  // A filename token used to match model paths, not an auth token.
  'FfiModelCatalogPathMatchToken.token',
]);

test('UniFFI records carry credentials as opaque handles, never printable fields', () => {
  const sources = rustSources('platforms/uniffi/src').filter(({ relativePath }) =>
    relativePath.includes('/mapper/'),
  );
  assert.ok(sources.length > 5, `expected the mapper modules, found ${sources.length}`);

  let checkedFields = 0;
  for (const { relativePath, source } of sources) {
    // Only look inside `uniffi::Record` bodies; enums and helpers are exempt.
    for (const record of source.matchAll(
      /#\[derive\([^)]*uniffi::Record[^)]*\)\]\s*pub struct (\w+)\s*\{([^}]*)\}/gu,
    )) {
      const [, recordName, body] = record;
      for (const field of body.matchAll(CREDENTIAL_FIELD_PATTERN)) {
        const [, fieldName, fieldType] = field;
        if (
          NON_SECRET_FIELD_TYPES.test(fieldType.trim()) ||
          NON_CREDENTIAL_FIELDS.has(`${recordName}.${fieldName}`)
        ) {
          continue;
        }
        checkedFields += 1;
        assert.match(
          fieldType,
          OPAQUE_SECRET_TYPES,
          `${relativePath}: ${recordName}.${fieldName} carries a credential and must be an ` +
            `opaque handle (found \`${fieldType.trim()}\`), or Kotlin's generated toString() ` +
            'will print it',
        );
      }
    }
  }

  assert.ok(
    checkedFields > 0,
    'expected at least one credential-bearing record field to guard',
  );

  // The secret holders themselves must redact in Rust logs too, and must never
  // expose the value as an exported getter on the generated Kotlin handle.
  for (const [name, ...filePath] of [
    ['FfiSecret', 'platforms', 'uniffi', 'src', 'mapper', 'secret_mapper.rs'],
    ['FfiOnlineAsrApiKey', 'platforms', 'uniffi', 'src', 'asr_batch_bridge.rs'],
  ]) {
    const source = read(...filePath);
    assert.match(
      source,
      new RegExp(`impl fmt::Debug for ${name}[\\s\\S]*?<redacted>`, 'u'),
      `${name} must implement a redacting Debug so Rust logs cannot print the secret`,
    );
    // `expose()` reads the secret and must stay unexported: an exported method
    // would become a readable property on the Kotlin handle.
    const exportedBlock = new RegExp(
      `#\\[uniffi::export\\]\\s*impl ${name}\\s*\\{([\\s\\S]*?)\\n\\}`,
      'u',
    ).exec(source);
    assert.ok(exportedBlock, `${name} must have an exported impl block`);
    assert.doesNotMatch(
      exportedBlock[1],
      /fn\s+expose\b/u,
      `${name}::expose must not be exported across the FFI boundary`,
    );
  }
});

test('SonaContext exposes every directory-scoped operation the free functions do', () => {
  const binding = withoutInlineRustTests(read('platforms', 'uniffi', 'src', 'lib.rs'));
  const context = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'sona_context.rs'),
  );

  // Free functions that take an application-data directory are exactly the ones
  // an explicit context can serve, so the handle must cover all of them or the
  // two surfaces drift.
  const directoryScoped = [
    ...binding.matchAll(
      /#\[uniffi::export[^\]]*\]\s*pub\s+(?:async\s+)?fn\s+(\w+)\(\s*app_data_dir: String/gu,
    ),
  ].map(([, name]) => name);
  assert.ok(
    directoryScoped.length > 100,
    `expected the directory-scoped surface, found ${directoryScoped.length}`,
  );

  const methods = new Set(
    [...context.matchAll(/pub\s+(?:async\s+)?fn\s+(\w+)\(\s*&self/gu)].map(([, name]) => name),
  );

  // `release_application_context` frees a directory rather than operating on one,
  // and secret-store registration is a lifecycle concern the handle does not own.
  const lifecycleOnly = new Set([
    'release_application_context',
    'register_sync_secret_store_for_app_data_dir',
  ]);
  const missing = directoryScoped.filter(
    (name) => !methods.has(name) && !lifecycleOnly.has(name),
  );
  assert.deepEqual(missing, [], 'SonaContext must cover every directory-scoped operation');

  // The generated operations must hand over the context the handle already
  // holds, never rebuild a source from a directory string.
  const operations = /#\[uniffi::export\(async_runtime = "tokio"\)\]\s*impl SonaContext \{([\s\S]*)$/u
    .exec(context)?.[1];
  assert.ok(operations, 'SonaContext must expose a generated operations block');
  assert.doesNotMatch(
    operations,
    /ContextSource::from\(/u,
    'SonaContext operations must hand over the context it already holds',
  );
  assert.match(operations, /self\.source\(\)/u);
});

test('UniFFI binding delegates to bridges without an intermediate facade layer', () => {
  assert.ok(
    !fs.existsSync(path.join(repoRoot, 'platforms', 'uniffi', 'src', 'facade.rs')),
    'the SonaCoreFacade forwarding layer was merged into lib.rs; do not reintroduce facade.rs',
  );

  const uniffiSources = rustSources('platforms/uniffi/src');
  for (const { relativePath, source } of uniffiSources) {
    assert.doesNotMatch(
      source,
      /SonaCoreFacade/u,
      `${relativePath} must call the *_bridge modules directly, not through a facade type`,
    );
  }

  // The exported surface must stay a thin delegation layer: every
  // #[uniffi::export] free function body starts with a `*_bridge::` call.
  // `release_application_context` is the reviewed exception because it drives
  // the host composition root itself rather than a domain bridge.
  const compositionRootExports = new Set(['release_application_context']);
  const binding = withoutInlineRustTests(
    read('platforms', 'uniffi', 'src', 'lib.rs'),
  );
  const exports = [
    ...binding.matchAll(
      /#\[uniffi::export[^\]]*\]\s*pub\s+(?:async\s+)?fn\s+(\w+)\s*\([^{]*\{\s*([\w:]+)/gu,
    ),
  ];
  assert.ok(
    exports.length > 100,
    `expected the full exported UniFFI surface, found ${exports.length} functions`,
  );
  const observedCompositionRootExports = new Set();
  for (const [, functionName, firstCall] of exports) {
    if (compositionRootExports.has(functionName)) {
      observedCompositionRootExports.add(functionName);
      assert.match(
        firstCall,
        /^application_context::/u,
        `${functionName} must delegate to the host composition root`,
      );
      continue;
    }
    assert.match(
      firstCall,
      /^\w+_bridge::/u,
      `#[uniffi::export] ${functionName} must delegate to a *_bridge module, found ${firstCall}`,
    );
  }
  assert.deepEqual(
    [...observedCompositionRootExports].sort(),
    [...compositionRootExports].sort(),
    'composition-root export exceptions must stay explicit and must not become stale',
  );
});
