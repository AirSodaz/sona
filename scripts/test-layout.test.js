import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const countLines = (source) => source === ''
  ? 0
  : source.split(/\r\n|\r|\n/u).length - Number(/(?:\r\n|\r|\n)$/u.test(source));
const countTopLevelTests = (source) => [
  ...source.matchAll(/^[ \t]*test[ \t]*(?:\.(?:skip|todo|only)[ \t]*)?\(/gmu),
].length;

test('top-level test counting includes supported test call variants but excludes hooks', () => {
  const source = [
    "test('direct', () => {});",
    "test ('space', () => {});",
    "  test   ('indented', () => {});",
    "test .skip ('skip', () => {});",
    " test.todo ('todo', () => {});",
    "\ttest.only   ('only', () => {});",
    'test.before(() => {});',
    'test.after(() => {});',
    'test.beforeEach(() => {});',
    'test.afterEach(() => {});',
  ].join('\n');

  assert.equal(countTopLevelTests(source), 6);
});

test('line counting handles common endings without counting a terminal newline', () => {
  assert.equal(countLines(''), 0);
  for (const newline of ['\n', '\r\n', '\r']) {
    assert.equal(countLines(`first${newline}second`), 2);
    assert.equal(countLines(`first${newline}second${newline}`), 2);
  }
});

test('script architecture guard suites stay focused and reusable', () => {
  const testFiles = fs
    .readdirSync(scriptsDir)
    .filter((name) => name.endsWith('.test.js'))
    .sort();

  assert.equal(testFiles.includes('packaging.test.js'), false, 'packaging.test.js must be removed');

  for (const name of testFiles) {
    const source = fs.readFileSync(path.join(scriptsDir, name), 'utf8');
    assert.ok(countLines(source) <= 2000, `${name} must not exceed 2000 lines`);
    assert.ok(
      countTopLevelTests(source) <= 60,
      `${name} must not exceed 60 top-level tests`,
    );
  }

  const supportDir = path.join(scriptsDir, 'test-support');
  if (fs.existsSync(supportDir)) {
    for (const name of fs.readdirSync(supportDir).filter((entry) => entry.endsWith('.js')).sort()) {
      const source = fs.readFileSync(path.join(supportDir, name), 'utf8');
      assert.ok(countLines(source) <= 1000, `${name} must not exceed 1000 lines`);
    }
  }
});
