import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const node = process.execPath;

function makeTempRepo() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-packaging-'));
  fs.mkdirSync(path.join(root, 'src-tauri', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'src-tauri', 'resources', 'shared_libs'), { recursive: true });
  return root;
}

function writeTauriConfig(root) {
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'tauri.conf.json'),
    JSON.stringify(
      {
        bundle: {
          resources: ['resources/shared_libs/*'],
          externalBin: ['binaries/ffmpeg'],
        },
      },
      null,
      2,
    ),
  );
}

function rustFilesUnder(root) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      return rustFilesUnder(fullPath);
    }
    return entry.isFile() && entry.name.endsWith('.rs') ? [fullPath] : [];
  });
}

test('tauri bundle verification requires ffmpeg sidecar and shared libraries', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  fs.mkdirSync(path.join(root, 'src-tauri', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'src-tauri', 'resources', 'shared_libs'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'binaries', 'ffmpeg-x86_64-pc-windows-msvc.exe'),
    'ffmpeg',
  );
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'resources', 'shared_libs', 'sherpa-onnx-c-api.dll'),
    'sherpa',
  );
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'resources', 'shared_libs', 'onnxruntime.dll'),
    'onnxruntime',
  );
  writeTauriConfig(root);
  const bundleRoot = path.join(root, 'target', target, 'release', 'bundle', 'nsis');
  fs.mkdirSync(bundleRoot, { recursive: true });
  fs.writeFileSync(path.join(bundleRoot, 'Sona_0.8.0_x64-setup.exe'), 'installer');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--bundle-root',
      bundleRoot,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified packaged ffmpeg sidecar/);
  assert.match(result.stdout, /Verified shared libraries/);
});

test('desktop tauri crate no longer bundles sona-cli sidecar artifacts', () => {
  const libRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const cargoToml = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const tauriConfig = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'tauri.conf.json'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const tauriScript = fs.readFileSync(path.join(repoRoot, 'scripts', 'tauri.js'), 'utf8');
  const verifyScript = fs.readFileSync(path.join(repoRoot, 'scripts', 'verify-tauri-bundle.js'), 'utf8');
  const oldCliSidecarScript = ['prepare', 'cli', 'sidecar'].join('-');
  const oldCliBundleScript = ['verify', 'cli', 'bundle'].join('-');

  assert.doesNotMatch(libRs, /\bmod cli;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'cli')), false);
  assert.doesNotMatch(cargoToml, /^clap\s*=/mu);
  assert.doesNotMatch(cargoToml, /^clap_complete\s*=/mu);
  assert.doesNotMatch(tauriConfig, /binaries\/sona-cli/u);
  assert.doesNotMatch(tauriScript, new RegExp(oldCliSidecarScript, 'u'));
  assert.doesNotMatch(verifyScript, /sona-cli/u);
  assert.doesNotMatch(prWorkflow, new RegExp(`${oldCliSidecarScript}|${oldCliBundleScript}`, 'u'));

  const desktopCliCoreReferences = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /sona_core::cli_|OfflineTranscribeCliOptions/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopCliCoreReferences, []);
});

test('standalone CLI package includes sona-cli binary and shared libraries', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-cli-package-'));
  const target = 'x86_64-pc-windows-msvc';
  const releaseDir = path.join(root, 'target', target, 'release');
  const libDir = path.join(root, 'sherpa-onnx-libs');
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.mkdirSync(libDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, 'sona-cli.exe'), 'cli');
  fs.writeFileSync(path.join(libDir, 'sherpa-onnx-c-api.dll'), 'sherpa');
  fs.writeFileSync(path.join(libDir, 'onnxruntime.dll'), 'onnxruntime');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'package-sona-cli.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--lib-dir',
      libDir,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Packaged standalone CLI/);

  const packageDir = path.join(releaseDir, 'sona-cli-package', `sona-cli-${target}`);
  assert.equal(fs.existsSync(path.join(packageDir, 'sona-cli.exe')), true);
  assert.equal(fs.existsSync(path.join(packageDir, 'sherpa-onnx-c-api.dll')), true);
  assert.equal(fs.existsSync(path.join(packageDir, 'onnxruntime.dll')), true);
  assert.equal(fs.existsSync(path.join(releaseDir, `sona-cli-${target}.tar.gz`)), true);
});

test('release workflows package standalone CLI artifacts with shared libraries', () => {
  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const workflow = fs.readFileSync(path.join(repoRoot, '.github', 'workflows', workflowName), 'utf8');

    assert.match(workflow, /cargo build -p sona-cli --release \$\{\{ matrix\.args \}\}/u);
    assert.match(workflow, /node scripts\/package-sona-cli\.js \$\{\{ matrix\.args \}\}/u);
    assert.match(workflow, /matrix\.args != '--target universal-apple-darwin'/u);
    assert.match(workflow, /target\/\*\*\/release\/sona-cli-\*\.tar\.gz/u);
  }
});

test('core crate does not keep sona-cli config template surface', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');

  assert.doesNotMatch(coreLib, /^pub mod cli_config;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'src', 'cli_config.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'tests', 'cli_config.rs')), false);
});

test('core crate exposes serve runtime without cli runtime surface', () => {
  const coreLib = fs.readFileSync(path.join(repoRoot, 'core', 'src', 'lib.rs'), 'utf8');

  assert.match(coreLib, /^pub mod serve_runtime;/mu);
  assert.doesNotMatch(coreLib, /^pub mod cli_runtime;/mu);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'src', 'cli_runtime.rs')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'core', 'tests', 'cli_runtime.rs')), false);
});

test('desktop api server reuses local ASR adapter for standalone offline transcription', () => {
  const tauriCargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const apiServer = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'server.rs'), 'utf8');

  assert.match(tauriCargo, /^sona-local-asr\s*=\s*\{ path = "\.\.\/adapters\/local_asr" \}/mu);
  assert.match(apiServer, /sona_local_asr::offline::run_offline_transcription/u);
  assert.doesNotMatch(apiServer, /use crate::integrations::asr::transcribe_batch_with_progress;/u);
  assert.doesNotMatch(apiServer, /LocalSherpaAdapter::offline_plan_to_batch_request/u);
});

test('pr guardrails run local ASR adapter tests with core bindings and standalone CLI', () => {
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.match(
    prWorkflow,
    /cargo test -p sona-core -p sona-local-asr -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
  );
});

test('desktop hardware module reuses local ASR adapter GPU planning', () => {
  const hardwareRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'hardware.rs'), 'utf8');

  assert.match(hardwareRs, /pub\(crate\) use sona_local_asr::gpu::\{/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::check_gpu_availability\(\)\.await/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::resolve_gpu_acceleration_plan/u);
  assert.doesNotMatch(hardwareRs, /tokio::process::Command/u);
  assert.doesNotMatch(hardwareRs, /struct GpuAccelerationPlan/u);
  assert.doesNotMatch(hardwareRs, /struct GpuFallbackNotice/u);
});

test('desktop local audio helpers come from local ASR adapter without Tauri core pipeline', () => {
  const coreMod = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'mod.rs'), 'utf8');
  const batchRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'), 'utf8');
  const speakerRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'speaker.rs'), 'utf8');
  const runtimeStatusRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'app', 'runtime_status.rs'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'core', 'pipeline.rs')), false);
  assert.doesNotMatch(coreMod, /^pub mod pipeline;/mu);
  assert.match(batchRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(batchRs, /sona_local_asr::audio::save_wav_file/u);
  assert.match(speakerRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(speakerRs, /sona_local_asr::audio::save_wav_file/u);
  assert.match(runtimeStatusRs, /sona_local_asr::audio::resolve_ffmpeg_sidecar_path/u);
  assert.doesNotMatch(prWorkflow, /core::pipeline::tests/u);

  const desktopPipelineReferences = rustFilesUnder(path.join(repoRoot, 'src-tauri', 'src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /crate::core::pipeline|core::pipeline/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopPipelineReferences, []);
});

test('desktop batch ASR delegates audio segmentation to local ASR adapter', () => {
  const batchRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'batch.rs'), 'utf8');

  assert.match(batchRs, /sona_local_asr::audio::segment_batch_audio/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::SileroVadModelConfig/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::VadModelConfig/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::vad_segment_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::fixed_chunk_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::whole_audio_segment/u);
});

test('desktop live VAD creation is delegated to local ASR adapter', () => {
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );

  assert.match(modelConfigRs, /sona_local_asr::audio::create_vad_detector/u);
  assert.match(modelConfigRs, /SafeVad\(pub sona_local_asr::audio::VadDetector\)/u);
  assert.doesNotMatch(modelConfigRs, /SileroVadModelConfig/u);
  assert.doesNotMatch(modelConfigRs, /VadModelConfig/u);
  assert.doesNotMatch(modelConfigRs, /VoiceActivityDetector/u);
});

test('desktop punctuation loading is delegated to local ASR adapter', () => {
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );

  assert.match(modelConfigRs, /pub use sona_local_asr::punctuation::\{Punctuation, load_punctuation\}/u);
  assert.doesNotMatch(modelConfigRs, /OfflinePunctuation/u);
  assert.doesNotMatch(modelConfigRs, /OfflinePunctuationConfig/u);
  assert.doesNotMatch(modelConfigRs, /OfflinePunctuationModelConfig/u);
});

test('desktop recognizer construction is delegated to local ASR adapter', () => {
  const modelConfigRs = fs.readFileSync(
    path.join(repoRoot, 'src-tauri', 'src', 'integrations', 'asr', 'model_config.rs'),
    'utf8',
  );

  assert.match(modelConfigRs, /pub use sona_local_asr::recognizer::/u);
  assert.doesNotMatch(modelConfigRs, /use sherpa_onnx::/u);
  assert.doesNotMatch(modelConfigRs, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(modelConfigRs, /OnlineRecognizerConfig/u);
  assert.doesNotMatch(modelConfigRs, /pub enum ModelType/u);
  assert.doesNotMatch(modelConfigRs, /impl Recognizer/u);
});
