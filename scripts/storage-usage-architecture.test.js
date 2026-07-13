import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import {
  expectedUniffiErrorVariants,
  expectedUniffiExports,
  read,
  readCargoDependencyNames,
  repoRoot,
  rustFilesUnder,
  rustProductionView,
  stripKotlinCommentsAndLiterals,
} from './test-support/repository.js';

const hostCapabilityDependencies = new Set([
  'clap',
  'rusqlite',
  'tauri',
  'tokio',
  'uniffi',
  'uuid',
  'walkdir',
]);

function assertReadOnlyStorageHost(source, label) {
  assert.match(source, /Database::open_read_only_with_analytics\s*\(/u, `${label} is not read-only`);
  assert.doesNotMatch(
    source,
    /Database::open\s*\(|with_(?:write|rw_)?transaction\s*\(|with_write_connection\s*\(|\.execute(?:_batch)?\s*\(/u,
    `${label} exposes a persistence path`,
  );
  for (const symbol of [
    'SqliteStorageUsageRepository',
    'StorageUsageService',
    'storage_usage_generated_at_now',
  ]) {
    assert.match(source, new RegExp(`\\b${symbol}\\b`, 'u'), `${label} missing ${symbol}`);
  }
}

function topLevelUniffiExports(source) {
  return [
    ...source.matchAll(/#\[uniffi::export\]\s*pub\s+(?:async\s+)?fn\s+([a-z0-9_]+)\s*\(/gu),
  ].map((match) => match[1]);
}

function uniffiErrorVariants(source) {
  const body = /pub enum SonaCoreBindingError\s*\{([\s\S]*?)\n\}/u
    .exec(source)?.[1] ?? assert.fail('missing SonaCoreBindingError');
  return [...body.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*\{/gmu)]
    .map((match) => match[1]);
}

test('storage usage policy is shared across hosts', () => {
  const coreDir = path.join(repoRoot, 'core', 'src', 'storage_usage');
  const coreStorage = rustFilesUnder(coreDir)
    .sort()
    .map((filePath) => rustProductionView(fs.readFileSync(filePath, 'utf8')))
    .join('\n');
  const coreModels = rustProductionView(read('core', 'src', 'storage_usage', 'models.rs'));
  const corePorts = rustProductionView(read('core', 'src', 'storage_usage', 'ports.rs'));
  const coreService = rustProductionView(read('core', 'src', 'storage_usage', 'service.rs'));
  const coreCargoPath = path.join(repoRoot, 'core', 'Cargo.toml');
  const coreDependencies = readCargoDependencyNames(coreCargoPath, 'dependencies');

  assert.match(coreModels, /pub struct StorageUsageSnapshot/u);
  assert.match(coreModels, /pub struct StorageUsageMeasurements/u);
  assert.match(corePorts, /pub trait StorageUsageRepository: Send \+ Sync/u);
  assert.match(coreService, /pub fn load_snapshot_at\s*\(/u);
  assert.match(coreService, /saturating_add/u);
  assert.match(coreService, /fold\(0_u64, u64::saturating_add\)/u);
  assert.match(coreService, /pub fn build_webview_clear_result/u);
  assert.doesNotMatch(
    coreStorage,
    /\b(?:rusqlite|tauri|uniffi|tokio|clap|uuid|walkdir)::|std::(?:fs|net|path|process|time)::|Utc::now/u,
  );
  assert.deepEqual(
    coreDependencies.filter((name) => hostCapabilityDependencies.has(name.replaceAll('_', '-'))),
    [],
  );

  const runtimeFsLib = rustProductionView(read('adapters', 'runtime_fs', 'src', 'lib.rs'));
  const runtimeFsTime = rustProductionView(
    read('adapters', 'runtime_fs', 'src', 'storage_usage_time.rs'),
  );
  assert.match(runtimeFsLib, /pub use storage_usage_time::storage_usage_generated_at_now;/u);
  assert.match(runtimeFsTime, /pub fn storage_usage_generated_at_now\s*\(\)/u);
  assert.match(runtimeFsTime, /Utc::now\s*\(\)/u);

  const sqliteStorage = rustProductionView(
    read('adapters', 'sqlite', 'src', 'storage_usage.rs'),
  );
  assert.match(sqliteStorage, /pub struct SqliteStorageUsageRepository/u);
  assert.match(sqliteStorage, /impl StorageUsageRepository for SqliteStorageUsageRepository/u);
  assert.match(sqliteStorage, /FROM dbstat/u);
  assert.match(sqliteStorage, /WalkDir::new/u);
  assert.doesNotMatch(sqliteStorage, /pub struct StorageUsageSnapshot/u);
  assert.doesNotMatch(sqliteStorage, /Utc::now|load_snapshot_at/u);

  const database = rustProductionView(read('adapters', 'sqlite', 'src', 'lib.rs'));
  assert.match(database, /pub fn open_read_only\s*\(/u);
  assert.match(database, /pub fn open_read_only_with_analytics\s*\(/u);

  const desktop = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'storage_usage.rs'),
  );
  assert.match(desktop, /pub use sona_core::storage_usage::\{StorageUsageSnapshot, WebviewBrowsingDataClearResult\}/u);
  assert.match(desktop, /SqliteStorageUsageRepository::new/u);
  assert.match(desktop, /StorageUsageService::new/u);
  assert.match(desktop, /spawn_blocking/u);
  assert.match(desktop, /sona_core::storage_usage::build_webview_clear_result/u);
  assert.match(desktop, /clear_all_browsing_data/u);
  assert.doesNotMatch(desktop, /collect_storage_usage_snapshot/u);

  const uniffiBridge = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'storage_usage_bridge.rs'),
  );
  const uniffiFacade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  assertReadOnlyStorageHost(uniffiBridge, 'UniFFI storage usage bridge');
  assert.match(uniffiBridge, /tokio::task::spawn_blocking/u);
  assert.match(uniffiBridge, /serde_json::to_value\s*\(snapshot\)/u);
  assert.match(uniffiBridge, /serde_json::to_string\s*\(&canonical\)/u);
  assert.match(uniffiFacade, /pub async fn load_storage_usage_snapshot_json/u);
  assert.match(uniffiLib, /#\[uniffi::export\]\s*pub async fn load_storage_usage_snapshot_json/u);
  assert.deepEqual(topLevelUniffiExports(uniffiLib), expectedUniffiExports);
  assert.deepEqual(uniffiErrorVariants(uniffiLib), expectedUniffiErrorVariants);

  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliStorage = rustProductionView(read('platforms', 'cli', 'src', 'storage.rs'));
  assertReadOnlyStorageHost(cliStorage, 'CLI storage usage host');
  assert.match(cliLib, /Storage\(storage::StorageArgs\)/u);
  assert.match(cliLib, /Commands::Storage\(args\)\s*=>\s*storage::run_storage\(args\)/u);
  assert.match(cliStorage, /StorageCommands::Usage/u);
  assert.match(cliStorage, /serde_json::to_string_pretty\(&snapshot\)/u);
  for (const header of [
    'GENERATED', 'TOTAL', 'AUDIO', 'DATABASE', 'MODELS', 'TEMPORARY', 'WEBVIEW', 'OTHER',
  ]) {
    assert.match(cliStorage, new RegExp(`"${header}"`, 'u'));
  }

  const androidSample = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  ));
  const androidConsumer = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  ));
  for (const source of [androidSample, androidConsumer]) {
    assert.match(source, /suspend fun loadStorageUsage\(appDataDir: String\): String/u);
    assert.match(source, /loadStorageUsageSnapshotJson\(appDataDir\)/u);
  }

  for (const relativePath of [
    ['adapters', 'runtime_fs', 'src', 'storage_usage_time.rs'],
    ['adapters', 'uniffi_bind', 'src', 'storage_usage_bridge.rs'],
    ['adapters', 'sqlite', 'src', 'storage_usage.rs'],
    ['platforms', 'cli', 'src', 'storage.rs'],
  ]) {
    const source = read(...relativePath);
    assert.ok(source.split(/\r?\n/u).length <= 800, `${relativePath.join('/')} exceeds 800 lines`);
  }
});
