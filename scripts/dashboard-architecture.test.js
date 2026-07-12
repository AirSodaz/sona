import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import {
  assertCargoDependencyVersionAndFeature,
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

function assertReadOnlyComposition(source, label) {
  assert.match(
    source,
    /Database::open_read_only_with_analytics\s*\(/u,
    `${label} must open the analytics-aware read-only snapshot`,
  );
  assert.doesNotMatch(
    source,
    /Database::open\s*\(|with_(?:write|rw_)?transaction\s*\(|with_write_connection\s*\(|\.execute(?:_batch)?\s*\(/u,
    `${label} must not expose a persistence path`,
  );
  for (const symbol of [
    'SqliteHistoryStore',
    'SqliteProjectRepository',
    'SqliteAnalyticsRepository',
    'DashboardService',
    'dashboard_snapshot_time_now',
    'build_snapshot_at',
  ]) {
    assert.match(source, new RegExp(`\\b${symbol}\\b`, 'u'), `${label} missing ${symbol}`);
  }
}

function topLevelUniffiExports(source) {
  return [
    ...source.matchAll(
      /#\[uniffi::export\]\s*pub\s+(?:async\s+)?fn\s+([a-z0-9_]+)\s*\(/gu,
    ),
  ].map((match) => match[1]);
}

function uniffiErrorVariants(source) {
  const body = /pub enum SonaCoreBindingError\s*\{([\s\S]*?)\n\}/u
    .exec(source)?.[1] ?? assert.fail('missing SonaCoreBindingError');
  return [...body.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*\{/gmu)]
    .map((match) => match[1]);
}

test('dashboard reporting policy is shared across hosts', () => {
  const rustFilterFixture = rustProductionView(String.raw`
    // Database::open("comment-only")
    const TEXT: &str = "Database::open(\"literal-only\")";
    #[cfg(test)]
    fn test_only() { Database::open("test-only"); }
    pub fn production() { Database::open_read_only("real"); }
  `);
  assert.doesNotMatch(rustFilterFixture, /comment-only|test-only/u);
  assert.match(rustFilterFixture, /literal-only/u);
  assert.match(rustFilterFixture, /Database::open_read_only/u);

  const kotlinFilterFixture = stripKotlinCommentsAndLiterals(String.raw`
    // loadDashboardSnapshotJson(commentOnly, true)
    val text = "loadDashboardSnapshotJson(literalOnly, true)"
    suspend fun real(path: String) = loadDashboardSnapshotJson(path, false)
  `);
  assert.doesNotMatch(kotlinFilterFixture, /commentOnly|literalOnly/u);
  assert.match(kotlinFilterFixture, /loadDashboardSnapshotJson\(path, false\)/u);

  const coreDashboardDir = path.join(repoRoot, 'core', 'src', 'dashboard');
  const coreDashboard = rustFilesUnder(coreDashboardDir)
    .sort()
    .map((filePath) => rustProductionView(fs.readFileSync(filePath, 'utf8')))
    .join('\n');
  const coreService = rustProductionView(read('core', 'src', 'dashboard', 'service.rs'));
  const coreCargoPath = path.join(repoRoot, 'core', 'Cargo.toml');
  const coreCargo = fs.readFileSync(coreCargoPath, 'utf8');
  const coreDependencies = readCargoDependencyNames(coreCargoPath, 'dependencies');

  assert.match(coreService, /pub async fn build_snapshot_at\s*\(/u);
  for (const port of ['HistoryRepository', 'ProjectRepository', 'AnalyticsRepository']) {
    assert.match(coreDashboard, new RegExp(`pub trait ${port}\\b`, 'u'));
  }
  assert.match(coreService, /pub async fn build_snapshot_at\s*\(/u);
  assert.doesNotMatch(coreService, /pub async fn build_snapshot\s*\(/u);
  assert.doesNotMatch(
    coreDashboard,
    /\b(?:rusqlite|tauri|uniffi|tokio|clap|uuid|walkdir)::|std::(?:fs|net|path|process|time)::|Utc::now|Local::now/u,
  );
  assert.deepEqual(
    coreDependencies.filter((name) => hostCapabilityDependencies.has(name.replaceAll('_', '-'))),
    [],
  );
  assert.doesNotMatch(coreCargo, /chrono\s*=\s*\{[^}]*"clock"/u);

  const runtimeFsLib = rustProductionView(read('adapters', 'runtime_fs', 'src', 'lib.rs'));
  const runtimeFsTime = rustProductionView(
    read('adapters', 'runtime_fs', 'src', 'dashboard_time.rs'),
  );
  const desktopTime = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'time.rs'),
  );
  assert.match(runtimeFsLib, /pub use dashboard_time::dashboard_snapshot_time_now;/u);
  assert.match(runtimeFsTime, /pub fn dashboard_snapshot_time_now\s*\(\)/u);
  assert.match(runtimeFsTime, /Utc::now\s*\(\)/u);
  assert.doesNotMatch(desktopTime, /dashboard_snapshot_time_now/u);

  const sqliteHistory = rustProductionView(read('adapters', 'sqlite', 'src', 'history_store.rs'));
  const sqliteProject = rustProductionView(read('adapters', 'sqlite', 'src', 'project.rs'));
  const sqliteAnalytics = rustProductionView(read('adapters', 'sqlite', 'src', 'analytics.rs'));
  assert.match(sqliteHistory, /impl<D> sona_core::dashboard::ports::HistoryRepository/u);
  assert.match(sqliteProject, /impl<D> ProjectRepository for SqliteProjectRepository<D>/u);
  assert.match(sqliteAnalytics, /impl AnalyticsRepository for SqliteAnalyticsRepository/u);

  const desktopDashboard = rustProductionView(
    read('platforms', 'desktop', 'src', 'app', 'dashboard.rs'),
  );
  const desktopComposition = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'dashboard.rs'),
  );
  assert.match(
    desktopDashboard,
    /pub async fn get_dashboard_snapshot\s*\([\s\S]*State<'_, Arc<AppDashboardService>>[\s\S]*DashboardSnapshotRequest/u,
  );
  assert.match(desktopDashboard, /sona_runtime_fs::dashboard_snapshot_time_now\s*\(\)/u);
  assert.match(desktopDashboard, /build_snapshot_at\s*\(request\.deep, time\)/u);
  assert.match(desktopDashboard, /serde_json::to_value\s*\(snapshot\)/u);
  for (const symbol of [
    'SqliteHistoryStore',
    'SqliteProjectRepository',
    'SqliteAnalyticsRepository',
    'DashboardService',
  ]) {
    assert.match(desktopComposition, new RegExp(`\\b${symbol}\\b`, 'u'));
  }

  const uniffiBridge = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'dashboard_bridge.rs'),
  );
  const uniffiFacade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  assertReadOnlyComposition(uniffiBridge, 'UniFFI dashboard bridge');
  assert.match(uniffiBridge, /tokio::task::spawn_blocking/u);
  assert.match(uniffiBridge, /Builder::new_current_thread\s*\(\)/u);
  assert.match(uniffiBridge, /serde_json::to_value\s*\(snapshot\)/u);
  assert.match(uniffiBridge, /serde_json::to_string\s*\(&canonical\)/u);
  assert.match(uniffiFacade, /pub async fn load_dashboard_snapshot_json/u);
  assert.match(uniffiFacade, /dashboard_bridge::load_dashboard_snapshot_json\(app_data_dir, deep\)\.await/u);
  assert.deepEqual(topLevelUniffiExports(uniffiLib), expectedUniffiExports);
  assert.deepEqual(uniffiErrorVariants(uniffiLib), expectedUniffiErrorVariants);
  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');

  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliDashboard = rustProductionView(read('platforms', 'cli', 'src', 'dashboard.rs'));
  assertReadOnlyComposition(cliDashboard, 'CLI dashboard host');
  assert.match(cliLib, /Dashboard\(dashboard::DashboardArgs\)/u);
  assert.match(cliLib, /Commands::Dashboard\(args\)\s*=>\s*dashboard::run_dashboard\(args\)/u);
  assert.match(cliDashboard, /DashboardCommands::Show/u);
  assert.match(cliDashboard, /Builder::new_current_thread\s*\(\)/u);
  for (const header of ['GENERATED', 'ITEMS', 'PROJECTS', 'DURATION', 'TOKENS', 'DEEP']) {
    assert.match(cliDashboard, new RegExp(`"${header}"`, 'u'));
  }

  const androidSample = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  ));
  const androidConsumer = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  ));
  assert.match(androidSample, /suspend fun loadDashboard\(appDataDir: String, deep: Boolean\): String/u);
  assert.match(androidSample, /loadDashboardSnapshotJson\(appDataDir, deep\)/u);
  assert.match(androidConsumer, /suspend fun loadDashboard\(appDataDir: String\): String/u);
  assert.match(androidConsumer, /loadDashboardSnapshotJson\(appDataDir, false\)/u);

  for (const relativePath of [
    ['adapters', 'runtime_fs', 'src', 'dashboard_time.rs'],
    ['adapters', 'uniffi_bind', 'src', 'dashboard_bridge.rs'],
    ['platforms', 'cli', 'src', 'dashboard.rs'],
  ]) {
    const source = read(...relativePath);
    assert.ok(source.split(/\r?\n/u).length <= 800, `${relativePath.join('/')} exceeds 800 lines`);
  }
  assert.ok(
    fs.readFileSync(new URL(import.meta.url), 'utf8').split(/\r?\n/u).length <= 2000,
    'dashboard architecture guard exceeds 2000 lines',
  );
});
