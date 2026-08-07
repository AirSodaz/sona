import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { repoRoot } from './test-support/repo-root.js';
import {
  node,
  writeTauriWrapperStubs,
} from './test-support/desktop-packaging-fixtures.js';

test('tauri wrapper passes generated config to build and bundle while dev preserves its base config', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-tauri-wrapper-'));
  const target = 'test-wrapper-target';
  const customDevConfig = path.join(root, 'dev-tauri.conf.json');
  const { logPath, preparerPath, tauriBinary } = writeTauriWrapperStubs(root);
  fs.writeFileSync(customDevConfig, '{}');

  const run = (command, commandArgs = [], environment = {}) => {
    fs.rmSync(logPath, { force: true });
    const { MACOSX_DEPLOYMENT_TARGET: _ignored, ...baseEnvironment } = process.env;
    const result = spawnSync(
      node,
      [path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'tauri.js'), command, ...commandArgs],
      {
        cwd: repoRoot,
        encoding: 'utf8',
        env: {
          ...baseEnvironment,
          SONA_TAURI_BINARY: tauriBinary,
          SONA_DESKTOP_BUNDLE_PREPARER: preparerPath,
          SONA_TAURI_ARGS_LOG: logPath,
          SHERPA_ONNX_LIB_DIR: path.join(root, 'source-runtime-libs'),
          ...environment,
        },
      },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    return JSON.parse(fs.readFileSync(logPath, 'utf8'));
  };

  const generatedConfig = path.join(
    repoRoot,
    'target',
    'desktop-bundle',
    target,
    'tauri.bundle.conf.json',
  );
  const macTarget = 'aarch64-apple-darwin';
  t.after(() => fs.rmSync(path.join(repoRoot, 'target', 'desktop-bundle', target), { recursive: true, force: true }));
  t.after(() => fs.rmSync(path.join(repoRoot, 'target', 'desktop-bundle', macTarget), { recursive: true, force: true }));
  for (const command of ['build', 'bundle']) {
    const invocation = run(command, ['--target', target]);
    assert.deepEqual(invocation.args.slice(0, 3), [command, '--config', generatedConfig]);
  }
  const devInvocation = run('dev', ['--config', customDevConfig, '--help']);
  assert.deepEqual(devInvocation.args.slice(0, 3), ['dev', '--config', customDevConfig]);
  const macInvocation = run('build', ['--target', macTarget]);
  assert.equal(
    macInvocation.sherpaLibDir,
    path.join(repoRoot, 'target', 'desktop-bundle', macTarget, 'runtime-libs'),
  );
  assert.equal(macInvocation.macosDeploymentTarget, '10.15');
  assert.equal(macInvocation.preparedMacosDeploymentTarget, '10.15');
  assert.equal(macInvocation.cmakeDisableFindPackageOpenSsl, 'TRUE');
  assert.equal(macInvocation.preparedCmakeDisableFindPackageOpenSsl, 'TRUE');
  const legacyMacInvocation = run(
    'build',
    ['--target', macTarget],
    { MACOSX_DEPLOYMENT_TARGET: '10.13' },
  );
  assert.equal(legacyMacInvocation.macosDeploymentTarget, '10.15');
  assert.equal(legacyMacInvocation.preparedMacosDeploymentTarget, '10.15');
  const customMacInvocation = run(
    'build',
    ['--target', macTarget],
    { MACOSX_DEPLOYMENT_TARGET: '12.0' },
  );
  assert.equal(customMacInvocation.macosDeploymentTarget, '12.0');
  assert.equal(customMacInvocation.preparedMacosDeploymentTarget, '12.0');
});

test('release workflows enable C++ exceptions for Windows ARM64 clang-cl builds', () => {
  for (const workflow of ['release.yml', 'nightly.yml']) {
    const source = fs.readFileSync(path.join(repoRoot, '.github', 'workflows', workflow), 'utf8');
    assert.match(source, /CXXFLAGS_aarch64_pc_windows_msvc=\/EHsc/u);
    assert.match(
      source,
      /LLAMA_BUILD_SHARED_LIBS:\s*\$\{\{\s*startsWith\(matrix\.platform,\s*'ubuntu'\)\s*&&\s*'0'\s*\|\|\s*'1'\s*\}\}/u,
    );
  }

  const config = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'platforms', 'desktop', 'tauri.conf.json'), 'utf8'),
  );
  assert.equal(config.bundle.macOS.minimumSystemVersion, '10.15');
});
