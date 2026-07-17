import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8');
}

const coreBackup = read('core', 'src', 'backup', 'model.rs');
const coreSync = read('core', 'src', 'sync', 'model.rs');
const desktopBindings = read(
  'platforms',
  'desktop',
  'frontend',
  'src',
  'bindings.ts',
);
const frontendBackup = read(
  'platforms',
  'desktop',
  'frontend',
  'src',
  'types',
  'backup.ts',
);
const syncConflictCenter = read(
  'platforms',
  'desktop',
  'frontend',
  'src',
  'components',
  'settings',
  'sync',
  'SyncConflictCenter.tsx',
);
const prGuardrails = read('.github', 'workflows', 'pr-guardrails.yml');

test('desktop backup contract follows Rust schema v2 and tags', () => {
  assert.match(coreBackup, /BACKUP_SCHEMA_VERSION:\s*u64\s*=\s*2/u);
  assert.match(frontendBackup, /BACKUP_SCHEMA_VERSION\s*=\s*2\s+as const/u);
  assert.match(
    desktopBindings,
    /export type BackupManifestCounts_Serialize = \{[\s\S]*?\btags: number,/u,
  );
  assert.doesNotMatch(
    desktopBindings,
    /export type BackupManifestCounts_Serialize = \{[\s\S]*?\bprojects: number,/u,
  );
});

test('generated desktop sync entity kinds include the canonical tag variant', () => {
  assert.match(coreSync, /enum SyncEntityKind\s*\{[\s\S]*?\bTag,/u);
  assert.match(
    desktopBindings,
    /export type SyncEntityKind = [^;]*"tag"/u,
  );
});

test('sync conflict UI reads the persisted HLC snake_case timestamp', () => {
  assert.match(syncConflictCenter, /version\.clock\.physical_ms\b/u);
  assert.doesNotMatch(syncConflictCenter, /version\.clock\.physicalMs\b/u);
});

test('PR guardrails reference the renamed WebDAV sync adapter crate', () => {
  assert.match(prGuardrails, /-p sona-sync-webdav\b/u);
  assert.doesNotMatch(prGuardrails, /-p sona-webdav\b/u);
});

test('desktop AppConfig is constrained by the generated Rust contract', () => {
  const configTypes = read(
    'platforms',
    'desktop',
    'frontend',
    'src',
    'types',
    'config.ts',
  );

  assert.match(
    desktopBindings,
    /export type AppConfig = AppConfig_Serialize \| AppConfig_Deserialize/u,
  );
  assert.match(configTypes, /AppConfig as GeneratedAppConfig/u);
  assert.match(configTypes, /GeneratedAppConfig\s*&/u);
  assert.doesNotMatch(configTypes, /Record<string, any>/u);
});
