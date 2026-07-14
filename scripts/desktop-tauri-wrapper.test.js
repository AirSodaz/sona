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

  const run = (command, commandArgs = []) => {
    fs.rmSync(logPath, { force: true });
    const result = spawnSync(
      node,
      [path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'tauri.js'), command, ...commandArgs],
      {
        cwd: repoRoot,
        encoding: 'utf8',
        env: {
          ...process.env,
          SONA_TAURI_BINARY: tauriBinary,
          SONA_DESKTOP_BUNDLE_PREPARER: preparerPath,
          SONA_TAURI_ARGS_LOG: logPath,
          SHERPA_ONNX_LIB_DIR: path.join(root, 'source-runtime-libs'),
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
});
