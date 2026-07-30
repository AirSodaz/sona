import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const corePortsDir = path.join(repoRoot, 'core', 'src', 'ports');

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8').replace(/\r\n/gu, '\n');
}

// Domain aggregates that own their own ports under `core/src/<domain>/`.
// A capability port in `core/src/ports/` must not be named after any of them.
const DOMAIN_AGGREGATES = [
  'History',
  'Tag',
  'Automation',
  'Backup',
  'Recovery',
  'TaskLedger',
  'Dashboard',
  'StorageUsage',
  'AppConfig',
];

// Capability ports that must stay in `core/src/ports/`. Guards the inverse
// drift: someone emptying the shared module by pushing everything into domains.
const REQUIRED_CAPABILITY_PORTS = [
  'FileSystemPort',
  'PathPort',
  'ClockPort',
  'EventEmitterPort',
];

function corePortFiles() {
  return fs
    .readdirSync(corePortsDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
    .map((entry) => ({
      name: entry.name,
      source: fs.readFileSync(path.join(corePortsDir, entry.name), 'utf8'),
    }));
}

function declaredTraits(source) {
  return [...source.matchAll(/^pub trait (\w+)/gmu)].map(([, name]) => name);
}

test('core capability ports declare no persistence-shaped traits', () => {
  const offenders = [];
  for (const { name, source } of corePortFiles()) {
    for (const traitName of declaredTraits(source)) {
      if (/(?:Store|Repository)$/u.test(traitName)) {
        offenders.push(`core/src/ports/${name}: ${traitName}`);
      }
    }
  }

  assert.deepEqual(
    offenders,
    [],
    'a *Store/*Repository trait is domain-owned and belongs in core/src/<domain>/, ' +
      'not in the shared capability port module',
  );
});

test('core capability ports are not named after a domain aggregate', () => {
  const offenders = [];
  for (const { name, source } of corePortFiles()) {
    for (const traitName of declaredTraits(source)) {
      const aggregate = DOMAIN_AGGREGATES.find((candidate) => traitName.startsWith(candidate));
      if (aggregate) {
        offenders.push(`core/src/ports/${name}: ${traitName} (aggregate ${aggregate})`);
      }
    }
  }

  assert.deepEqual(
    offenders,
    [],
    'a trait named after a domain aggregate belongs under core/src/<domain>/',
  );
});

test('shared capability ports stay in the shared port module', () => {
  const declared = new Set(corePortFiles().flatMap(({ source }) => declaredTraits(source)));

  for (const capability of REQUIRED_CAPABILITY_PORTS) {
    assert.ok(
      declared.has(capability),
      `${capability} is a domain-agnostic capability and must stay in core/src/ports/`,
    );
  }
});

test('port placement rules are published in both architecture guides', () => {
  const guides = [
    { source: read('docs', 'architecture.md'), title: 'docs/architecture.md' },
    { source: read('docs', 'architecture.zh-CN.md'), title: 'docs/architecture.zh-CN.md' },
  ];

  for (const { source, title } of guides) {
    assert.match(
      source,
      /<a id="port-placement"><\/a>/u,
      `${title} must anchor the port placement section`,
    );
    assert.match(
      source,
      /core\/src\/ports\/<capability>\.rs/u,
      `${title} must document the capability port home`,
    );
    assert.match(
      source,
      /core\/src\/<domain>\/ports\.rs/u,
      `${title} must document the preferred domain port home`,
    );
    assert.match(
      source,
      /scripts\/core-port-placement\.test\.js/u,
      `${title} must name the test that enforces the rules`,
    );
    for (const aggregate of DOMAIN_AGGREGATES) {
      assert.match(
        source,
        new RegExp(`\\b${aggregate}\\b`, 'u'),
        `${title} must list ${aggregate} among the domain aggregates`,
      );
    }
  }
});
