import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { repoRoot, read, exists, desktopCrateSegments, desktopCratePath, rustFilesUnder } from './test-support/repository.js';

test('UniFFI Android builds use the pinned local ASR runtime release', () => {
  const uniffiCargo = read('adapters', 'uniffi_bind', 'Cargo.toml');
  const localAsrCargo = read('adapters', 'local_asr', 'Cargo.toml');
  const releaseWorkflow = read('.github', 'workflows', 'release.yml');
  const nightlyWorkflow = read('.github', 'workflows', 'nightly.yml');
  const sourceLockSegments = [
    'platforms', 'android', 'packaging', 'sherpa-onnx-sources.json',
  ];

  assert.match(uniffiCargo, /^sona-local-asr\s*=\s*\{\s*path\s*=\s*"\.\.\/local_asr"\s*\}/mu);
  assert.match(
    localAsrCargo,
    /^sherpa-onnx\s*=\s*\{[^}]*version\s*=\s*"=1\.13\.4"[^}]*features\s*=\s*\["shared"\][^}]*\}/mu,
  );
  assert.match(releaseWorkflow, /SHERPA_ONNX_VERSION:\s*1\.13\.4/u);
  assert.match(nightlyWorkflow, /SHERPA_ONNX_VERSION:\s*1\.13\.4/u);
  assert.equal(exists(...sourceLockSegments), true);

  const sourceLock = JSON.parse(read(...sourceLockSegments));
  assert.deepEqual(sourceLock, {
    version: '1.13.4',
    url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.4/sherpa-onnx-v1.13.4-android.tar.bz2',
    sha256: '7983fc3de23f6e64148f2fb05fa94a2efaa8c0516cc1573383dc5c7d4d2a43b0',
    abis: ['arm64-v8a', 'armeabi-v7a', 'x86', 'x86_64'],
    runtimeLibraries: ['libsherpa-onnx-c-api.so', 'libonnxruntime.so'],
  });
});

test('shared api server invokes local batch ASR through the core transcriber port', () => {
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const tauriServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const apiCargo = read('adapters', 'api_server', 'Cargo.toml');
  const apiServer = read('adapters', 'api_server', 'src', 'lib.rs');

  assert.match(tauriCargo, /^sona-local-asr\s*=\s*\{ path = "\.\.\/\.\.\/adapters\/local_asr" \}/mu);
  assert.match(apiCargo, /^sona-local-asr\s*=\s*\{\s*path = "\.\.\/local_asr" \}/mu);
  assert.match(apiServer, /use sona_core::ports::asr::\{[\s\S]*BatchTranscriber/u);
  assert.match(apiServer, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.match(apiServer, /\.transcribe\(plan\)/u);
  assert.match(tauriServer, /use sona_api_server::\{[\s\S]*start_api_server_runtime/u);
  assert.doesNotMatch(tauriServer, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.doesNotMatch(apiServer, /run_offline_transcription/u);
  assert.doesNotMatch(apiServer, /use crate::integrations::asr::transcribe_batch_with_progress;/u);
  assert.doesNotMatch(apiServer, /LocalSherpaAdapter::offline_plan_to_batch_request/u);
});

test('model download runtime implementation lives in a dedicated adapter crate', () => {
  const workspaceCargo = read('Cargo.toml');
  const coreCargo = read('core', 'Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformModelDownloadsPath = desktopCratePath('src', 'platform', 'model_downloads.rs');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const cliModels = read('platforms', 'cli', 'src', 'models.rs');
  const desktopDownloads = fs.readFileSync(
    desktopCratePath('src', 'commands', 'downloads.rs'),
    'utf8',
  );
  const coreModelDownloads = read('core', 'src', 'models', 'downloads.rs');
  const coreModelCatalog = read('core', 'src', 'models', 'catalog.rs');
  const adapterLib = read('adapters', 'model_downloads', 'src', 'lib.rs');
  const adapterModels = read('adapters', 'model_downloads', 'src', 'models.rs');
  const adapterDownloads = read('adapters', 'model_downloads', 'src', 'downloads.rs');

  assert.match(workspaceCargo, /"adapters\/model_downloads"/u);
  assert.match(tauriCargo, /sona-model-downloads\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/model_downloads" \}/u);
  assert.match(cliCargo, /sona-model-downloads\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/model_downloads" \}/u);
  assert.match(cliModels, /use sona_model_downloads::\{download_model, installed_model_is_valid, remove_model_install_path\}/u);
  assert.equal(fs.existsSync(platformModelDownloadsPath), true);
  const platformModelDownloads = fs.readFileSync(platformModelDownloadsPath, 'utf8');
  assert.match(platformMod, /^pub mod model_downloads;/mu);
  assert.match(desktopLib, /crate::platform::model_downloads::DownloadState::new\(\)/u);
  assert.match(platformModelDownloads, /pub struct DownloadState/u);
  assert.match(platformModelDownloads, /DownloadClient/u);
  assert.doesNotMatch(platformModelDownloads, /pub downloads:/u);
  assert.doesNotMatch(platformModelDownloads, /pub client:/u);
  assert.match(platformModelDownloads, /fn client\(&self\) -> &DownloadClient/u);
  assert.match(platformModelDownloads, /async fn insert_download/u);
  assert.match(platformModelDownloads, /async fn remove_download/u);
  assert.match(platformModelDownloads, /async fn notify_download/u);
  assert.match(platformModelDownloads, /const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress"/u);
  assert.match(
    platformModelDownloads,
    /state\s*\.client\(\)\s*\.download_file\(&url, &temp_path, notify, Some\(progress_cb\)\)/u,
  );
  assert.match(platformModelDownloads, /complete_download_file/u);
  assert.match(platformModelDownloads, /temporary_download_path/u);
  assert.match(desktopDownloads, /crate::platform::model_downloads::cancel_download\(state, id\)\.await/u);
  assert.match(desktopDownloads, /crate::platform::model_downloads::has_active_downloads\(state\)\.await/u);
  assert.match(desktopDownloads, /crate::platform::model_downloads::download_file\(/u);
  assert.doesNotMatch(desktopDownloads, /sona_model_downloads/u);
  assert.doesNotMatch(desktopDownloads, /DownloadClient/u);
  assert.doesNotMatch(desktopDownloads, /DOWNLOAD_PROGRESS_EVENT/u);
  assert.doesNotMatch(desktopDownloads, /temporary_download_path|complete_download_file/u);
  assert.doesNotMatch(desktopDownloads, /reqwest::Client|Client::builder|adapter_download_file/u);
  assert.doesNotMatch(coreModelDownloads, /pub async fn download_model/u);
  assert.doesNotMatch(coreModelDownloads, /reqwest|tokio::fs|sha256_file|tar::|bzip2::/u);
  assert.doesNotMatch(coreModelCatalog, /std::fs::/u);
  assert.doesNotMatch(coreModelCatalog, /pub fn remove_model_install_path/u);
  assert.doesNotMatch(coreCargo, /^hex\s*=/mu);
  assert.doesNotMatch(coreCargo, /^sha2\s*=/mu);
  assert.equal(exists('core', 'src', 'downloads.rs'), false);
  assert.match(adapterLib, /pub use downloads::/u);
  assert.match(adapterLib, /DownloadClient/u);
  assert.match(adapterLib, /remove_model_install_path/u);
  assert.match(adapterDownloads, /pub struct DownloadClient/u);
  assert.match(adapterDownloads, /impl DownloadClient/u);
  assert.match(adapterDownloads, /user_agent\("Sona\/1\.0"\)/u);
  assert.match(adapterDownloads, /pub async fn download_file/u);
  assert.match(adapterModels, /pub fn remove_model_install_path/u);
});

test('core owns ASR runtime error contract reused by desktop', () => {
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const desktopAsrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const desktopAsrErrorPath = desktopCratePath('src', 'integrations', 'asr', 'error.rs');

  assert.match(coreAsr, /pub enum SherpaError/u);
  assert.match(coreAsr, /impl Serialize for SherpaError/u);
  assert.match(coreAsr, /UNSUPPORTED_ONLINE_PROVIDER/u);
  assert.match(coreAsr, /GENERIC_ERROR/u);
  assert.equal(fs.existsSync(desktopAsrErrorPath), false);
  assert.doesNotMatch(desktopAsrMod, /^mod error;/mu);
  assert.match(desktopAsrMod, /pub use sona_core::ports::asr::SherpaError;/u);
});

test('core owns ASR metric helpers reused by desktop', () => {
  const coreMetrics = read('core', 'src', 'transcription', 'asr_metrics.rs');
  const desktopMetrics = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'metrics.rs'),
    'utf8',
  );

  for (const helper of [
    'duration_to_ms',
    'samples_to_ms',
    'calculate_rtf',
    'calculate_rss_delta_mb',
    'format_optional_mb',
    'format_optional_ms',
    'format_optional_rtf',
    'format_optional_count',
  ]) {
    assert.match(coreMetrics, new RegExp(`pub fn ${helper}`, 'u'));
    assert.match(desktopMetrics, new RegExp(`sona_core::transcription::asr_metrics::[\\s\\S]*${helper}`, 'u'));
    assert.doesNotMatch(desktopMetrics, new RegExp(`fn ${helper}`, 'u'));
  }

  assert.doesNotMatch(coreMetrics, /SystemTime::now|UNIX_EPOCH|current_time_millis/u);
  assert.match(desktopMetrics, /pub\(crate\) fn current_time_millis/u);
  assert.match(desktopMetrics, /crate::platform::time::unix_timestamp_millis\(\)/u);
  assert.doesNotMatch(desktopMetrics, /SystemTime::now|UNIX_EPOCH/u);
  assert.match(desktopMetrics, /pub\(crate\) fn capture_process_memory_mb/u);
  assert.match(desktopMetrics, /sysinfo::/u);
});

test('local ASR runtime pool is owned by the local ASR adapter', () => {
  const localAsrLib = read('adapters', 'local_asr', 'src', 'lib.rs');
  const localAsrRuntime = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'runtime.rs'),
    'utf8',
  );
  const desktopAsrState = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'state.rs'),
    'utf8',
  );
  const desktopAsrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(localAsrLib, /^pub mod runtime;/mu);
  assert.match(localAsrRuntime, /pub struct RecognizerPool/u);
  assert.match(localAsrRuntime, /pub struct ModelConfigKey/u);
  assert.doesNotMatch(localAsrRuntime, /pub recognizers:/u);
  assert.doesNotMatch(localAsrRuntime, /pub punctuations:/u);
  assert.match(localAsrRuntime, /pub async fn recognizer_cell_for_gpu_plan/u);
  assert.match(localAsrRuntime, /pub async fn register_recognizer_gpu_provider/u);
  assert.match(localAsrRuntime, /pub async fn punctuation_cell_for_path/u);
  assert.match(desktopAsrState, /use sona_local_asr::runtime::RecognizerPool;/u);
  assert.doesNotMatch(desktopAsrState, /pub struct RecognizerPool/u);
  assert.doesNotMatch(desktopAsrState, /pub struct ModelConfigKey/u);
  assert.match(desktopAsrMod, /pub use sona_local_asr::runtime::RecognizerPool;/u);
  assert.match(desktopAsrMod, /pub\(crate\) use sona_local_asr::runtime::ModelConfigKey;/u);

  for (const desktopFile of [
    ...rustFilesUnder(desktopCratePath('src', 'integrations', 'asr')),
    desktopCratePath('src', 'integrations', 'streaming.rs'),
  ]) {
    const content = fs.readFileSync(desktopFile, 'utf8');
    assert.doesNotMatch(content, /\.recognizers\.lock\(\)/u);
    assert.doesNotMatch(content, /\.recognizers\.insert/u);
    assert.doesNotMatch(content, /\.recognizers\.get/u);
    assert.doesNotMatch(content, /\.punctuations\.lock\(\)/u);
  }
});

test('desktop API server obtains ASR recognizer pools through integration facade', () => {
  const appServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const desktopAsrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(desktopAsrMod, /pub\(crate\) fn recognizer_pool_for_app/u);
  assert.match(appServer, /crate::integrations::asr::recognizer_pool_for_app\(app\.as_ref\(\)\)/u);
  assert.doesNotMatch(appServer, /state::<crate::integrations::asr::AsrState>\(\)\s*\.recognizer_pool/u);
  assert.doesNotMatch(appServer, /RecognizerPool::new/u);
});

test('desktop ASR session maps are owned by AsrState', () => {
  const stateRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'state.rs');
  const commandsAsr = read(...desktopCrateSegments, 'src', 'commands', 'asr.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');

  assert.match(stateRs, /pub async fn insert_session/u);
  assert.match(stateRs, /pub async fn session/u);
  assert.doesNotMatch(stateRs, /pub active_sessions:/u);
  assert.doesNotMatch(stateRs, /pub instance_engines:/u);
  assert.doesNotMatch(commandsAsr, /active_sessions\.lock\(\)/u);
  assert.doesNotMatch(asrMod, /active_sessions\.lock\(\)/u);
});

test('desktop standalone streaming uses local ASR recognizer accessors', () => {
  const streaming = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');

  assert.doesNotMatch(streaming, /RecognizerInner/u);
  assert.doesNotMatch(streaming, /\b(?:recognizer|r)\.inner\b/u);
  assert.doesNotMatch(asrMod, /\bRecognizerInner\b/u);
});

test('desktop batch ASR uses local ASR recognizer accessors', () => {
  const batch = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');

  assert.doesNotMatch(batch, /RecognizerInner/u);
  assert.doesNotMatch(batch, /\brecognizer\.inner\b/u);
});

test('local streaming ASR uses local ASR recognizer accessors', () => {
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );

  assert.doesNotMatch(adapterSession, /RecognizerInner/u);
  assert.doesNotMatch(adapterSession, /\b(?:recognizer|recognizer_copy)\.inner\b/u);
});

test('local ASR recognizer internals stay behind accessors', () => {
  const localAsrRecognizer = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'),
    'utf8',
  );

  for (const helper of ['kind_label', 'is_offline', 'offline', 'online']) {
    assert.match(localAsrRecognizer, new RegExp(`pub fn ${helper}\\(&self\\)`, 'u'));
  }
  assert.doesNotMatch(localAsrRecognizer, /pub enum RecognizerInner/u);
  assert.doesNotMatch(localAsrRecognizer, /pub inner: RecognizerInner/u);
});

test('streaming ASR session contract is core-owned and platform-neutral', () => {
  const coreAsr = fs.readFileSync(
    path.join(repoRoot, 'core', 'src', 'ports', 'asr.rs'),
    'utf8',
  );
  const tauriTraits = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'traits.rs'),
    'utf8',
  );
  const tauriState = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'state.rs'),
    'utf8',
  );
  const tauriCommands = fs.readFileSync(
    desktopCratePath('src', 'commands', 'asr.rs'),
    'utf8',
  );

  assert.match(coreAsr, /pub trait AsrStreamingSession/u);
  assert.match(coreAsr, /pub trait AsrRuntimeObserver/u);
  assert.doesNotMatch(
    coreAsr,
    /AppHandle|AsrState|TauriEventEmitter|crate::platform/u,
  );
  assert.doesNotMatch(tauriTraits, /trait AsrStreamingSession/u);
  assert.match(
    tauriState,
    /sona_core::ports::asr::AsrStreamingSession/u,
  );
  assert.match(tauriCommands, /session\.start\(\)\.await/u);
  assert.match(tauriCommands, /session\.flush\(\)\.await/u);
});

test('ASR provider resolution policy is core-owned and shared by host bindings', () => {
  const coreResolution = read(
    'core', 'src', 'transcription', 'provider_resolution.rs',
  );
  const desktopRegistry = read(
    'platforms', 'desktop', 'src', 'integrations', 'asr', 'mod.rs',
  );
  const uniffiStreaming = read(
    'adapters', 'uniffi_bind', 'src', 'asr_streaming_bridge.rs',
  );

  assert.match(coreResolution, /pub fn resolve_asr_provider_id/u);
  assert.match(coreResolution, /pub fn resolve_asr_streaming_provider_id/u);
  assert.doesNotMatch(coreResolution, /tauri::|uniffi::|sona_(?:local|online)_asr/u);
  for (const [hostBinding, resolver] of [
    [desktopRegistry, 'resolve_asr_provider_id'],
    [uniffiStreaming, 'resolve_asr_streaming_provider_id'],
  ]) {
    assert.match(hostBinding, new RegExp(`${resolver}\\(`, 'u'));
    assert.doesNotMatch(
      hostBinding,
      /find_online_asr_provider|\.streaming\.supported/u,
    );
  }
});

test('local streaming ASR session is implemented by the local adapter', () => {
  const localAsrRoot = path.join(repoRoot, 'adapters', 'local_asr');
  const adapterSessionPath = path.join(
    localAsrRoot, 'src', 'streaming', 'session.rs',
  );
  const adapterStreamingMod = fs.readFileSync(
    path.join(localAsrRoot, 'src', 'streaming', 'mod.rs'),
    'utf8',
  );
  const localAsrSources = rustFilesUnder(path.join(localAsrRoot, 'src'))
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');
  const localAsrCargo = fs.readFileSync(path.join(localAsrRoot, 'Cargo.toml'), 'utf8');
  const desktopSessionPath = desktopCratePath('src', 'integrations', 'asr', 'sherpa_onnx.rs');
  const desktopSchedulerPath = desktopCratePath('src', 'platform', 'asr_runtime.rs');
  const provider = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'adapter.rs'),
    'utf8',
  );

  assert.equal(fs.existsSync(adapterSessionPath), true);
  assert.equal(fs.existsSync(desktopSessionPath), false);
  assert.equal(fs.existsSync(desktopSchedulerPath), false);
  const adapterSession = fs.readFileSync(adapterSessionPath, 'utf8');
  assert.match(adapterSession, /impl AsrStreamingSession for LocalSherpaSession/u);
  assert.doesNotMatch(adapterSession, /tauri::|crate::platform|AsrState/u);
  assert.match(adapterStreamingMod, /^mod inference;$/mu);
  assert.doesNotMatch(
    `${localAsrSources}\n${localAsrCargo}`,
    /tauri|AsrState|crate::platform/iu,
  );
  assert.match(provider, /sona_local_asr::streaming::create_streaming_session/u);
});

test('local ASR streaming runtime state is owned by the local ASR adapter', () => {
  const localAsrRuntime = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'runtime.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const desktopAsr = rustFilesUnder(
    desktopCratePath('src', 'integrations', 'asr'),
  )
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');
  const desktopStreaming = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'streaming.rs'),
    'utf8',
  );

  for (const symbol of ['SherpaInstance', 'OfflineState', 'RecordDiagnosticsState']) {
    assert.match(localAsrRuntime, new RegExp(`pub struct ${symbol}`, 'u'));
    assert.doesNotMatch(desktopAsr, new RegExp(`pub struct ${symbol}`, 'u'));
  }

  for (const field of ['vad', 'vad_model', 'vad_buffer']) {
    assert.doesNotMatch(localAsrRuntime, new RegExp(`pub ${field}:`, 'u'));
  }

  for (const helper of [
    'configure_vad',
    'reset_or_reload_vad',
    'has_vad_configuration',
    'vad',
    'vad_buffer',
  ]) {
    assert.match(localAsrRuntime, new RegExp(`pub fn ${helper}\\(`, 'u'));
  }

  for (const field of [
    'speech_buffer',
    'ring_buffer',
    'is_speaking',
    'last_inference_time',
    'utterance_start_sample',
  ]) {
    assert.doesNotMatch(localAsrRuntime, new RegExp(`pub ${field}:`, 'u'));
  }

  for (const field of [
    'first_sample_logged',
    'skipped_while_stopped_logged',
    'first_segment_emitted',
    'is_running',
    'recognizer',
    'punctuation',
    'stream',
    'last_partial_metric_sample',
  ]) {
    assert.doesNotMatch(localAsrRuntime, new RegExp(`pub ${field}:`, 'u'));
    assert.doesNotMatch(
      adapterSession,
      field === 'is_running'
        ? /\binstance\.is_running(?!\()/u
        : field === 'recognizer'
          ? /\binstance\.recognizer\b(?!\s*\()/u
        : field === 'punctuation'
          ? /\binstance\.punctuation\b(?!\s*\()/u
        : field === 'stream'
          ? /\binstance\.stream(?!\s*\()/u
        : field === 'last_partial_metric_sample'
          ? /\binstance\.last_partial_metric_sample\b/u
          : new RegExp(`record_diagnostics[\\s\\S]{0,80}\\.${field}\\b`, 'u'),
    );
  }

  assert.match(adapterSession, /pub struct LocalSherpaSession/u);
  assert.match(adapterSession, /pub async fn create_streaming_session/u);
  assert.doesNotMatch(adapterSession, /pub instance:/u);

  for (const helper of [
    'buffered_sample_count',
    'begin_speech',
    'buffered_speech_chunk_count',
    'buffered_speech_sample_count',
    'clear_speech_buffer',
    'push_speech_chunk',
    'finish_speech_with_chunk',
    'push_ring_chunk',
    'push_ring_chunk_with_sample_limit',
    'ring_sample_count',
    'speech_chunks',
    'should_run_inference',
    'mark_inference_time',
    'utterance_start_seconds',
    'start_instance_runtime',
    'stop_instance_runtime',
    'should_log_first_sample',
    'mark_first_sample_logged',
    'should_log_skipped_while_stopped',
    'mark_skipped_while_stopped_logged',
    'first_segment_emitted_flag',
    'is_running',
    'recognizer',
    'recognizer_clone',
    'set_recognizer',
    'punctuation',
    'punctuation_clone',
    'has_punctuation',
    'set_punctuation',
    'stream',
    'take_stream',
    'restore_stream',
    'should_record_partial_metric',
    'mark_partial_metric_sample',
    'clear_partial_metric_sample',
  ]) {
    assert.match(localAsrRuntime, new RegExp(`pub fn ${helper}`, 'u'));
    assert.doesNotMatch(adapterSession, new RegExp(`pub fn ${helper}`, 'u'));
  }

  assert.match(adapterSession, /use crate::runtime::\{[\s\S]*SherpaInstance/u);
  assert.match(desktopStreaming, /sona_local_asr::runtime::OfflineState/u);
  assert.doesNotMatch(
    adapterSession,
    /offline_state\.(?:is_speaking|ring_buffer|speech_buffer|utterance_start_sample|last_inference_time)\b/u,
  );
  assert.doesNotMatch(
    desktopStreaming,
    /offline_state\.(?:is_speaking|ring_buffer|speech_buffer|utterance_start_sample)\b/u,
  );
});

test('core owns local batch ASR request contract reused by desktop', () => {
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const desktopAsrTypes = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'types.rs'),
    'utf8',
  );
  const desktopAsrAdapter = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'adapter.rs'),
    'utf8',
  );
  const desktopBatchProcessor = desktopAsrAdapter.slice(
    desktopAsrAdapter.indexOf('pub struct LocalSherpaBatchProcessor'),
  );

  assert.match(coreAsr, /pub struct BatchTranscriptionRequest/u);
  assert.match(coreAsr, /pub struct LocalSherpaStreamingRequest/u);
  assert.match(coreAsr, /pub instance_id: Option<String>/u);
  assert.match(coreAsr, /pub instance_id: String/u);
  assert.match(coreAsr, /pub postprocessor: TranscriptPostprocessor/u);
  assert.match(coreAsr, /pub fn validate_local_sherpa_mode/u);
  assert.match(coreAsr, /pub fn from_local_sherpa_request/u);
  assert.match(desktopAsrTypes, /BatchTranscriptionRequest/u);
  assert.match(desktopAsrTypes, /LocalSherpaStreamingRequest/u);
  assert.doesNotMatch(desktopAsrTypes, /pub struct BatchTranscriptionRequest/u);
  assert.doesNotMatch(desktopAsrTypes, /pub struct LocalSherpaStreamingRequest/u);
  assert.match(desktopAsrAdapter, /validate_local_sherpa_mode\(request, AsrMode::Batch\)/u);
  assert.match(desktopAsrAdapter, /BatchTranscriptionRequest::from_local_sherpa_request/u);
  assert.match(desktopAsrAdapter, /LocalSherpaStreamingRequest::from_local_sherpa_request/u);
  assert.doesNotMatch(desktopBatchProcessor, /AsrEngineConfig::LocalSherpa/u);
  assert.doesNotMatch(desktopAsrAdapter, /AsrEngineConfig::LocalSherpa/u);
  assert.doesNotMatch(desktopAsrAdapter, /fn ensure_mode/u);
});

test('online ASR provider manifest is owned by core and used directly by desktop', () => {
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const coreManifestPath = path.join(repoRoot, 'core', 'src', 'ports', 'online-asr-providers.json');
  const legacySharedManifestPath = path.join(repoRoot, 'src', 'shared', 'online-asr-providers.json');
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const apiServer = read('adapters', 'api_server', 'src', 'lib.rs');
  const streamingRs = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');
  const onlineAdapterRs = ['groq.rs', 'mistral.rs', 'volcengine.rs']
    .map((file) => read(...desktopCrateSegments, 'src', 'integrations', 'asr', file))
    .join('\n');
  const tsBindLib = read('adapters', 'ts_bind', 'src', 'lib.rs');
  const onlineProvidersTs = read('platforms', 'desktop', 'frontend', 'src', 'services', 'onlineAsrProviders.ts');
  const asrConfigServiceTest = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'desktop', 'frontend', 'src', 'services', '__tests__', 'asrConfigService.test.ts'),
    'utf8',
  );

  assert.ok(fs.existsSync(coreManifestPath));
  assert.equal(fs.existsSync(legacySharedManifestPath), false);
  assert.match(coreAsr, /ONLINE_ASR_PROVIDERS_JSON/u);
  assert.match(coreAsr, /include_str!\("online-asr-providers\.json"\)/u);
  assert.doesNotMatch(coreAsr, /src\/shared\/online-asr-providers\.json/u);
  assert.match(coreAsr, /pub const VOLCENGINE_DOUBAO_PROVIDER_ID/u);
  assert.match(coreAsr, /pub fn online_asr_providers\(\)/u);
  assert.match(coreAsr, /pub fn find_online_asr_provider/u);
  assert.match(coreAsr, /pub struct OnlineAsrProvider/u);
  assert.match(coreAsr, /pub struct OnlineAsrBatchCapability/u);
  assert.match(coreAsr, /pub fn provider_id\(&self\) -> &str/u);

  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'asr_providers.rs'), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod asr_providers;/mu);
  assert.match(apiServer, /sona_core::ports::asr::\{[^}]*find_online_asr_provider[^}]*online_asr_providers/u);
  assert.match(streamingRs, /sona_core::ports::asr::find_online_asr_provider/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::GROQ_WHISPER_PROVIDER_ID/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::MISTRAL_VOXTRAL_PROVIDER_ID/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::VOLCENGINE_DOUBAO_PROVIDER_ID/u);

  assert.match(tsBindLib, /OnlineAsrProvider/u);
  assert.match(tsBindLib, /OnlineAsrCapability/u);
  assert.match(tsBindLib, /OnlineAsrBatchCapability/u);
  assert.match(tsBindLib, /OnlineAsrLocalFileBatchMode/u);
  assert.match(onlineProvidersTs, /\.\.\/\.\.\/\.\.\/\.\.\/\.\.\/core\/src\/ports\/online-asr-providers\.json/u);
  assert.match(asrConfigServiceTest, /\.\.\/\.\.\/\.\.\/\.\.\/\.\.\/\.\.\/core\/src\/ports\/online-asr-providers\.json/u);
});

test('preset model catalog data is owned by core and reused by frontend', () => {
  const corePresetModels = read('core', 'src', 'models', 'preset_models.rs');
  const corePresetModelsPath = path.join(repoRoot, 'core', 'src', 'models', 'preset-models.json');
  const legacySharedPresetModelsPath = path.join(repoRoot, 'src', 'shared', 'preset-models.json');
  const modelServiceTs = read('platforms', 'desktop', 'frontend', 'src', 'services', 'modelService.ts');

  assert.ok(fs.existsSync(corePresetModelsPath));
  assert.equal(fs.existsSync(legacySharedPresetModelsPath), false);
  assert.match(corePresetModels, /include_str!\("preset-models\.json"\)/u);
  assert.doesNotMatch(corePresetModels, /src\/shared\/preset-models\.json/u);
  assert.match(modelServiceTs, /\.\.\/\.\.\/\.\.\/\.\.\/\.\.\/core\/src\/models\/preset-models\.json/u);
});

test('core model path resolution is adapter-driven without desktop filesystem probes', () => {
  const coreModelPaths = read('core', 'src', 'models', 'paths.rs');
  const cliDesktopPaths = read('platforms', 'cli', 'src', 'desktop_paths.rs');
  const cliModels = read('platforms', 'cli', 'src', 'models.rs');
  const coreTranscribeRuntime = read('core', 'src', 'transcription', 'runtime.rs');
  const coreServeRuntime = read('core', 'src', 'runtime', 'serve.rs');

  assert.match(coreModelPaths, /pub enum ModelsDirStatus/u);
  assert.match(coreModelPaths, /status_of/u);
  assert.doesNotMatch(coreModelPaths, /default_desktop_models_dir/u);
  assert.doesNotMatch(coreModelPaths, /std::fs::metadata/u);
  assert.match(cliDesktopPaths, /pub fn models_dir_status/u);
  assert.match(cliModels, /crate::desktop_paths::default_models_dir/u);
  assert.match(cliModels, /crate::desktop_paths::models_dir_status/u);
  assert.match(coreTranscribeRuntime, /default_models_dir: Option<PathBuf>/u);
  assert.match(coreServeRuntime, /default_models_dir: Option<PathBuf>/u);
});

test('desktop platform adapters own Tauri path event diagnostics and preset model bridges', () => {
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformPaths = read(...desktopCrateSegments, 'src', 'platform', 'paths.rs');
  const platformEvent = read(...desktopCrateSegments, 'src', 'platform', 'event.rs');
  const platformPresetModels = fs.readFileSync(
    desktopCratePath('src', 'platform', 'preset_models.rs'),
    'utf8',
  );
  const platformDiagnostics = fs.readFileSync(
    desktopCratePath('src', 'platform', 'diagnostics.rs'),
    'utf8',
  );
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');

  assert.match(desktopPlatform, /^pub mod paths;/mu);
  assert.match(desktopPlatform, /^pub mod event;/mu);
  assert.match(desktopPlatform, /^pub mod preset_models;/mu);
  assert.match(desktopPlatform, /^pub mod diagnostics;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'paths.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'event.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'preset_models.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'diagnostics.rs'), false);
  assert.match(platformPaths, /pub struct TauriPathProvider/u);
  assert.match(platformPaths, /impl<R: Runtime> PathProvider for TauriPathProvider<R>/u);
  assert.match(platformEvent, /pub struct TauriEventEmitter<R: Runtime>\(pub AppHandle<R>\)/u);
  assert.match(platformEvent, /impl<R: Runtime> EventEmitter for TauriEventEmitter<R>/u);
  assert.match(platformPresetModels, /pub use sona_core::models::preset_models::\*/u);
  assert.match(platformPresetModels, /tauri::async_runtime::spawn_blocking/u);
  assert.match(platformPresetModels, /pub async fn get_model_catalog_snapshot_for_app/u);
  assert.match(platformPresetModels, /pub async fn resolve_model_catalog_selected_ids_for_app/u);
  assert.match(platformDiagnostics, /pub use sona_core::runtime::diagnostics::\{/u);
  assert.match(platformDiagnostics, /crate::platform::paths::\{PathKind, PathProvider\}/u);
  assert.match(platformDiagnostics, /pub async fn get_diagnostics_core_snapshot_for_app/u);
  assert.match(platformDiagnostics, /DiagnosticsService::new/u);
  assert.match(platformDiagnostics, /FsDiagnosticsEnrichmentRepository::new/u);
  assert.match(platformDiagnostics, /tauri::async_runtime::spawn_blocking/u);
  assert.doesNotMatch(platformDiagnostics, /resolve_model_catalog_selected_ids/u);
  assert.match(systemCommand, /crate::platform::preset_models::get_model_catalog_snapshot_for_app\(&app\)\.await/u);
  assert.match(systemCommand, /crate::platform::preset_models::resolve_model_catalog_selected_ids_for_app\(&app, paths\)\.await/u);
  assert.match(systemCommand, /crate::platform::diagnostics::get_diagnostics_core_snapshot_for_app\(&app, state, input\)\.await/u);
  assert.doesNotMatch(systemCommand, /TauriPathProvider/u);
});

test('standalone CLI invokes local batch ASR through the core transcriber port', () => {
  const cliAsrAdapterRs = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'cli', 'src', 'asr_adapter.rs'),
    'utf8',
  );
  const cliTranscribeRs = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'cli', 'src', 'transcribe.rs'),
    'utf8',
  );

  assert.match(cliAsrAdapterRs, /use sona_core::ports::asr::BatchTranscriber;/u);
  assert.match(cliAsrAdapterRs, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.match(cliTranscribeRs, /use sona_core::ports::asr::BatchTranscriber;/u);
  assert.match(cliTranscribeRs, /crate::asr_adapter::local_batch_transcriber\(\)/u);
  assert.match(cliTranscribeRs, /\.transcribe\(plan\)/u);
  assert.doesNotMatch(cliTranscribeRs, /run_offline_transcription/u);
});

test('recognizer transcript utilities are owned by core and reused by adapters', () => {
  const coreTranscript = read('core', 'src', 'transcription', 'transcript.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');
  const tauriTranscript = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const localBatch = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'batch.rs'),
    'utf8',
  );

  assert.match(coreTranscript, /pub fn normalize_recognizer_text\(/u);
  assert.match(coreTranscript, /pub fn synthesize_durations\(/u);
  assert.match(asrMod, /pub use sona_core::transcription::postprocess::TranscriptPostprocessor/u);
  assert.doesNotMatch(asrMod, /^mod postprocess;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'asr', 'postprocess.rs'), false);
  assert.match(
    tauriTranscript,
    /pub\(crate\) use sona_core::transcription::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.match(
    localBatch,
    /use sona_core::transcription::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn normalize_recognizer_text/u);
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn synthesize_durations/u);
  assert.doesNotMatch(localBatch, /^fn normalize_recognizer_text/mu);
  assert.doesNotMatch(localBatch, /^fn synthesize_durations/mu);
});

test('timeline transcript normalization is owned by core and reused by desktop', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const coreTranscript = read('core', 'src', 'transcription', 'transcript.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');
  const tauriTranscript = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const groqRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'groq.rs');
  const mistralRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mistral.rs');

  assert.match(coreTranscript, /pub fn apply_timeline_normalization_with_id_generator/u);
  assert.match(coreTranscript, /pub fn build_transcript_update_with_id_generator/u);
  assert.doesNotMatch(coreTranscript, /Uuid::new_v4|uuid::Uuid::new_v4/u);
  assert.doesNotMatch(coreCargo, /^uuid\s*=/mu);
  assert.match(asrMod, /pub\(crate\) use transcript::\{[\s\S]*apply_timeline_normalization[\s\S]*\};/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn apply_timeline_normalization\(/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn build_transcript_update\(/u);
  assert.match(tauriTranscript, /apply_timeline_normalization_with_id_generator/u);
  assert.match(tauriTranscript, /build_transcript_update_with_id_generator/u);
  assert.match(tauriTranscript, /uuid::Uuid::new_v4\(\)\.to_string\(\)/u);
  assert.doesNotMatch(tauriTranscript, /struct TokenMap/u);
  assert.doesNotMatch(tauriTranscript, /struct SplitterState/u);
  assert.doesNotMatch(tauriTranscript, /fn split_segment_by_parts/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn emit_transcript_update/u);
  assert.match(groqRs, /use super::apply_timeline_normalization;/u);
  assert.match(mistralRs, /use super::apply_timeline_normalization;/u);
  assert.doesNotMatch(groqRs, /crate::integrations::asr::transcript::apply_timeline_normalization/u);
  assert.doesNotMatch(mistralRs, /crate::integrations::asr::transcript::apply_timeline_normalization/u);
});

test('desktop Groq and Mistral batch providers delegate HTTP work to online ASR adapter', () => {
  const workspaceCargo = read('Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const onlineAsrCargoPath = path.join(repoRoot, 'adapters', 'online_asr', 'Cargo.toml');
  const onlineAsrLibPath = path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs');
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const groqRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'groq.rs');
  const mistralRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mistral.rs');

  assert.match(workspaceCargo, /"adapters\/online_asr"/u);
  assert.ok(fs.existsSync(onlineAsrCargoPath));
  assert.ok(fs.existsSync(onlineAsrLibPath));
  assert.match(tauriCargo, /sona-online-asr\s*=\s*\{\s*path\s*=\s*"..\/..\/adapters\/online_asr"/u);
  assert.match(prWorkflow, /sona-online-asr/u);
  assert.match(coreAsr, /trait OnlineBatchTranscriber/u);
  assert.match(coreAsr, /struct OnlineBatchTranscriptionRequest/u);
  assert.match(coreAsr, /struct OnlineBatchTranscriptionOutput/u);

  for (const providerRs of [groqRs, mistralRs]) {
    assert.match(providerRs, /sona_online_asr::/u);
    assert.doesNotMatch(providerRs, /reqwest::multipart/u);
    assert.doesNotMatch(providerRs, /reqwest::Client/u);
    assert.doesNotMatch(providerRs, /\.post\(/u);
  }
});

test('desktop Volcengine batch provider delegates HTTP work to online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /VolcengineDoubaoBatchTranscriber/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config/u);
  assert.match(onlineAsrLib, /build_volcengine_flash_batch_request_body/u);
  assert.match(onlineAsrLib, /segments_from_volcengine_response/u);
  assert.match(volcengineRs, /sona_online_asr::VolcengineDoubaoBatchTranscriber/u);
  assert.doesNotMatch(volcengineRs, /reqwest::Client/u);
  assert.doesNotMatch(volcengineRs, /\.post\(/u);
  assert.doesNotMatch(volcengineRs, /base64::engine::general_purpose::STANDARD\.encode/u);
});

test('desktop Volcengine streaming protocol helpers are owned by online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const onlineStreaming = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'volcengine', 'streaming.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /build_volcengine_full_client_request_frame/u);
  assert.match(onlineAsrLib, /build_volcengine_audio_frame/u);
  assert.match(onlineAsrLib, /parse_volcengine_server_response_frame/u);
  assert.match(onlineAsrLib, /volcengine_streaming_segments_from_response/u);
  assert.match(onlineAsrLib, /f32_samples_to_i16_pcm_bytes/u);
  assert.match(onlineStreaming, /crate::build_volcengine_full_client_request_frame/u);
  assert.match(onlineStreaming, /crate::parse_volcengine_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /build_volcengine_full_client_request_frame/u);
  assert.doesNotMatch(volcengineRs, /parse_volcengine_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /pub fn build_audio_frame/u);
  assert.doesNotMatch(volcengineRs, /fn parse_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /fn f32_samples_to_i16_pcm_bytes/u);
});

test('Volcengine streaming session is implemented by the online ASR adapter', () => {
  const onlineStreamingPath = path.join(
    repoRoot, 'adapters', 'online_asr', 'src', 'volcengine', 'streaming.rs',
  );
  const onlineCargo = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'Cargo.toml'),
    'utf8',
  );
  const desktopCargo = fs.readFileSync(
    desktopCratePath('Cargo.toml'),
    'utf8',
  );
  const desktopVolcengine = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );
  const desktopAsr = rustFilesUnder(
    desktopCratePath('src', 'integrations', 'asr'),
  )
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');

  assert.equal(fs.existsSync(onlineStreamingPath), true);
  const onlineStreaming = fs.readFileSync(onlineStreamingPath, 'utf8');
  assert.match(onlineStreaming, /impl AsrStreamingSession for VolcengineStreamingSession/u);
  assert.match(onlineStreaming, /tokio_tungstenite::connect_async/u);
  assert.match(onlineStreaming, /tokio::spawn/u);
  assert.doesNotMatch(onlineStreaming, /tauri::|AsrState|crate::platform/u);
  assert.match(
    desktopVolcengine,
    /sona_online_asr::create_volcengine_streaming_session/u,
  );
  assert.doesNotMatch(
    desktopVolcengine,
    /VolcengineWriter|connect_async|tauri::async_runtime::spawn|parse_volcengine_server_response_frame/u,
  );
  assert.doesNotMatch(desktopAsr, /observe_streaming_transcript_update/u);
  assert.match(onlineCargo, /^tokio-tungstenite\s*=/mu);
  assert.doesNotMatch(desktopCargo, /^tokio-tungstenite\s*=/mu);
});

test('desktop Volcengine config and response helpers are owned by online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /VolcengineConfigError/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config_checked/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config_from_value_checked/u);
  assert.match(onlineAsrLib, /build_volcengine_flash_batch_request_body/u);
  assert.match(onlineAsrLib, /segments_from_volcengine_response/u);
  assert.match(onlineAsrLib, /map_volcengine_status_error/u);
  assert.match(volcengineRs, /sona_online_asr::resolve_volcengine_config_checked/u);
  assert.doesNotMatch(volcengineRs, /pub enum VolcengineMode/u);
  assert.doesNotMatch(volcengineRs, /pub struct VolcengineDoubaoConfigFields/u);
  assert.doesNotMatch(volcengineRs, /fn config_fields/u);
  assert.doesNotMatch(volcengineRs, /pub fn validate_config/u);
  assert.doesNotMatch(volcengineRs, /fn detect_audio_format/u);
  assert.doesNotMatch(volcengineRs, /fn build_flash_batch_request_body/u);
  assert.doesNotMatch(volcengineRs, /pub fn segments_from_response_value/u);
  assert.doesNotMatch(volcengineRs, /fn segment_from_utterance/u);
  assert.doesNotMatch(volcengineRs, /fn ms_value/u);
  assert.doesNotMatch(volcengineRs, /pub fn map_status_error/u);
});

test('desktop hardware GPU adapter lives in platform layer', () => {
  const appMod = read(...desktopCrateSegments, 'src', 'app', 'mod.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');
  const streamingRs = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const hardwarePath = desktopCratePath('src', 'platform', 'hardware.rs');

  assert.equal(fs.existsSync(hardwarePath), true);
  const hardwareRs = fs.readFileSync(hardwarePath, 'utf8');

  assert.match(platformMod, /^pub mod hardware;/mu);
  assert.doesNotMatch(appMod, /^pub mod hardware;/mu);
  assert.match(hardwareRs, /pub\(crate\) use sona_local_asr::gpu::\{/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::check_gpu_availability\(\)\.await/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::resolve_gpu_acceleration_plan/u);
  assert.match(systemCommand, /crate::platform::hardware::check_gpu_availability\(\)\.await/u);
  assert.match(batchRs, /crate::platform::hardware::resolve_gpu_acceleration_plan/u);
  assert.match(streamingRs, /crate::platform::hardware::resolve_gpu_acceleration_plan/u);
  assert.match(adapterSession, /use crate::gpu::resolve_gpu_acceleration_plan/u);
  assert.doesNotMatch(hardwareRs, /tokio::process::Command/u);
  assert.doesNotMatch(hardwareRs, /struct GpuAccelerationPlan/u);
  assert.doesNotMatch(hardwareRs, /struct GpuFallbackNotice/u);
  assert.doesNotMatch(systemCommand, /crate::app::hardware/u);
  assert.doesNotMatch(batchRs, /crate::app::hardware/u);
  assert.doesNotMatch(streamingRs, /crate::app::hardware/u);
  assert.doesNotMatch(adapterSession, /crate::app::hardware/u);
});

test('desktop local audio helpers come from local ASR adapter without Tauri core pipeline', () => {
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');
  const desktopAudio = read(...desktopCrateSegments, 'src', 'integrations', 'audio.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformAudioStoragePath = desktopCratePath('src', 'platform', 'audio_storage.rs');
  const platformAudioStorage = fs.existsSync(platformAudioStoragePath) ? fs.readFileSync(platformAudioStoragePath, 'utf8') : '';
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const localAsrSpeakerProcessing = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs'),
    'utf8',
  );
  const runtimeStatusRs = fs.readFileSync(
    desktopCratePath('src', 'platform', 'runtime_status.rs'),
    'utf8',
  );
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'pipeline.rs'), false);
  assert.match(batchRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(batchRs, /sona_local_asr::audio::save_wav_file/u);
  assert.match(localAsrSpeakerProcessing, /crate::audio::extract_and_resample_audio/u);
  assert.match(localAsrSpeakerProcessing, /crate::audio::save_wav_file/u);
  assert.match(runtimeStatusRs, /sona_local_asr::audio::resolve_ffmpeg_sidecar_path/u);
  assert.equal(fs.existsSync(platformAudioStoragePath), true);
  assert.match(platformMod, /^pub mod audio_storage;/mu);
  assert.match(platformAudioStorage, /pub fn create_history_recording_path_for_app/u);
  assert.match(platformAudioStorage, /sona_runtime_fs::ensure_directory_exists\(&history_dir\)/u);
  assert.match(desktopAudio, /crate::platform::audio_storage::create_history_recording_path_for_app\(&app\)/u);
  assert.match(desktopAudio, /sona_local_asr::audio::LiveWavRecorder/u);
  assert.doesNotMatch(desktopAudio, /sona_runtime_fs::ensure_directory_exists/u);
  assert.doesNotMatch(desktopAudio, /TauriPathProvider|PathKind|PathProvider/u);
  assert.doesNotMatch(desktopAudio, /std::fs::create_dir_all/u);
  assert.doesNotMatch(desktopAudio, /\bhound::/u);
  assert.doesNotMatch(tauriCargo, /^hound\s*=/mu);
  assert.doesNotMatch(prWorkflow, /core::pipeline::tests/u);

  const desktopPipelineReferences = rustFilesUnder(desktopCratePath('src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /crate::core::pipeline|core::pipeline/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopPipelineReferences, []);
});

test('local ASR blocking tasks are owned by the local adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformAsrRuntimePath = desktopCratePath('src', 'platform', 'asr_runtime.rs');
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );

  assert.equal(fs.existsSync(platformAsrRuntimePath), false);
  assert.doesNotMatch(platformMod, /^pub mod asr_runtime;/mu);
  assert.match(adapterSession, /tokio::task::spawn_blocking\(task\)/u);
  assert.match(adapterSession, /drop\(tokio::task::spawn_blocking\(task\)\)/u);
  assert.doesNotMatch(adapterSession, /tauri::async_runtime::spawn_blocking/u);
});

test('desktop system audio mute command is owned by platform adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const commandAudio = read(...desktopCrateSegments, 'src', 'commands', 'audio.rs');
  const desktopAudio = read(...desktopCrateSegments, 'src', 'integrations', 'audio.rs');
  const platformSystemAudioPath = desktopCratePath('src', 'platform', 'system_audio.rs');

  assert.equal(fs.existsSync(platformSystemAudioPath), true);
  const platformSystemAudio = fs.readFileSync(platformSystemAudioPath, 'utf8');

  assert.match(platformMod, /^pub mod system_audio;/mu);
  assert.match(commandAudio, /crate::platform::system_audio::set_system_audio_mute\(mute\)\.await/u);
  assert.match(platformSystemAudio, /pub async fn set_system_audio_mute/u);
  assert.match(platformSystemAudio, /set_mute_windows/u);
  assert.match(platformSystemAudio, /set_mute_macos/u);
  assert.match(platformSystemAudio, /set_mute_linux/u);
  assert.doesNotMatch(desktopAudio, /pub async fn set_system_audio_mute/u);
  assert.doesNotMatch(desktopAudio, /set_mute_windows|set_mute_macos|set_mute_linux/u);
  assert.doesNotMatch(desktopAudio, /std::process::Command|Command::new|IAudioEndpointVolume/u);
});

test('desktop batch ASR delegates audio segmentation to local ASR adapter', () => {
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');

  assert.match(batchRs, /sona_local_asr::audio::segment_batch_audio/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::SileroVadModelConfig/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::VadModelConfig/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::vad_segment_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::fixed_chunk_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::whole_audio_segment/u);
});

test('speaker processing runtime is owned by local ASR adapter and wrapped by desktop platform', () => {
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const localAsrLib = read('adapters', 'local_asr', 'src', 'lib.rs');
  const localAsrProcessing = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs'),
    'utf8',
  );
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformSpeaker = fs.readFileSync(
    desktopCratePath('src', 'platform', 'speaker_processing.rs'),
    'utf8',
  );
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');

  assert.match(localAsrLib, /^pub mod speaker;/mu);
  assert.match(localAsrLib, /^pub mod speaker_processing;/mu);
  assert.equal(exists('adapters', 'local_asr', 'src', 'speaker.rs'), true);
  assert.equal(exists('adapters', 'local_asr', 'src', 'speaker_processing.rs'), true);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'speaker.rs'), false);
  assert.match(desktopPlatform, /^pub mod speaker_processing;/mu);
  assert.doesNotMatch(desktopIntegrations, /^pub mod speaker;/mu);
  assert.doesNotMatch(tauriCargo, /^sherpa-onnx\s*=/mu);
  assert.match(localAsrProcessing, /crate::speaker::run_speaker_diarization/u);
  assert.match(localAsrProcessing, /crate::speaker::SpeakerEmbeddingIndex/u);
  assert.match(localAsrProcessing, /crate::audio::extract_and_resample_audio/u);
  assert.doesNotMatch(localAsrProcessing, /use sherpa_onnx::/u);
  assert.doesNotMatch(localAsrProcessing, /OfflineSpeakerDiarization/u);
  assert.doesNotMatch(localAsrProcessing, /SpeakerEmbeddingExtractor/u);
  assert.doesNotMatch(localAsrProcessing, /SpeakerEmbeddingManager/u);
  assert.match(platformSpeaker, /sona_local_asr::speaker_processing::annotate_speaker_segments_from_file/u);
  assert.match(platformSpeaker, /sona_local_asr::speaker_processing::import_speaker_profile_sample/u);
  assert.match(platformSpeaker, /pub async fn import_speaker_profile_sample_for_app/u);
  assert.match(systemCommand, /crate::platform::speaker_processing::annotate_speaker_segments_from_file/u);
  assert.match(systemCommand, /crate::platform::speaker_processing::import_speaker_profile_sample_for_app\(\s*&app,\s*profile_id,\s*source_path,\s*source_name\s*,?\s*\)\s*\.await/u);
  assert.match(batchRs, /sona_local_asr::speaker_processing::annotate_segments_with_speakers/u);
  assert.doesNotMatch(batchRs, /crate::integrations::speaker/u);
});

test('media file IO detection is delegated to media detector adapter', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const coreLib = read('core', 'src', 'lib.rs');
  const coreRuntime = read('core', 'src', 'runtime', 'mod.rs');
  const coreMediaDetector = read('core', 'src', 'runtime', 'media_detector.rs');
  const mediaDetectorCargo = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'media_detector', 'Cargo.toml'),
    'utf8',
  );
  const mediaDetectorLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'media_detector', 'src', 'lib.rs'),
    'utf8',
  );
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const apiCargo = read('adapters', 'api_server', 'Cargo.toml');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformMediaDetectorPath = desktopCratePath('src', 'platform', 'media_detector.rs');
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const tauriServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const apiServer = read('adapters', 'api_server', 'src', 'lib.rs');

  assert.match(coreCargo, /^infer\s*=/mu);
  assert.match(coreLib, /^pub mod runtime;/mu);
  assert.match(coreRuntime, /^pub mod media_detector;/mu);
  assert.match(coreMediaDetector, /pub fn is_valid_media_bytes/u);
  assert.doesNotMatch(coreMediaDetector, /tokio::fs::File/u);
  assert.doesNotMatch(coreMediaDetector, /pub async fn is_valid_media_file/u);
  assert.doesNotMatch(coreMediaDetector, /pub async fn check_media_formats/u);
  assert.match(mediaDetectorCargo, /^sona-core\s*=/mu);
  assert.match(mediaDetectorCargo, /^tokio\s*=/mu);
  assert.match(mediaDetectorLib, /pub async fn is_valid_media_file/u);
  assert.match(mediaDetectorLib, /pub async fn check_media_formats/u);
  assert.match(mediaDetectorLib, /sona_core::runtime::media_detector::is_valid_media_bytes/u);
  assert.equal(fs.existsSync(platformMediaDetectorPath), true);
  const platformMediaDetector = fs.readFileSync(platformMediaDetectorPath, 'utf8');
  assert.match(platformMod, /^pub mod media_detector;/mu);
  assert.match(platformMediaDetector, /pub async fn check_media_formats/u);
  assert.match(platformMediaDetector, /sona_media_detector::check_media_formats/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'media_detector.rs'), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod media_detector;/mu);
  assert.doesNotMatch(tauriCargo, /^infer\s*=/mu);
  assert.match(tauriCargo, /^sona-media-detector\s*=/mu);
  assert.doesNotMatch(cliCargo, /^sona-media-detector\s*=/mu);
  assert.match(apiCargo, /^sona-media-detector\s*=/mu);
  assert.match(systemCommand, /crate::platform::media_detector::check_media_formats\(paths\)\.await/u);
  assert.doesNotMatch(systemCommand, /sona_media_detector::check_media_formats/u);
  assert.match(apiServer, /sona_media_detector::is_valid_media_file/u);
  assert.doesNotMatch(systemCommand, /sona_core::runtime::media_detector::check_media_formats/u);
  assert.doesNotMatch(apiServer, /sona_core::runtime::media_detector::is_valid_media_file/u);
  assert.doesNotMatch(systemCommand, /crate::integrations::media_detector/u);
  assert.doesNotMatch(apiServer, /crate::integrations::media_detector/u);
  assert.doesNotMatch(tauriServer, /sona_media_detector::is_valid_media_file/u);
});

test('local streaming ASR VAD creation is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const localAsrRuntime = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'runtime.rs'),
    'utf8',
  );

  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'asr', 'model_config.rs'), false);
  assert.doesNotMatch(asrMod, /^mod model_config;/mu);
  assert.match(localAsrRuntime, /use crate::audio::\{[^}]*\bload_vad\b/u);
  assert.match(localAsrRuntime, /use crate::audio::\{[^}]*\bSafeVad\b/u);
  assert.match(localAsrRuntime, /pub fn configure_vad\(&mut self/u);
  assert.match(adapterSession, /session_instance\.configure_vad\(/u);
  assert.doesNotMatch(asrMod, /\bload_vad\b/u);
  assert.doesNotMatch(adapterSession, /use crate::audio::\{[\s\S]*SafeVad/u);
  assert.doesNotMatch(asrMod, /create_vad_detector/u);
  assert.doesNotMatch(asrMod, /pub struct SafeVad/u);
  assert.doesNotMatch(asrMod, /SileroVadModelConfig/u);
  assert.doesNotMatch(asrMod, /VadModelConfig/u);
  assert.doesNotMatch(asrMod, /VoiceActivityDetector/u);
});

test('desktop punctuation loading is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(asrMod, /pub use sona_local_asr::punctuation::\{Punctuation, load_punctuation\}/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuation/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuationConfig/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuationModelConfig/u);
});

test('desktop recognizer construction is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(asrMod, /pub use sona_local_asr::recognizer::/u);
  assert.doesNotMatch(asrMod, /^use sherpa_onnx::/mu);
  assert.doesNotMatch(asrMod, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(asrMod, /OnlineRecognizerConfig/u);
  assert.doesNotMatch(asrMod, /pub enum ModelType/u);
  assert.doesNotMatch(asrMod, /impl Recognizer/u);
});

test('desktop ASR modules use a local runtime facade instead of sherpa implementation paths', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const adapterRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'adapter.rs'),
    'utf8',
  );
  const stateRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'state.rs');
  const transcriptRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const traitsRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'traits.rs');
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.doesNotMatch(asrMod, /^mod sherpa_onnx;$/mu);
  assert.match(asrMod, /^mod state;$/mu);
  assert.match(asrMod, /^mod traits;$/mu);
  assert.doesNotMatch(asrMod, /^pub mod state;$/mu);
  assert.doesNotMatch(asrMod, /^pub mod traits;$/mu);
  assert.match(adapterRs, /sona_local_asr::streaming::create_streaming_session/u);
  assert.doesNotMatch(adapterRs, /super::state::AsrState/u);
  assert.doesNotMatch(adapterRs, /super::traits::/u);
  assert.doesNotMatch(stateRs, /resolve_punctuation/u);
  assert.doesNotMatch(stateRs, /super::traits::/u);
  assert.match(transcriptRs, /pub\(crate\) fn diagnostics_instance_label/u);
  assert.match(transcriptRs, /pub\(crate\) fn log_segment_emit_diagnostics/u);
  assert.match(traitsRs, /use super::\{[\s\S]*AsrTranscriptionRequest[\s\S]*\};/u);
  assert.doesNotMatch(stateRs, /crate::integrations::asr::state::/u);
  assert.doesNotMatch(transcriptRs, /crate::integrations::asr::traits::/u);
  assert.doesNotMatch(traitsRs, /crate::integrations::asr::types::/u);
  assert.doesNotMatch(volcengineRs, /crate::integrations::asr::types::/u);
  assert.doesNotMatch(volcengineRs, /super::super::types::/u);
  assert.doesNotMatch(adapterRs, /sherpa_onnx::/u);
  assert.doesNotMatch(stateRs, /sherpa_onnx::/u);
  assert.doesNotMatch(transcriptRs, /sherpa_onnx::/u);
});

test('local batch transcription reuses local ASR recognizer model construction', () => {
  const batchRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'batch.rs'),
    'utf8',
  );
  const recognizerRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'),
    'utf8',
  );

  assert.match(batchRs, /pub struct LocalBatchAsrAdapter/u);
  assert.match(batchRs, /impl BatchTranscriber for LocalBatchAsrAdapter/u);
  assert.match(batchRs, /use crate::recognizer::\{[^}]*build_offline_model_config/u);
  assert.match(batchRs, /decode_offline_samples/u);
  assert.match(batchRs, /create_offline_recognizer/u);
  assert.match(recognizerRs, /pub fn create_offline_recognizer\([\s\S]*\) -> Result<SafeOfflineRecognizer, String>/u);
  assert.doesNotMatch(batchRs, /pub async fn run_offline_transcription/u);
  assert.doesNotMatch(batchRs, /enum ModelType/u);
  assert.doesNotMatch(batchRs, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(recognizerRs, /\) -> Result<OfflineRecognizer, String>/u);
  assert.doesNotMatch(batchRs, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(batchRs, /fn build_model_config/u);
  assert.doesNotMatch(batchRs, /fn create_recognizer/u);
});

test('local streaming ASR offline decode is delegated to recognizer helpers', () => {
  const adapterInference = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'inference.rs'),
    'utf8',
  );

  assert.match(adapterInference, /decode_offline_samples/u);
  assert.doesNotMatch(adapterInference, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(adapterInference, /let stream = r\.create_stream\(\)/u);
});

test('desktop batch offline decode is delegated to local ASR adapter', () => {
  const batchRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );

  assert.match(batchRs, /decode_offline_samples/u);
  assert.doesNotMatch(batchRs, /FFI: Calling accept_waveform \(Offline segment\)/u);
  assert.doesNotMatch(batchRs, /let stream = r\.0\.create_stream\(\)/u);
});

test('desktop streaming offline decode is delegated to local ASR adapter', () => {
  const localAsrAudio = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'audio.rs'),
    'utf8',
  );
  const streamingRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'streaming.rs'),
    'utf8',
  );

  assert.match(localAsrAudio, /pub fn pcm_s16le_bytes_to_f32/u);
  assert.match(streamingRs, /pcm_s16le_bytes_to_f32\(&pcm\)/u);
  assert.match(streamingRs, /decode_offline_samples/u);
  assert.doesNotMatch(streamingRs, /chunks_exact\(2\).*i16::from_le_bytes/u);
  assert.doesNotMatch(streamingRs, /let stream = r\.0\.create_stream\(\)/u);
  assert.doesNotMatch(streamingRs, /stream\.accept_waveform\(16000, &full_audio\)/u);
  assert.doesNotMatch(streamingRs, /r\.0\.decode\(&stream\)/u);
});

test('local streaming ASR online operations use recognizer helpers', () => {
  const batchRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const localOnlineRs = `${batchRs}\n${adapterSession}`;

  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*create_online_stream/u);
  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*accept_online_samples/u);
  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*decode_online_ready/u);
  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*online_stream_result/u);
  assert.doesNotMatch(localOnlineRs, /SafeStream\(r\.0\.create_stream\(\)\)/u);
  assert.doesNotMatch(localOnlineRs, /\.0\.accept_waveform\(16000/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.is_ready\(&[^)]*\.0\)/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.decode\(&[^)]*\.0\)/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.get_result\(&[^)]*\.0\)/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.reset\(&[^)]*\.0\)/u);
});

test('local streaming ASR VAD runtime operations use private wrappers', () => {
  const audioRs = read('adapters', 'local_asr', 'src', 'audio.rs');
  const recognizerRs = read('adapters', 'local_asr', 'src', 'recognizer.rs');
  const runtimeRs = read('adapters', 'local_asr', 'src', 'runtime.rs');
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const streamingRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'streaming.rs'),
    'utf8',
  );
  const vadConsumers = `${adapterSession}\n${streamingRs}`;

  assert.match(asrMod, /accept_vad_samples/u);
  assert.match(runtimeRs, /pub fn reset_or_reload_vad\(&mut self\)/u);
  assert.match(adapterSession, /instance\.reset_or_reload_vad\(\)/u);
  assert.doesNotMatch(
    adapterSession,
    /^use crate::audio::\{[^}]*(?:load_vad|reset_vad)\b/mu,
  );
  assert.match(asrMod, /vad_detected/u);
  assert.doesNotMatch(vadConsumers, /SafeVad\([^)]+\)/u);
  assert.doesNotMatch(vadConsumers, /\.0\.accept_waveform/u);
  assert.doesNotMatch(vadConsumers, /\.0\.detected/u);
  assert.doesNotMatch(audioRs, /pub struct SafeVad\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeOnlineRecognizer\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeOfflineRecognizer\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeStream\(pub /u);
});

test('local ASR VAD sherpa primitives are crate-private', () => {
  const audioRs = read('adapters', 'local_asr', 'src', 'audio.rs');

  assert.doesNotMatch(audioRs, /^pub type VadConfig\s*=/mu);
  assert.doesNotMatch(audioRs, /^pub type VadDetector\s*=/mu);
  assert.doesNotMatch(audioRs, /^pub fn create_vad_config/mu);
  assert.doesNotMatch(audioRs, /^pub fn create_vad_detector/mu);
  assert.doesNotMatch(audioRs, /^pub fn vad_segment_audio_with_capacity/mu);
  assert.match(audioRs, /^pub\(crate\) type VadConfig\s*=/mu);
  assert.match(audioRs, /^pub\(crate\) fn create_vad_config/mu);
});
