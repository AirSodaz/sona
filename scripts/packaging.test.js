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
          externalBin: ['binaries/ffmpeg', 'binaries/sona-cli'],
        },
      },
      null,
      2,
    ),
  );
}

test('prepare-cli-sidecar copies a target-suffixed standalone CLI binary for Tauri', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  const sourceDir = path.join(root, 'target', target, 'release');
  fs.mkdirSync(sourceDir, { recursive: true });
  fs.writeFileSync(path.join(sourceDir, 'sona-cli.exe'), 'fake cli binary');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'prepare-cli-sidecar.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--skip-build',
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.equal(
    fs.readFileSync(
      path.join(root, 'src-tauri', 'binaries', 'sona-cli-x86_64-pc-windows-msvc.exe'),
      'utf8',
    ),
    'fake cli binary',
  );
});

test('verify-cli-bundle requires the installer, CLI sidecar, and shared sherpa libraries', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  const bundleRoot = path.join(root, 'target', target, 'release', 'bundle', 'nsis');
  fs.mkdirSync(bundleRoot, { recursive: true });
  fs.writeFileSync(path.join(bundleRoot, 'Sona_0.8.0_x64-setup.exe'), 'installer');
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'binaries', 'sona-cli-x86_64-pc-windows-msvc.exe'),
    'cli',
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

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'verify-cli-bundle.js'),
      '--repo-root',
      root,
      '--bundle-root',
      bundleRoot,
      '--target',
      target,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified packaged CLI sidecar/);
  assert.match(result.stdout, /Verified shared libraries/);
});

test('desktop tauri crate relies on the standalone sona-cli sidecar instead of an embedded cli module', () => {
  const libRs = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const cargoToml = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.doesNotMatch(libRs, /\bmod cli;/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src-tauri', 'src', 'cli')), false);
  assert.doesNotMatch(cargoToml, /^clap\s*=/mu);
  assert.doesNotMatch(cargoToml, /^clap_complete\s*=/mu);
  assert.doesNotMatch(prWorkflow, /cli::transcribe::tests/u);
});
