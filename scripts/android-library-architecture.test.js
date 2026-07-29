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

function sourceFiles(root) {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const child = path.join(root, entry.name);
    return entry.isDirectory() ? sourceFiles(child) : [child];
  });
}

test('Android library reads history through an application port and UniFFI adapter', () => {
  const rustBindings = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const port = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'library',
    'HistoryWorkspace.kt',
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

  assert.match(port, /interface HistoryWorkspacePort/u);
  assert.match(adapter, /HistoryWorkspacePort/u);
  assert.doesNotMatch(adapter, /Json/u);
  assert.match(bindings, /queryHistoryWorkspaceV1/u);
  assert.match(bindings, /loadHistoryTranscriptV1/u);
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

  assert.match(container, /val recordingLibrary: HistoryWorkspacePort = history/u);
  assert.match(activity, /LibraryViewModel\.factory\(\s*library = container\.recordingLibrary,/u);
  assert.match(navigation, /LIBRARY_DETAIL_ROUTE/u);
  assert.match(navigation, /LibraryDetailScreen/u);
  assert.match(viewModel, /LibraryListError\.LOAD_FAILED/u);
  assert.doesNotMatch(viewModel, /Throwable|\.message/u);
  assert.match(screen, /LazyColumn/u);
  assert.match(screen, /HistoryItemStatus\.DRAFT/u);
  assert.match(screen, /item\.title\.ifBlank/u);
  assert.doesNotMatch(screen, /OpenDocument|onImportAudio|AudioImportStatus/u);
});

test('Android home owns both workspaces and only previews three recent records', () => {
  const destination = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'navigation', 'SonaDestination.kt',
  );
  const navigation = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'navigation', 'SonaApp.kt',
  );
  const home = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'home', 'HomeScreen.kt',
  );
  const fileWorkspace = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'home',
    'FileTranscriptionScreen.kt',
  );

  assert.match(destination, /HOME\("home"/u);
  assert.doesNotMatch(destination, /RECORD\(/u);
  assert.match(navigation, /startDestination = SonaDestination\.HOME\.route/u);
  assert.match(navigation, /HOME_LIVE_ROUTE = "home\/live"/u);
  assert.match(navigation, /HOME_FILE_ROUTE = "home\/file"/u);
  assert.match(home, /items\.take\(3\)/u);
  assert.match(home, /maxWidth >= 600\.dp/u);
  assert.match(fileWorkspace, /ActivityResultContracts\.OpenDocument/u);
  assert.match(fileWorkspace, /onStart: \(String\) -> Unit/u);
  assert.match(fileWorkspace, /AudioImportJobState\.Running/u);
  assert.match(fileWorkspace, /AudioImportJobState\.Completed/u);
});

test('Android public beta data capabilities stay behind typed application ports', () => {
  const clientRoot = path.join(repoRoot, 'platforms', 'android', 'client');
  const oldPort = path.join(
    clientRoot, 'application', 'src', 'main', 'kotlin', 'com', 'sona', 'android',
    'application', 'library', 'RecordingLibraryPort.kt',
  );
  const dataPort = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'data',
    'DataTransfer.kt',
  );
  const recoveryPort = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recovery',
    'Recovery.kt',
  );
  const syncPort = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'sync',
    'Sync.kt',
  );
  const dataAdapter = clientSource(
    path.join('adapters', 'uniffi'), 'kotlin', 'com', 'sona', 'android', 'adapters',
    'uniffi', 'data', 'UniffiDataTransferAdapter.kt',
  );
  const recoveryAdapter = clientSource(
    path.join('adapters', 'uniffi'), 'kotlin', 'com', 'sona', 'android', 'adapters',
    'uniffi', 'recovery', 'UniffiRecoveryAdapter.kt',
  );
  const syncAdapter = clientSource(
    path.join('adapters', 'uniffi'), 'kotlin', 'com', 'sona', 'android', 'adapters',
    'uniffi', 'sync', 'UniffiSyncAdapter.kt',
  );
  const saf = clientSource(
    path.join('adapters', 'android'), 'kotlin', 'com', 'sona', 'android', 'adapters',
    'android', 'data', 'AndroidSafFileTransferAdapter.kt',
  );
  const worker = clientSource(
    path.join('adapters', 'android'), 'kotlin', 'com', 'sona', 'android', 'adapters',
    'android', 'sync', 'AndroidSyncWorker.kt',
  );

  assert.equal(fs.existsSync(oldPort), false);
  assert.match(dataPort, /interface BackupPort/u);
  assert.match(dataPort, /fun interface TranscriptExportPort/u);
  assert.match(recoveryPort, /interface RecoveryPort/u);
  assert.match(syncPort, /interface SyncPort/u);
  assert.match(dataAdapter, /exportTranscriptFileV1/u);
  assert.match(dataAdapter, /inspectBackupArchiveV1/u);
  assert.match(recoveryAdapter, /persistRecoveryQueueSnapshotV1/u);
  assert.match(syncAdapter, /syncGetConflictV1/u);
  assert.match(saf, /ContentResolver\.SCHEME_CONTENT/u);
  assert.match(worker, /PeriodicWorkRequestBuilder<SyncWorker>\(6, TimeUnit\.HOURS\)/u);
  assert.match(worker, /NetworkType\.CONNECTED/u);

  for (const module of ['application', path.join('adapters', 'android'), 'app']) {
    const root = path.join(clientRoot, module, 'src', 'main');
    const generatedImports = sourceFiles(root)
      .filter((file) => file.endsWith('.kt'))
      .filter((file) => fs.readFileSync(file, 'utf8').includes('uniffi.sona_uniffi_bind'));
    assert.deepEqual(generatedImports, [], `generated UniFFI import escaped adapter: ${module}`);
  }
});
