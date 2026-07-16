import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8');
}

function rustSources(...parts) {
  const root = path.join(repoRoot, ...parts);
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      return rustSources(...parts, entry.name);
    }
    return entry.name.endsWith('.rs') ? [fs.readFileSync(entryPath, 'utf8')] : [];
  }).join('\n');
}

test('sona-sync remains provider neutral', () => {
  const manifest = read('adapters', 'sync', 'Cargo.toml');

  for (const dependency of ['reqwest', 'roxmltree', 'url', 'sona-sync-webdav']) {
    assert.doesNotMatch(
      manifest,
      new RegExp(`^${dependency}\\s*=`, 'mu'),
      `${dependency} must not enter the sona-sync dependency graph`,
    );
  }
});

test('sona-sync-webdav depends on ports but not the sync runtime or local database', () => {
  const manifest = read('adapters', 'sync_webdav', 'Cargo.toml');
  assert.match(manifest, /^sona-core\s*=/mu);
  assert.doesNotMatch(manifest, /^sona-sync\s*=/mu);
  assert.doesNotMatch(manifest, /^sona-sqlite\s*=/mu);

  const source = [
    rustSources('adapters', 'sync_webdav', 'src'),
    rustSources('adapters', 'sync_webdav', 'tests'),
  ].join('\n');
  for (const runtimeType of ['SyncOperation', 'SyncConflict', 'SyncCheckpointV1', 'SyncSegmentV1']) {
    assert.doesNotMatch(source, new RegExp(`\\b${runtimeType}\\b`, 'u'));
  }
});

test('the workspace uses the renamed sync WebDAV adapter only', () => {
  const workspace = read('Cargo.toml');
  assert.match(workspace, /"adapters\/sync_webdav"/u);
  assert.doesNotMatch(workspace, /"adapters\/webdav"/u);
  assert.equal(fs.existsSync(path.join(repoRoot, 'adapters', 'webdav')), false);
});
