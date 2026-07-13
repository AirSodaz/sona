import assert from 'node:assert/strict';
import test from 'node:test';

import {
  exists,
  expectedHistoryMutationExports,
  read,
  readKotlinFunctionItem,
  readRustFunctionBlock,
  rustProductionView,
  stripKotlinCommentsAndLiterals,
} from './test-support/repository.js';

const mutationMethods = [
  'create_live_draft',
  'complete_live_draft',
  'save_recording',
  'save_imported_file',
  'delete_items',
  'update_transcript',
  'create_transcript_snapshot',
  'update_item_meta',
  'update_project_assignments',
  'reassign_project',
];

test('history mutation policy is shared by the SQLite and Tauri adapters', () => {
  const mutationPort = rustProductionView(
    read('core', 'src', 'history', 'mutation_repository.rs'),
  );
  const mutationService = rustProductionView(
    read('core', 'src', 'history', 'mutation_service.rs'),
  );
  const historyStore = rustProductionView(read('core', 'src', 'history_store.rs'));

  assert.match(mutationPort, /pub trait HistoryMutationRepository: Send \+ Sync/u);
  assert.match(mutationPort, /pub enum HistoryMutationError/u);
  for (const method of mutationMethods) {
    assert.match(mutationPort, new RegExp(`fn ${method}\\b`, 'u'));
    assert.match(mutationService, new RegExp(`pub fn ${method}\\b`, 'u'));
    assert.match(mutationService, new RegExp(`self\\.repository\\.${method}\\b`, 'u'));
  }
  assert.match(
    historyStore,
    /pub trait HistoryStore: HistoryQueryRepository \+ HistoryMutationRepository/u,
  );
  assert.doesNotMatch(
    historyStore,
    new RegExp(`fn (?:${mutationMethods.join('|')})\\b`, 'u'),
  );
  for (const source of [mutationPort, mutationService, historyStore]) {
    assert.doesNotMatch(source, /std::fs|rusqlite|tauri|tokio|uniffi|clap/u);
  }

  const sqlite = rustProductionView(read('adapters', 'sqlite', 'src', 'history_store.rs'));
  assert.match(sqlite, /impl<D> HistoryMutationRepository for SqliteHistoryStore<D>/u);
  assert.match(sqlite, /fn with_history_file_lock<T>\s*\(/u);
  assert.match(sqlite, /\.lock_exclusive\(\)/u);
  for (const method of [
    'create_live_draft',
    'save_recording',
    'save_imported_file',
    'delete_items',
  ]) {
    assert.match(
      sqlite,
      new RegExp(`with_history_file_lock\\(\\|\\|\\s*SqliteHistoryStore::${method}\\b`, 'u'),
    );
  }
  for (const method of mutationMethods) {
    assert.match(sqlite, new RegExp(`SqliteHistoryStore::${method}\\b`, 'u'));
  }

  const desktopPlatform = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'history_repository.rs'),
  );
  const desktopCommands = rustProductionView(
    read('platforms', 'desktop', 'src', 'commands', 'history.rs'),
  );
  assert.match(desktopPlatform, /pub async fn run_history_mutation_file_task/u);
  assert.match(desktopPlatform, /pub async fn run_history_mutation_db_task/u);
  assert.match(desktopPlatform, /HistoryMutationService::new/u);
  assert.match(desktopPlatform, /tauri::async_runtime::spawn_blocking/u);
  assert.match(desktopPlatform, /let _guard = lock\.lock\(/u);
  for (const method of mutationMethods) {
    assert.match(desktopCommands, new RegExp(`service\\.${method}\\b`, 'u'));
    assert.doesNotMatch(
      desktopCommands,
      new RegExp(`repository\\.${method}\\b`, 'u'),
    );
  }
  assert.match(desktopCommands, /run_history_mutation_file_task/u);
  assert.match(desktopCommands, /run_history_mutation_db_task/u);
  assert.doesNotMatch(
    desktopCommands,
    /SqliteHistoryStore|HistoryMutationRepository/u,
  );

  const llmHelpers = rustProductionView(read(
    'platforms', 'desktop', 'src', 'platform', 'history_repository', 'llm_helpers.rs',
  ));
  const llmJobs = rustProductionView(
    read('platforms', 'desktop', 'src', 'integrations', 'llm', 'jobs.rs'),
  );
  assert.match(llmHelpers, /mutation_service: &HistoryMutationService/u);
  assert.match(llmHelpers, /mutation_service\s*\.create_transcript_snapshot\b/u);
  assert.match(llmHelpers, /mutation_service\s*\.update_transcript\b/u);
  assert.doesNotMatch(
    llmHelpers,
    /repository\.(?:create_transcript_snapshot|update_transcript)\b/u,
  );
  assert.match(llmJobs, /HistoryMutationService::new/u);

  const cliHistory = rustProductionView(read('platforms', 'cli', 'src', 'history.rs'));
  for (const command of [
    'CreateLiveDraft',
    'CompleteLiveDraft',
    'SaveRecording',
    'ImportFile',
    'Delete',
    'UpdateTranscript',
    'CreateSnapshot',
    'UpdateMeta',
    'AssignProject',
    'ReassignProject',
  ]) {
    assert.match(cliHistory, new RegExp(`HistoryCommands::${command}\\b`, 'u'));
  }
  for (const method of mutationMethods) {
    assert.match(cliHistory, new RegExp(`service\\s*\\.${method}\\b`, 'u'));
  }
  assert.match(
    cliHistory,
    /impl HistoryMutationRepository for CliHistoryMutationRepository/u,
  );
  assert.match(cliHistory, /fn open_mutation_service\b/u);
  assert.match(cliHistory, /fn with_store<T>\s*\(/u);
  assert.match(cliHistory, /Database::open\(&self\.app_data_dir\)/u);
  assert.match(cliHistory, /HistoryMutationService::new/u);
});

test('UniFFI history mutations use a lazy blocking SQLite adapter', () => {
  assert.equal(
    exists('adapters', 'uniffi_bind', 'src', 'history_mutation_bridge.rs'),
    true,
  );
  const bridge = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'history_mutation_bridge.rs'),
  );

  assert.match(bridge, /impl HistoryMutationRepository for LazySqliteHistoryMutationRepository/u);
  assert.match(bridge, /tokio::task::spawn_blocking/u);
  assert.match(bridge, /HistoryMutationService::new/u);
  assert.match(bridge, /Database::open\(&self\.app_data_dir\)/u);
  assert.match(bridge, /History app data directory does not exist/u);
  assert.match(bridge, /serde_json::to_string\(&canonical\)/u);
  const canonicalJson = readRustFunctionBlock(bridge, 'canonical_json');
  assert.equal(
    canonicalJson.match(/HistoryMutationError::Serialization/gu)?.length,
    2,
    'canonical JSON serialization errors must preserve their history mutation category',
  );
  for (const method of mutationMethods) {
    assert.match(bridge, new RegExp(`service\\.${method}\\b`, 'u'));
    assert.match(bridge, new RegExp(`fn ${method}\\b`, 'u'));
  }

  for (const exportName of expectedHistoryMutationExports) {
    const body = readRustFunctionBlock(bridge, exportName);
    assert.notEqual(body, '', `missing ${exportName} bridge function`);
    assert.match(body, /parse_request\(/u);
    assert.match(body, /run_mutation\(/u);
    assert.ok(
      body.indexOf('parse_request(') < body.indexOf('run_mutation('),
      `${exportName} must parse JSON before scheduling SQLite work`,
    );
  }
  const recording = readRustFunctionBlock(bridge, 'save_history_recording_json');
  assert.match(recording, /audio_bytes: Option<Vec<u8>>/u);
  assert.match(recording, /native_audio_path: Option<String>/u);
  assert.doesNotMatch(recording, /from_(?:str|value)[\s\S]*audio_(?:bytes|path)/u);
});

function assertAndroidHistoryMutationWrappers(kotlin) {
  const kotlinProduction = stripKotlinCommentsAndLiterals(kotlin);
  for (const exportName of expectedHistoryMutationExports) {
    const generatedName = exportName.replace(/_([a-z])/gu, (_, letter) => letter.toUpperCase());
    const wrapperName = generatedName.replace(/Json$/u, '');
    assert.match(
      kotlinProduction,
      new RegExp(`^import uniffi\\.sona_uniffi_bind\\.${generatedName}$`, 'mu'),
    );
    const wrapper = readKotlinFunctionItem(kotlinProduction, wrapperName);
    assert.notEqual(wrapper, '', `missing Android wrapper ${wrapperName}`);
    assert.match(wrapper, new RegExp(`suspend fun ${wrapperName}\\(`, 'u'));
    assert.match(wrapper, new RegExp(`${generatedName}\\(`, 'u'));
  }

  const recording = readKotlinFunctionItem(kotlinProduction, 'saveHistoryRecording');
  assert.match(recording, /audioBytes: ByteArray\?/u);
  assert.match(recording, /nativeAudioPath: String\?/u);
  assert.match(
    recording,
    /saveHistoryRecordingJson\(appDataDir, requestJson, audioBytes, nativeAudioPath\)/u,
  );
}

test('history mutation Android guard rejects block-comment wrapper decoys', () => {
  const declarations = expectedHistoryMutationExports.map((exportName) => {
    const generatedName = exportName.replace(/_([a-z])/gu, (_, letter) => letter.toUpperCase());
    const wrapperName = generatedName.replace(/Json$/u, '');
    const wrapper = wrapperName === 'saveHistoryRecording'
      ? `suspend fun ${wrapperName}(appDataDir: String, requestJson: String, audioBytes: ByteArray?, nativeAudioPath: String?): String = ${generatedName}(appDataDir, requestJson, audioBytes, nativeAudioPath)`
      : `suspend fun ${wrapperName}(appDataDir: String, requestJson: String): String = ${generatedName}(appDataDir, requestJson)`;
    return `import uniffi.sona_uniffi_bind.${generatedName}\n${wrapper}`;
  }).join('\n');
  const decoy = `/*\n${declarations}\n*/`;

  assert.throws(() => assertAndroidHistoryMutationWrappers(decoy));
});

test('history mutations are exposed as async UniFFI JSON APIs and Android suspend wrappers', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const sample = read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  );
  const consumer = read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  );

  assert.match(uniffiLib, /^mod history_mutation_bridge;/mu);
  assert.match(uniffiLib, /HistoryMutation\s*\{\s*reason: String\s*\}/u);
  for (const exportName of expectedHistoryMutationExports) {
    assert.match(
      uniffiLib,
      new RegExp(`#\\[uniffi::export\\]\\s*pub async fn ${exportName}\\(`, 'u'),
    );
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}\\(`, 'u'));
    assert.match(uniffiFacade, new RegExp(`pub async fn ${exportName}\\(`, 'u'));
    assert.match(
      uniffiFacade,
      new RegExp(`history_mutation_bridge::${exportName}\\(`, 'u'),
    );
  }

  for (const kotlin of [sample, consumer]) {
    assertAndroidHistoryMutationWrappers(kotlin);
  }
});
