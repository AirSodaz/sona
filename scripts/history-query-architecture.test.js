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

test('history query policy is shared across hosts', () => {
  const queryPort = rustProductionView(
    read('core', 'src', 'history', 'query_repository.rs'),
  );
  const queryService = rustProductionView(
    read('core', 'src', 'history', 'query_service.rs'),
  );
  const historyStore = rustProductionView(read('core', 'src', 'history_store.rs'));

  assert.match(queryPort, /pub trait HistoryQueryRepository: Send \+ Sync/u);
  assert.match(queryPort, /pub enum HistoryQueryError/u);
  assert.doesNotMatch(queryPort, /history_store/u);
  for (const method of [
    'list_items_with_reconciled_live_drafts_paginated',
    'query_workspace',
    'load_transcript',
    'list_transcript_snapshots',
    'load_transcript_snapshot',
  ]) {
    assert.match(queryPort, new RegExp(`fn ${method}\\b`, 'u'));
  }
  assert.match(historyStore, /pub trait HistoryStore: HistoryQueryRepository/u);
  assert.doesNotMatch(
    historyStore,
    /fn (?:list_items|query_workspace|load_transcript|list_transcript_snapshots|load_transcript_snapshot)\b/u,
  );
  assert.match(queryService, /pub struct HistoryQueryService/u);
  assert.match(queryService, /validate_workspace_query_request/u);
  assert.match(queryService, /validate_pagination_value/u);
  assert.match(queryService, /validate_id/u);
  assert.doesNotMatch(queryService, /history_store/u);
  assert.match(queryService, /self\.repository\s*\.list_items_with_reconciled_live_drafts_paginated/u);
  for (const source of [queryPort, queryService, historyStore]) {
    assert.doesNotMatch(source, /std::fs|rusqlite|tauri|tokio|uniffi|clap/u);
  }

  const sqlite = rustProductionView(read('adapters', 'sqlite', 'src', 'history_store.rs'));
  assert.match(sqlite, /impl<D> HistoryQueryRepository for SqliteHistoryStore<D>/u);
  assert.match(sqlite, /impl<D> HistoryStore for SqliteHistoryStore<D>/u);
  assert.match(sqlite, /SqliteHistoryStore::query_workspace\(self, request\)/u);
  assert.match(sqlite, /HistoryQueryRepository::list_items\(self\)/u);
  assert.match(sqlite, /HistoryQueryRepository::load_transcript\(self, history_id\)/u);

  const desktopPlatform = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'history_repository.rs'),
  );
  const desktopCommands = rustProductionView(
    read('platforms', 'desktop', 'src', 'commands', 'history.rs'),
  );
  assert.match(desktopPlatform, /pub async fn run_history_query_file_task/u);
  assert.match(desktopPlatform, /pub async fn run_history_query_db_task/u);
  assert.match(desktopPlatform, /HistoryQueryService::new/u);
  assert.match(desktopPlatform, /tauri::async_runtime::spawn_blocking/u);
  assert.match(desktopPlatform, /let _guard = lock\.lock\(/u);
  assert.match(desktopCommands, /run_history_query_file_task/u);
  assert.match(desktopCommands, /run_history_query_db_task/u);
  for (const method of [
    'list_items',
    'query_workspace',
    'load_transcript',
    'list_transcript_snapshots',
    'load_transcript_snapshot',
  ]) {
    assert.match(desktopCommands, new RegExp(`service\\.${method}\\b`, 'u'));
  }
  assert.doesNotMatch(desktopCommands, /SqliteHistoryStore|HistoryQueryRepository/u);

  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliHistory = rustProductionView(read('platforms', 'cli', 'src', 'history.rs'));
  assert.match(cliLib, /History\(history::HistoryArgs\)/u);
  assert.match(cliLib, /Commands::History\(args\)\s*=>\s*history::run_history\(args\)/u);
  for (const command of ['List', 'Query', 'Transcript', 'Snapshots', 'Snapshot']) {
    assert.match(cliHistory, new RegExp(`HistoryCommands::${command}`, 'u'));
  }
  assert.match(cliHistory, /HistoryQueryService::new/u);
  assert.match(cliHistory, /serde_json::from_slice/u);
  assert.match(cliHistory, /if !app_data_dir\.is_dir\(\)/u);
  assert.match(cliHistory, /sanitize_table_cell/u);

  const uniffiBridge = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'history_query_bridge.rs'),
  );
  const uniffiFacade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  assert.match(uniffiBridge, /tokio::task::spawn_blocking/u);
  assert.match(uniffiBridge, /HistoryQueryService::new/u);
  assert.match(uniffiBridge, /ensure_existing_directory/u);
  assert.match(uniffiBridge, /parse_pagination_value/u);
  assert.match(uniffiBridge, /serde_json::to_string\(&canonical\)/u);
  for (const functionName of [
    'list_history_items_json',
    'query_history_workspace_json',
    'load_history_transcript_json',
    'list_history_transcript_snapshots_json',
    'load_history_transcript_snapshot_json',
  ]) {
    assert.match(uniffiFacade, new RegExp(`pub async fn ${functionName}\\b`, 'u'));
    assert.match(
      uniffiLib,
      new RegExp(`#\\[uniffi::export\\]\\s*pub async fn ${functionName}\\b`, 'u'),
    );
  }
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
    for (const method of [
      'listHistory',
      'queryHistory',
      'loadHistoryTranscript',
      'listHistorySnapshots',
      'loadHistorySnapshot',
    ]) {
      assert.match(source, new RegExp(`suspend fun ${method}\\b`, 'u'));
    }
    for (const generatedCall of [
      'listHistoryItemsJson',
      'queryHistoryWorkspaceJson',
      'loadHistoryTranscriptJson',
      'listHistoryTranscriptSnapshotsJson',
      'loadHistoryTranscriptSnapshotJson',
    ]) {
      assert.match(source, new RegExp(`${generatedCall}\\(`, 'u'));
    }
  }
});
