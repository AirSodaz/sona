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

test('sona-sync-webdav implements the provider factory without depending on the local database', () => {
  const manifest = read('adapters', 'sync_webdav', 'Cargo.toml');
  assert.match(manifest, /^sona-core\s*=/mu);
  assert.match(manifest, /^sona-sync\s*=/mu);
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

test('desktop and UniFFI delegate the complete sync lifecycle to SyncApplication', () => {
  const hosts = [
    read('platforms', 'desktop', 'src', 'platform', 'sync.rs'),
    read('adapters', 'uniffi_bind', 'src', 'sync_bridge.rs'),
  ];

  for (const source of hosts) {
    assert.match(source, /\bSyncApplication\b/u);
    for (const forbidden of [
      'create_remote_vault',
      'open_remote_vault_with_password',
      'open_remote_vault_with_recovery_key',
      'open_remote_vault_with_vault_key',
      'run_sync_cycle',
      'load_remote_state_for_join',
      'change_sync_preset',
    ]) {
      assert.doesNotMatch(source, new RegExp(`\\b${forbidden}\\b`, 'u'));
    }
    assert.doesNotMatch(source, /\bstruct\s+(?:UnlockedSession|Session|PersistedSyncConfig|PersistedConfig)\b/u);
  }
});
