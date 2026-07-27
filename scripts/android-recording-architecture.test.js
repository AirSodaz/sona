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

test('Android streaming adapters support online and local engines behind generated bindings', () => {
  const bindings = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiRecordingBindings.kt',
  );
  const provider = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiStreamingProviderCatalogAdapter.kt',
  );
  const streaming = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiStreamingTranscriptionAdapter.kt',
  );
  const history = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiRecordingHistoryAdapter.kt',
  );
  const transcriptMapper = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiTranscriptMapper.kt',
  );

  assert.match(provider, /StreamingProviderCatalogPort/u);
  assert.match(streaming, /StreamingTranscriptionPort/u);
  assert.match(streaming, /override fun onStreamingError/u);
  assert.match(streaming, /finally[\s\S]*eventChannel\.close/u);
  assert.match(history, /RecordingHistoryPort/u);
  assert.match(bindings, /createAsrStreamingSession/u);
  assert.match(streaming, /StreamingEngineConfig\.LocalSherpa/u);
  for (const functionName of [
    'createHistoryLiveDraftV1',
    'updateHistoryTranscriptV1',
    'completeHistoryLiveDraftV1',
    'purgeHistoryItemsV1',
    'queryHistoryWorkspaceV1',
    'loadHistoryTranscriptV1',
  ]) {
    assert.match(bindings, new RegExp(`\\b${functionName}\\b`, 'u'));
  }
  assert.doesNotMatch(
    bindings,
    /(?:createHistoryLiveDraft|updateHistoryTranscript|completeHistoryLiveDraft|purgeHistoryItems|queryHistoryWorkspace|loadHistoryTranscript)Json/u,
  );
  assert.match(transcriptMapper, /internal fun TranscriptSegment\.toFfi/u);
  assert.doesNotMatch(history, /kotlinx\.serialization\.json|buildJsonObject|parseJson/u);
});

test('Android local ASR catalog, managed downloads, and recording startup are wired end to end', () => {
  const settingsPort = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recording',
    'RecognitionSettings.kt',
  );
  const coordinator = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recording',
    'LiveRecordingCoordinator.kt',
  );
  const repository = clientSource(
    path.join('adapters', 'android'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'android', 'settings',
    'AndroidRecognitionSettingsRepository.kt',
  );
  const storage = clientSource(
    path.join('adapters', 'android'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'android', 'settings',
    'AndroidLocalAsrModelStorage.kt',
  );
  const capabilities = clientSource(
    path.join('adapters', 'android'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'android', 'settings',
    'AndroidLocalAsrDeviceCapabilities.kt',
  );
  const catalog = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiLocalAsrModelCatalogAdapter.kt',
  );
  const settingsPane = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'RecognitionSettingsPane.kt',
  );
  const recordScreen = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'recording',
    'RecordScreen.kt',
  );
  const container = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'composition',
    'SonaAppContainer.kt',
  );

  assert.doesNotMatch(settingsPort, /^import (?:android|androidx|uniffi)\./mu);
  assert.match(settingsPort, /enum class RecognitionEngine/u);
  assert.match(settingsPort, /interface RecognitionSettingsPort/u);
  assert.match(settingsPort, /downloadLocalModel/u);
  assert.match(settingsPort, /validateLocalModel/u);
  assert.match(settingsPort, /deleteLocalModel/u);
  assert.match(coordinator, /RecognitionEngine\.LOCAL[\s\S]*StreamingEngineConfig\.LocalSherpa/u);
  assert.match(repository, /AndroidLocalAsrModelStorage/u);
  assert.match(storage, /filesDir, "models"/u);
  assert.match(storage, /verifySha256/u);
  assert.match(storage, /extractTarBz2/u);
  assert.match(storage, /safeArchiveDestination/u);
  assert.match(storage, /silero_vad\.onnx/u);
  assert.match(capabilities, /ActivityManager\.MemoryInfo/u);
  assert.match(capabilities, /StatFs/u);
  assert.match(catalog, /presetModels/u);
  assert.match(catalog, /"streaming" in it\.modes/u);
  assert.doesNotMatch(settingsPane, /OpenDocumentTree|DocumentFile|FolderOpen/u);
  assert.match(recordScreen, /onEngineSelected\(RecognitionEngine\.LOCAL\)/u);
  assert.match(container, /AndroidRecognitionSettingsRepository/u);
});

test('Android online batch ASR stays behind an application port and Tokio UniFFI adapter', () => {
  const port = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recording',
    'OnlineBatchPorts.kt',
  );
  const adapter = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiOnlineBatchTranscriptionAdapter.kt',
  );
  const bindings = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiOnlineBatchBindings.kt',
  );
  const transcriptMapper = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiTranscriptMapper.kt',
  );
  const rustBindings = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const rustBatchBridge = read('adapters', 'uniffi_bind', 'src', 'asr_batch_bridge.rs');

  assert.match(port, /enum class OnlineBatchProvider/u);
  assert.match(port, /interface OnlineBatchTranscriptionPort/u);
  assert.match(port, /OnlineBatchCredential\(apiKey=<redacted>\)/u);
  assert.doesNotMatch(port, /^import (?:android|androidx|uniffi)\./mu);
  assert.match(adapter, /OnlineBatchTranscriptionPort/u);
  assert.match(adapter, /FfiTranscriptSegment::toApplication/u);
  assert.match(bindings, /FfiOnlineAsrApiKey/u);
  assert.match(bindings, /transcribeOnlineAsrBatch/u);
  assert.match(transcriptMapper, /internal fun FfiTranscriptSegment\.toApplication/u);
  assert.match(rustBatchBridge, /find_online_asr_provider/u);
  assert.match(rustBatchBridge, /<redacted>/u);
  assert.match(
    rustBindings,
    /#\[uniffi::export\(async_runtime = "tokio"\)\]\s+pub async fn transcribe_online_asr_batch/u,
  );
});

test('Android cloud batch transcription is wired end to end behind its own credential slot', () => {
  const useCase = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recording',
    'TranscribeRecordingWithCloud.kt',
  );
  const credentialPorts = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recording',
    'BatchCredentialPorts.kt',
  );
  const libraryPort = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'library',
    'RecordingLibraryPort.kt',
  );
  const historyAdapter = clientSource(
    path.join('adapters', 'uniffi'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'uniffi', 'recording',
    'UniffiRecordingHistoryAdapter.kt',
  );
  const repository = clientSource(
    path.join('adapters', 'android'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'android', 'credential',
    'AndroidBatchCredentialRepository.kt',
  );
  const keystoreCipher = clientSource(
    path.join('adapters', 'android'),
    'kotlin', 'com', 'sona', 'android', 'adapters', 'android', 'credential',
    'AndroidKeyStoreCredentialCipher.kt',
  );
  const container = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'composition', 'SonaAppContainer.kt',
  );
  const libraryViewModel = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'library',
    'LibraryViewModel.kt',
  );
  const settingsViewModel = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'CloudTranscriptionSettingsViewModel.kt',
  );
  const settingsCard = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'CloudTranscriptionSettingsCard.kt',
  );
  const detailScreen = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'library',
    'LibraryDetailScreen.kt',
  );

  // The use case owns the persistence rules the surfaces must not re-decide.
  assert.match(useCase, /class TranscribeRecordingWithCloud/u);
  assert.match(useCase, /OnlineBatchTranscriptionPort/u);
  assert.match(useCase, /if \(result\.segments\.isEmpty\(\)\)[\s\S]*EMPTY_TRANSCRIPT/u);
  assert.match(useCase, /if \(request\.isDraft\)[\s\S]*completeLiveDraft/u);
  assert.doesNotMatch(useCase, /^import (?:android|androidx|uniffi)\./mu);
  assert.doesNotMatch(credentialPorts, /^import (?:android|androidx|uniffi)\./mu);
  assert.match(credentialPorts, /interface BatchCredentialSettingsPort/u);
  assert.match(credentialPorts, /interface BatchCredentialResolverPort/u);

  // The audio file a re-transcription needs travels through the library port.
  assert.match(libraryPort, /val audioPath: String/u);
  assert.match(libraryPort, /val audioAvailable: Boolean/u);
  assert.match(historyAdapter, /FfiHistoryAudioStatusV1\.AVAILABLE/u);

  // Each provider key is isolated by its own Keystore alias and AAD binding.
  assert.match(keystoreCipher, /fun batch\(providerStorageId: String\)/u);
  assert.match(keystoreCipher, /sona\.batch_credential\.\$providerStorageId\.aes_gcm\.v1/u);
  assert.match(keystoreCipher, /sona\/android\/batch-credential\/v1\/\$providerStorageId/u);
  assert.match(repository, /BatchCredentialSettingsPort, BatchCredentialResolverPort/u);
  assert.match(repository, /noBackupFilesDir|BatchCredentialDataStore\.create/u);
  assert.match(repository, /LegacyStreamingCredentialRepository\.createIfPresent/u);
  assert.match(repository, /slotFor\(DEFAULT_PROVIDER\.storageId\)/u);
  assert.match(repository, /tryClearLegacyCredential/u);
  assert.doesNotMatch(repository, /Log\.|println\(/u);

  // Composition and surfaces stay wired to the ports rather than the adapters.
  assert.match(container, /AndroidBatchCredentialRepository\.create\(appContext\)/u);
  assert.match(container, /UniffiOnlineBatchTranscriptionAdapter\(\)/u);
  assert.match(container, /val transcribeRecordingWithCloud = TranscribeRecordingWithCloud\(/u);
  assert.match(container, /val batchCredentialSettings: BatchCredentialSettingsPort/u);
  assert.match(libraryViewModel, /fun transcribeWithCloud\(item: RecordingLibraryItem\)/u);
  assert.match(libraryViewModel, /CloudTranscriptionUiState\.Running/u);
  assert.match(settingsViewModel, /apiKeyInput=<redacted>/u);
  assert.doesNotMatch(settingsViewModel, /SavedStateHandle/u);
  assert.match(settingsCard, /PasswordVisualTransformation/u);
  assert.doesNotMatch(settingsCard, /rememberSaveable/u);
  assert.match(detailScreen, /onTranscribeWithCloud/u);
  assert.match(detailScreen, /enabled = item\.audioAvailable/u);
});

test('Android recording composition preserves lifecycle, permission, and credential boundaries', () => {
  const manifest = clientSource('app', 'AndroidManifest.xml');
  const application = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'SonaApplication.kt',
  );
  const container = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'composition',
    'SonaAppContainer.kt',
  );
  const activity = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'MainActivity.kt',
  );
  const coordinator = clientSource(
    'application', 'kotlin', 'com', 'sona', 'android', 'application', 'recording',
    'LiveRecordingCoordinator.kt',
  );
  const recordingViewModel = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'recording',
    'RecordingViewModel.kt',
  );
  const recordScreen = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'recording',
    'RecordScreen.kt',
  );
  const lifecycleEffect = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'recording',
    'ForegroundRecordingLifecycleEffect.kt',
  );
  const permissionPolicy = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'recording',
    'MicrophonePermissionPolicy.kt',
  );
  const settingsViewModel = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'CloudTranscriptionSettingsViewModel.kt',
  );
  const cloudSettingsCard = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'CloudTranscriptionSettingsCard.kt',
  );
  const settingsScreen = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'SettingsScreen.kt',
  );
  const recognitionSettingsPane = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'feature', 'settings',
    'RecognitionSettingsPane.kt',
  );
  const navigation = clientSource(
    'app', 'kotlin', 'com', 'sona', 'android', 'app', 'navigation', 'SonaApp.kt',
  );

  assert.match(manifest, /android:name="\.SonaApplication"/u);
  assert.match(manifest, /android\.permission\.INTERNET/u);
  assert.match(application, /SonaAppContainer\(this\)/u);
  assert.match(container, /AndroidBatchCredentialRepository\.create\(appContext\)/u);
  assert.doesNotMatch(container, /AndroidStreamingCredentialRepository/u);
  assert.match(container, /appContext\.filesDir\.absolutePath/u);
  assert.match(container, /UniffiStreamingProviderCatalogAdapter\(\)/u);
  assert.match(container, /UniffiStreamingTranscriptionAdapter\(\)/u);
  assert.match(container, /UniffiRecordingHistoryAdapter\(appDataDir\)/u);
  assert.match(container, /createLiveRecording\(scope: CoroutineScope\): LiveRecordingController/u);
  assert.match(coordinator, /:\s*LiveRecordingController/u);
  assert.match(coordinator, /credentialResolver:\s*BatchCredentialResolverPort/u);
  assert.match(
    coordinator,
    /credentialResolver\.load\(OnlineBatchProvider\.VOLCENGINE_DOUBAO\)/u,
  );
  assert.match(recordingViewModel, /controllerFactory\.create\(viewModelScope\)/u);
  assert.match(activity, /\(application as SonaApplication\)\.container/u);
  assert.doesNotMatch(activity, /Manifest\.permission\.RECORD_AUDIO/u);

  assert.match(recordScreen, /rememberLauncherForActivityResult/u);
  assert.match(recordScreen, /ActivityResultContracts\.RequestPermission/u);
  assert.match(recordScreen, /onConfigureCredential/u);
  assert.match(permissionPolicy, /SHOW_RATIONALE/u);
  assert.match(permissionPolicy, /OPEN_APP_SETTINGS/u);
  assert.match(lifecycleEffect, /ProcessLifecycleOwner/u);
  assert.match(lifecycleEffect, /Lifecycle\.Event\.ON_STOP/u);

  assert.match(settingsScreen, /NavigableListDetailPaneScaffold/u);
  assert.match(settingsScreen, /initialDestinationHistory/u);
  assert.match(settingsScreen, /requestCloudCredentialFocus/u);
  assert.match(settingsScreen, /cloudCredentialFocusSessionActive/u);
  assert.match(settingsScreen, /DisposableEffect/u);
  assert.doesNotMatch(
    settingsScreen,
    /initialSection\s*==\s*SettingsSection\.RECOGNITION/u,
  );
  assert.doesNotMatch(recognitionSettingsPane, /CredentialSettingsUiState|credentialInput/u);
  assert.match(recognitionSettingsPane, /initialFocusRequester/u);
  assert.match(recognitionSettingsPane, /keyboardController\?\.hide\(\)/u);
  assert.doesNotMatch(recognitionSettingsPane, /rememberSaveable/u);
  assert.match(cloudSettingsCard, /PasswordVisualTransformation/u);
  assert.match(cloudSettingsCard, /value = state\.apiKeyInput/u);
  assert.match(cloudSettingsCard, /onGloballyPositioned/u);
  assert.match(settingsViewModel, /apiKeyInput=<redacted>/u);
  assert.doesNotMatch(settingsViewModel, /SavedStateHandle/u);
  assert.match(navigation, /cloudCredentialFocusRequested/u);
  assert.match(
    navigation,
    /onCloudProviderSelected\(OnlineBatchProvider\.VOLCENGINE_DOUBAO\)/u,
  );
  assert.doesNotMatch(navigation, /cloudCredentialFocusRequested by rememberSaveable/u);
  assert.equal(
    fs.existsSync(path.join(
      repoRoot,
      'platforms', 'android', 'client', 'app', 'src', 'main', 'kotlin',
      'com', 'sona', 'android', 'app', 'feature', 'settings',
      'CredentialSettingsViewModel.kt',
    )),
    false,
  );
});

test('Android verification runs all recording tests in one serial Gradle invocation', () => {
  const verifier = read('scripts', 'verify-android-client.js');
  const appGradle = read('platforms', 'android', 'client', 'app', 'build.gradle.kts');
  const bindingsGradle = read('platforms', 'android', 'sona-uniffi-bindings.gradle.kts');

  for (const task of [
    ':application:testDebugUnitTest',
    ':adapters:android:testDebugUnitTest',
    ':adapters:uniffi:testDebugUnitTest',
    ':app:testDebugUnitTest',
    ':adapters:android:lintDebug',
    ':app:lintDebug',
    ':app:assembleDebug',
  ]) {
    assert.equal(verifier.includes(`'${task}'`), true, `missing Android task: ${task}`);
  }
  assert.equal(
    verifier.match(/run\(process\.execPath, \[/gu)?.length,
    1,
    'Android verification must keep one managed Gradle invocation',
  );
  assert.match(appGradle, /lifecycle-process:2\.11\.0/u);
  assert.match(appGradle, /kotlinx-coroutines-test:1\.11\.0/u);
  assert.match(
    bindingsGradle,
    /net\.java\.dev\.jna:jna:5\.19\.1@aar/u,
    'Android JNA must use the API 37-verified 16 KB-aligned release',
  );
});

test('Android app desugars JNA Java APIs for API 23', () => {
  const appGradle = read('platforms', 'android', 'client', 'app', 'build.gradle.kts');

  assert.match(appGradle, /isCoreLibraryDesugaringEnabled\s*=\s*true/u);
  assert.match(
    appGradle,
    /coreLibraryDesugaring\("com\.android\.tools:desugar_jdk_libs:2\.1\.5"\)/u,
  );
});
