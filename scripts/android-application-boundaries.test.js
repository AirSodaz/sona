import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const applicationRoot = path.join(repoRoot, 'platforms', 'android', 'client', 'application');

function filesUnder(root, extension) {
  const files = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const child = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(child);
      } else if (entry.isFile() && entry.name.endsWith(extension)) {
        files.push(child);
      }
    }
  };
  visit(root);
  return files.sort();
}

test('Android application layer is a platform-independent JVM module', () => {
  const buildScript = fs.readFileSync(path.join(applicationRoot, 'build.gradle.kts'), 'utf8');

  assert.match(buildScript, /id\("org\.jetbrains\.kotlin\.jvm"\)/u);
  assert.doesNotMatch(buildScript, /id\("com\.android\.(?:application|library)"\)/u);
  assert.equal(fs.existsSync(path.join(applicationRoot, 'src', 'main', 'AndroidManifest.xml')), false);
});

test('all Android application Kotlin sources stay free of platform imports', () => {
  const sourceRoot = path.join(applicationRoot, 'src');
  const sources = filesUnder(sourceRoot, '.kt');

  assert.ok(sources.length > 0, 'application/src must contain Kotlin sources');
  for (const sourcePath of sources) {
    const source = fs.readFileSync(sourcePath, 'utf8');
    const relativePath = path.relative(repoRoot, sourcePath).split(path.sep).join('/');
    assert.doesNotMatch(
      source,
      /^import (?:android|androidx|uniffi)\./mu,
      `${relativePath} must not import Android, AndroidX, or UniFFI APIs`,
    );
  }
});
