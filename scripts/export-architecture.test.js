import assert from 'node:assert/strict';
import test from 'node:test';

import {
  expectedUniffiErrorVariants,
  expectedUniffiExports,
  read,
  rustProductionView,
  stripKotlinCommentsAndLiterals,
} from './test-support/repository.js';

function topLevelUniffiExports(source) {
  return [...source.matchAll(/#\[uniffi::export\]\s*pub\s+(?:async\s+)?fn\s+([a-z0-9_]+)\s*\(/gu)]
    .map((match) => match[1]);
}

function uniffiErrorVariants(source) {
  const body = /pub enum SonaCoreBindingError\s*\{([\s\S]*?)\n\}/u
    .exec(source)?.[1] ?? assert.fail('missing SonaCoreBindingError');
  return [...body.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*\{/gmu)]
    .map((match) => match[1]);
}

test('transcript export policy is shared across hosts', () => {
  const coreMod = rustProductionView(read('core', 'src', 'export', 'mod.rs'));
  const coreModels = rustProductionView(read('core', 'src', 'export', 'models.rs'));
  const corePort = rustProductionView(read('core', 'src', 'export', 'ports.rs'));
  const coreService = rustProductionView(read('core', 'src', 'export', 'service.rs'));
  const coreError = rustProductionView(read('core', 'src', 'export', 'error.rs'));
  const adapter = rustProductionView(read('adapters', 'export', 'src', 'lib.rs'));

  assert.match(coreMod, /pub use ports::TranscriptExportRepository/u);
  assert.match(coreMod, /pub use service::ExportService/u);
  assert.match(coreModels, /pub struct ExportTranscriptFileRequest/u);
  assert.match(coreModels, /pub struct ExportTranscriptFileResult/u);
  assert.match(corePort, /pub trait TranscriptExportRepository: Send \+ Sync/u);
  assert.match(corePort, /fn write_export\(&self, output_path: &str, content: &str\)/u);
  assert.match(coreService, /export_segments_with_mode/u);
  assert.match(coreService, /self\.repository\s*\.write_export/u);
  assert.match(coreError, /pub enum ExportError/u);
  for (const source of [coreMod, coreModels, corePort, coreService, coreError]) {
    assert.doesNotMatch(source, /std::fs|tauri|tokio|uniffi|clap/u);
  }
  assert.match(adapter, /pub struct FsTranscriptExportRepository/u);
  assert.match(adapter, /impl TranscriptExportRepository for FsTranscriptExportRepository/u);
  assert.match(adapter, /std::fs::write/u);
  assert.doesNotMatch(adapter, /export_segments_with_mode/u);

  const desktop = rustProductionView(
    read('platforms', 'desktop', 'src', 'commands', 'export.rs'),
  );
  assert.match(desktop, /tauri::async_runtime::spawn_blocking/u);
  assert.match(desktop, /ExportService::new\(FsTranscriptExportRepository\)/u);
  assert.match(desktop, /ExportTranscriptFileRequest/u);
  assert.doesNotMatch(desktop, /std::fs|adapter_export_transcript_file/u);

  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliExport = rustProductionView(read('platforms', 'cli', 'src', 'export.rs'));
  assert.match(cliLib, /Export\(export::ExportArgs\)/u);
  assert.match(cliLib, /Commands::Export\(args\)\s*=>\s*export::run_export\(args\)/u);
  assert.match(cliExport, /serde_json::from_slice/u);
  assert.match(cliExport, /ExportFormat::from_output_path/u);
  assert.match(cliExport, /ExportService::new\(FsTranscriptExportRepository\)/u);
  assert.match(cliExport, /sanitize_table_cell\(&result\.output_path\)/u);

  const uniffiBridge = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'export_bridge.rs'),
  );
  const uniffiFacade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  assert.match(uniffiBridge, /tokio::task::spawn_blocking/u);
  assert.match(uniffiBridge, /serde_json::from_str/u);
  assert.match(uniffiBridge, /ExportService::new\(FsTranscriptExportRepository\)/u);
  assert.match(uniffiBridge, /serde_json::to_string\(&canonical\)/u);
  assert.match(uniffiFacade, /pub async fn export_transcript_file_json/u);
  assert.match(
    uniffiLib,
    /#\[uniffi::export\]\s*pub async fn export_transcript_file_json/u,
  );
  assert.deepEqual(topLevelUniffiExports(uniffiLib), expectedUniffiExports);
  assert.deepEqual(uniffiErrorVariants(uniffiLib), expectedUniffiErrorVariants);

  const androidSample = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  ));
  const androidConsumer = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  ));
  for (const source of [androidSample, androidConsumer]) {
    assert.match(source, /suspend fun exportTranscript\(inputJson: String\): String/u);
    assert.match(source, /exportTranscriptFileJson\(inputJson\)/u);
  }
});
