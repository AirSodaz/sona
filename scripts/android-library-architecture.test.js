import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...segments) {
  const file = path.join(repoRoot, ...segments);
  assert.equal(fs.existsSync(file), true, `missing repository file: ${file}`);
  return fs.readFileSync(file, 'utf8');
}

function clientSource(module, ...segments) {
  return read('platforms', 'android', 'client', module, 'src', 'main', ...segments);
}

test('Android library reads history through an application port and UniFFI adapter', () => {
  const rustBindings = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const port = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'library',
    'RecordingLibraryPort.kt',
  );
  const adapter = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiRecordingHistoryAdapter.kt',
  );
  const bindings = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiRecordingBindings.kt',
  );

  assert.match(port, /interface RecordingLibraryPort/u);
  assert.doesNotMatch(port, /^import (?:android|androidx|uniffi)\./mu);
  assert.match(adapter, /RecordingLibraryPort/u);
  assert.match(bindings, /queryHistoryWorkspaceJson/u);
  assert.match(bindings, /loadHistoryTranscriptJson/u);
  for (const functionName of [
    'query_history_workspace_json',
    'load_history_transcript_json',
    'create_history_live_draft_json',
    'complete_history_live_draft_json',
    'delete_history_items_json',
    'update_history_transcript_json',
  ]) {
    assert.match(
      rustBindings,
      new RegExp(
        `#\\[uniffi::export\\(async_runtime = "tokio"\\)\\]\\s+pub async fn ${functionName}`,
        'u',
      ),
      `missing Tokio-backed UniFFI export: ${functionName}`,
    );
  }
});

test('Android composition owns library loading and detail navigation', () => {
  const container = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'composition',
    'SonaAppContainer.kt',
  );
  const activity = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'MainActivity.kt',
  );
  const navigation = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'navigation', 'SonaApp.kt',
  );
  const viewModel = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'library',
    'LibraryViewModel.kt',
  );
  const screen = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'library',
    'LibraryScreen.kt',
  );

  assert.match(container, /val recordingLibrary: RecordingLibraryPort = history/u);
  assert.match(activity, /LibraryViewModel\.factory\(container\.recordingLibrary\)/u);
  assert.match(navigation, /LIBRARY_DETAIL_ROUTE/u);
  assert.match(navigation, /LibraryDetailScreen/u);
  assert.match(viewModel, /LibraryListError\.LOAD_FAILED/u);
  assert.doesNotMatch(viewModel, /Throwable|\.message/u);
  assert.match(screen, /LazyColumn/u);
  assert.match(screen, /RecordingLibraryItemStatus\.DRAFT/u);
  assert.match(screen, /item\.title\.ifBlank/u);
});
