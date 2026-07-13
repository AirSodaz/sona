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

test('diagnostics policy is shared across hosts', () => {
  const coreDiagnostics = rustProductionView(read('core', 'src', 'runtime', 'diagnostics.rs'));
  const coreCargoPath = path.join(repoRoot, 'core', 'Cargo.toml');
  const coreDependencies = readCargoDependencyNames(coreCargoPath, 'dependencies');

  assert.match(coreDiagnostics, /pub struct DiagnosticsEnrichmentMeasurements/u);
  assert.match(coreDiagnostics, /pub trait DiagnosticsEnrichmentRepository: Send \+ Sync/u);
  assert.match(coreDiagnostics, /fn collect_measurements\s*\(/u);
  assert.doesNotMatch(coreDiagnostics, /async fn collect_measurements/u);
  assert.match(coreDiagnostics, /pub struct DiagnosticsService/u);
  assert.match(coreDiagnostics, /pub fn build_snapshot_at\s*\(/u);
  for (const symbol of [
    'resolve_model_catalog_selected_ids',
    'SelectedModelsInput',
    'ModelRulesInput',
    'onboarding_ready',
    'punctuation_required',
  ]) {
    assert.match(coreDiagnostics, new RegExp(`\\b${symbol}\\b`, 'u'));
  }
  assert.doesNotMatch(
    coreDiagnostics,
    /\b(?:rusqlite|tauri|uniffi|tokio|clap|uuid|walkdir)::|std::(?:fs|net|process|time)::|Utc::now/u,
  );
  assert.deepEqual(
    coreDependencies.filter((name) => hostCapabilityDependencies.has(name.replaceAll('_', '-'))),
    [],
  );

  const runtimeFsDiagnostics = rustProductionView(
    read('adapters', 'runtime_fs', 'src', 'diagnostics.rs'),
  );
  const runtimeFsTime = rustProductionView(
    read('adapters', 'runtime_fs', 'src', 'diagnostics_time.rs'),
  );
  const runtimeFsLib = rustProductionView(read('adapters', 'runtime_fs', 'src', 'lib.rs'));
  assert.match(runtimeFsDiagnostics, /pub struct FsDiagnosticsEnrichmentRepository/u);
  assert.match(runtimeFsDiagnostics, /impl DiagnosticsEnrichmentRepository/u);
  assert.match(runtimeFsDiagnostics, /crate::ensure_directory_exists/u);
  assert.match(runtimeFsDiagnostics, /crate::build_model_catalog_snapshot/u);
  assert.match(runtimeFsDiagnostics, /crate::resolve_runtime_path_status/u);
  assert.doesNotMatch(runtimeFsDiagnostics, /DiagnosticsService|selected_models|model_rules/u);
  assert.match(runtimeFsLib, /pub use diagnostics_time::diagnostics_scanned_at_now;/u);
  assert.match(runtimeFsTime, /pub fn diagnostics_scanned_at_now\(\)/u);
  assert.match(runtimeFsTime, /Utc::now\(\)/u);

  const desktop = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'diagnostics.rs'),
  );
  assert.match(desktop, /state\.metrics_snapshot\(\)\.await/u);
  assert.match(desktop, /sona_runtime_fs::diagnostics_scanned_at_now\(\)/u);
  assert.match(desktop, /tauri::async_runtime::spawn_blocking/u);
  assert.match(desktop, /resolve_runtime_environment_status_for_log_dir/u);
  assert.match(desktop, /FsDiagnosticsEnrichmentRepository::new/u);
  assert.match(desktop, /DiagnosticsService::new/u);
  assert.doesNotMatch(desktop, /resolve_model_catalog_selected_ids/u);
  assert.doesNotMatch(desktop, /input\.selected_models\s*=|input\.model_rules\s*=/u);
  assert.doesNotMatch(desktop, /input\.onboarding_ready\s*=|input\.punctuation_required\s*=/u);

  const uniffiBridge = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'diagnostics_bridge.rs'),
  );
  const uniffiFacade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  assert.match(uniffiBridge, /tokio::task::spawn_blocking/u);
  assert.match(uniffiBridge, /serde_json::from_str/u);
  assert.match(uniffiBridge, /std::path::absolute/u);
  assert.match(uniffiBridge, /FsDiagnosticsEnrichmentRepository::new/u);
  assert.match(uniffiBridge, /DiagnosticsService::new/u);
  assert.match(uniffiBridge, /serde_json::to_string\(&canonical\)/u);
  assert.doesNotMatch(uniffiBridge, /sona_local_asr|AsrState|sherpa|onnxruntime/ui);
  assert.match(uniffiFacade, /pub async fn load_diagnostics_snapshot_json/u);
  assert.match(uniffiLib, /#\[uniffi::export\]\s*pub async fn load_diagnostics_snapshot_json/u);
  assert.deepEqual(topLevelUniffiExports(uniffiLib), expectedUniffiExports);
  assert.deepEqual(uniffiErrorVariants(uniffiLib), expectedUniffiErrorVariants);

  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliDiagnostics = rustProductionView(read('platforms', 'cli', 'src', 'diagnostics.rs'));
  assert.match(cliLib, /Diagnostics\(diagnostics::DiagnosticsArgs\)/u);
  assert.match(cliLib, /Commands::Diagnostics\(args\)\s*=>\s*diagnostics::run_diagnostics\(args\)/u);
  assert.match(cliDiagnostics, /DiagnosticsCommands::Snapshot/u);
  assert.match(cliDiagnostics, /std::fs::read/u);
  assert.match(cliDiagnostics, /serde_json::from_slice/u);
  assert.match(cliDiagnostics, /sanitize_table_cell\(&snapshot\.permission_state\)/u);
  assert.match(cliDiagnostics, /FsDiagnosticsEnrichmentRepository::new/u);
  assert.match(cliDiagnostics, /DiagnosticsService::new/u);
  assert.match(cliDiagnostics, /serde_json::to_string_pretty\(&snapshot\)/u);
  for (const header of [
    'SCANNED', 'LIVE_MODEL', 'BATCH_MODEL', 'ONBOARDING',
    'PUNCTUATION', 'PERMISSION', 'MIC', 'SYSTEM_AUDIO',
  ]) {
    assert.match(cliDiagnostics, new RegExp(`"${header}"`, 'u'));
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
    assert.match(source, /suspend fun loadDiagnostics\(appDataDir: String, inputJson: String\): String/u);
    assert.match(source, /loadDiagnosticsSnapshotJson\(appDataDir, inputJson\)/u);
  }

  const guardedFiles = [
    ['core', 'src', 'runtime', 'diagnostics.rs'],
    ['adapters', 'runtime_fs', 'src', 'diagnostics.rs'],
    ['adapters', 'runtime_fs', 'src', 'diagnostics_time.rs'],
    ['adapters', 'uniffi_bind', 'src', 'diagnostics_bridge.rs'],
    ['platforms', 'cli', 'src', 'diagnostics.rs'],
    ['platforms', 'desktop', 'src', 'platform', 'diagnostics.rs'],
  ];
  for (const relativePath of guardedFiles) {
    const source = read(...relativePath);
    assert.ok(source.split(/\r?\n/u).length <= 800, `${relativePath.join('/')} exceeds 800 lines`);
  }

  const diagnosticsSources = rustFilesUnder(path.join(repoRoot, 'core', 'src', 'runtime'))
    .filter((filePath) => filePath.endsWith('diagnostics.rs'));
  assert.equal(diagnosticsSources.length, 1);
});
