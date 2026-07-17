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

test('Android online recording adapters own generated bindings and close event streams', () => {
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

  assert.match(provider, /StreamingProviderCatalogPort/u);
  assert.match(streaming, /StreamingTranscriptionPort/u);
  assert.match(streaming, /override fun onStreamingError/u);
  assert.match(streaming, /finally[\s\S]*eventChannel\.close/u);
  assert.match(history, /RecordingHistoryPort/u);
  assert.match(bindings, /createOnlineAsrStreamingSession/u);
  assert.match(bindings, /createHistoryLiveDraftJson/u);
  assert.match(bindings, /updateHistoryTranscriptJson/u);
  assert.match(bindings, /completeHistoryLiveDraftJson/u);
  assert.match(bindings, /purgeHistoryItemsJson/u);
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
    'CredentialSettingsViewModel.kt',
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
  assert.match(container, /AndroidStreamingCredentialRepository\.create\(appContext\)/u);
  assert.match(container, /appContext\.filesDir\.absolutePath/u);
  assert.match(container, /UniffiStreamingProviderCatalogAdapter\(\)/u);
  assert.match(container, /UniffiStreamingTranscriptionAdapter\(\)/u);
  assert.match(container, /UniffiRecordingHistoryAdapter\(appDataDir\)/u);
  assert.match(container, /createLiveRecording\(scope: CoroutineScope\): LiveRecordingController/u);
  assert.match(coordinator, /:\s*LiveRecordingController/u);
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
  assert.match(settingsScreen, /requestCredentialFocus/u);
  assert.match(settingsScreen, /credentialFocusSessionActive/u);
  assert.match(settingsScreen, /DisposableEffect/u);
  assert.doesNotMatch(
    settingsScreen,
    /initialSection\s*==\s*SettingsSection\.RECOGNITION/u,
  );
  assert.match(recognitionSettingsPane, /PasswordVisualTransformation/u);
  assert.match(recognitionSettingsPane, /value = state\.credentialInput/u);
  assert.match(recognitionSettingsPane, /onGloballyPositioned/u);
  assert.doesNotMatch(recognitionSettingsPane, /else if \(!focusInitialized\)/u);
  assert.doesNotMatch(recognitionSettingsPane, /rememberSaveable/u);
  assert.match(settingsViewModel, /credentialInput=<redacted>/u);
  assert.doesNotMatch(settingsViewModel, /SavedStateHandle/u);
  assert.match(navigation, /credentialFocusRequested/u);
  assert.doesNotMatch(navigation, /credentialFocusRequested by rememberSaveable/u);
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
