import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  LIFECYCLE_ONLY,
  currentSonaContextOperations,
  renderSonaContextOperations,
} from './generate-sona-context.js';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/**
 * Reduces an operations block to `name -> delegation call`, discarding layout.
 *
 * The checked-in file is rustfmt-shaped and the generator's output is not, so
 * comparing text would report formatting as drift. What must match is which
 * operations exist and what each one delegates to.
 */
function delegations(block) {
  const map = new Map();
  for (const match of block.matchAll(
    /pub\s+(?:async\s+)?fn\s+(\w+)\(\s*&self[\s\S]*?\)\s*->[\s\S]*?\{\s*([\s\S]*?)\n    \}/gu,
  )) {
    const [, name, body] = match;
    // rustfmt adds a trailing comma when it wraps a call across lines.
    map.set(name, body.replace(/\s+/gu, '').replace(/,\)/gu, ')'));
  }
  return map;
}

test('the checked-in SonaContext operations match the generator', () => {
  const generated = renderSonaContextOperations();
  const current = currentSonaContextOperations();
  assert.ok(current, 'sona_context.rs must contain a generated operations block');

  const expected = delegations(generated.block);
  const actual = delegations(current);

  assert.ok(expected.size > 100, `expected the full operation set, got ${expected.size}`);
  assert.deepEqual(
    [...actual.keys()].sort(),
    [...expected.keys()].sort(),
    'run `pnpm run generate:sona-context` after changing a directory-scoped export',
  );
  for (const [name, body] of expected) {
    assert.equal(
      actual.get(name),
      body,
      `${name} delegates differently than the generator produces; regenerate`,
    );
  }
});

test('SonaContext generation is reachable through an npm script', () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  );
  assert.equal(
    packageJson.scripts['generate:sona-context'],
    'node scripts/generate-sona-context.js && cargo fmt -p sona-uniffi-bind',
    'the generator must stay runnable the same way as the other generated artifacts',
  );
});

test('operations the handle omits stay explicitly justified', () => {
  assert.ok(LIFECYCLE_ONLY.size > 0, 'omissions must be recorded, not implicit');
  for (const [name, reason] of LIFECYCLE_ONLY) {
    assert.match(
      reason,
      /\w+\s+\w+/u,
      `${name} must record why the handle does not offer it`,
    );
  }
});
