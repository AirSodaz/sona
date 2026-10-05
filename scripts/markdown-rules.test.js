import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { validateMarkdown } from './markdown-rules.js';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('validateMarkdown catches unclosed fenced code blocks', () => {
  const bad = '# Title\n\n```bash\necho "hello"\n';
  const errors = validateMarkdown(bad, 'test.md');
  assert.equal(errors.length, 1);
  assert.match(errors[0], /unclosed fenced code block/u);

  const good = '# Title\n\n```bash\necho "hello"\n```\n';
  assert.deepEqual(validateMarkdown(good, 'test.md'), []);
});

test('validateMarkdown catches code blocks missing language identifier', () => {
  const bad = '# Title\n\n```\necho "hello"\n```\n';
  const errors = validateMarkdown(bad, 'test.md');
  assert.equal(errors.length, 1);
  assert.match(errors[0], /missing a language identifier/u);

  const good = '# Title\n\n```text\necho "hello"\n```\n';
  assert.deepEqual(validateMarkdown(good, 'test.md'), []);
});

test('validateMarkdown catches headings missing space after #', () => {
  const bad = '#Title\n\nSome text.\n';
  const errors = validateMarkdown(bad, 'test.md');
  assert.equal(errors.length, 1);
  assert.match(errors[0], /missing a space after '#'/u);

  const good = '# Title\n\n## Subtitle\n';
  assert.deepEqual(validateMarkdown(good, 'test.md'), []);
});

test('validateMarkdown catches empty link targets', () => {
  const bad = 'See [link]().\n';
  const errors = validateMarkdown(bad, 'test.md');
  assert.equal(errors.length, 1);
  assert.match(errors[0], /empty Markdown link target/u);
});

test('validateMarkdown catches stale CLI commands in docs/cli.md', () => {
  const bad1 = 'Run `sona-cli export transcript` now.\n';
  const errors1 = validateMarkdown(bad1, 'docs/cli.md');
  assert.equal(errors1.length, 1);
  assert.match(errors1[0], /stale command 'export transcript'/u);

  const good = 'Run `sona-cli export ./segments.json` now.\n';
  assert.deepEqual(validateMarkdown(good, 'docs/cli.md'), []);
});

test('docs/cli.md passes all Markdown quality rules', () => {
  const cliDocPath = path.join(repoRoot, 'docs', 'cli.md');
  assert.ok(fs.existsSync(cliDocPath), 'docs/cli.md must exist');
  const content = fs.readFileSync(cliDocPath, 'utf8');
  const errors = validateMarkdown(content, 'docs/cli.md');
  assert.deepEqual(errors, [], `docs/cli.md had validation errors:\n${errors.join('\n')}`);
});

test('docs/cli.md passes quality rules and contains generated commands with exact heading levels', () => {
  const cliPath = path.join(repoRoot, 'docs', 'cli.md');
  assert.ok(fs.existsSync(cliPath), 'docs/cli.md must exist');
  const content = fs.readFileSync(cliPath, 'utf8');
  const errors = validateMarkdown(content, 'docs/cli.md');
  assert.deepEqual(errors, [], `docs/cli.md had validation errors:\n${errors.join('\n')}`);
  assert.match(content, /^# Sona CLI Guide & Reference$/mu);
  assert.match(content, /^## Stateless Boundary$/mu);
  assert.match(content, /^## Configuration Precedence$/mu);
  assert.match(content, /^## Public Command Reference$/mu);
  assert.match(content, /^### `sona-cli`$/mu);
  assert.match(content, /^#### `sona-cli live`$/mu);
  assert.match(content, /^#### `sona-cli transcribe`$/mu);
  assert.match(content, /^#### `sona-cli export`$/mu);
  assert.match(content, /^## Internal Integration Contracts \(Hidden Commands\)$/mu);
  assert.match(content, /^### `sona-cli diagnostics`$/mu);
  assert.match(content, /^#### `sona-cli diagnostics snapshot`$/mu);
  assert.match(content, /^### `sona-cli path-status`$/mu);
});
