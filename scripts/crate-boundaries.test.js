import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  ALLOWED_TARGET_ROLES,
  EXPECTED_ROLES,
  REVIEWED_OUTBOUND_ADAPTER_EDGES,
  ROLE_DIRECTORY_ROOTS,
  workspacePackages,
} from './architecture-policy.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const APPLICATION_SERVICE_PATHS = [
  ['automation', 'service.rs'],
  ['backup', 'service.rs'],
  ['config', 'service.rs'],
  ['dashboard', 'service.rs'],
  ['diagnostics', 'service.rs'],
  ['export', 'service.rs'],
  ['history', 'mutation_service.rs'],
  ['history', 'query_service.rs'],
  ['llm', 'tasks', 'service.rs'],
  ['llm', 'runtime_service.rs'],
  ['recovery', 'service.rs'],
  ['storage_usage', 'service.rs'],
  ['tag', 'service.rs'],
  ['task_ledger', 'service.rs'],
];

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8').replace(/\r\n/gu, '\n');
}

function rustSourceFiles(relativeDir) {
  const absoluteDir = path.join(repoRoot, relativeDir);
  const files = [];

  for (const entry of fs.readdirSync(absoluteDir, { withFileTypes: true })) {
    const relativePath = path.join(relativeDir, entry.name);
    if (entry.isDirectory()) {
      files.push(...rustSourceFiles(relativePath));
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      files.push(relativePath);
    }
  }

  return files;
}

test('workspace crate role registry is complete and matches manifest metadata', () => {
  const packages = workspacePackages();
  assert.deepEqual(
    packages.map(({ packageName }) => packageName).sort(),
    [...EXPECTED_ROLES.keys()].sort(),
    'workspace members and the reviewed crate-role registry must stay in sync',
  );

  for (const { memberPath, packageName, roles } of packages) {
    assert.equal(
      roles.length,
      1,
      `${packageName} (${memberPath}) must declare exactly one [package.metadata.sona] role`,
    );
    assert.equal(
      roles[0],
      EXPECTED_ROLES.get(packageName),
      `${packageName} must keep its reviewed architecture role`,
    );
    assert.ok(
      ALLOWED_TARGET_ROLES.has(roles[0]),
      `${packageName} declares unsupported architecture role ${roles[0]}`,
    );
  }
});

test('workspace runtime dependencies follow the reviewed role direction', () => {
  const packages = workspacePackages();
  const packageByName = new Map(packages.map((pkg) => [pkg.packageName, pkg]));
  const observedOutboundExceptions = new Map();

  for (const source of packages) {
    const sourceRole = EXPECTED_ROLES.get(source.packageName);
    for (const dependencyName of new Set(source.runtimeDependencies)) {
      const target = packageByName.get(dependencyName);
      if (!target) {
        continue;
      }

      const targetRole = EXPECTED_ROLES.get(target.packageName);
      const edge = `${source.packageName}->${target.packageName}`;
      if (REVIEWED_OUTBOUND_ADAPTER_EDGES.has(edge)) {
        observedOutboundExceptions.set(edge, REVIEWED_OUTBOUND_ADAPTER_EDGES.get(edge));
        continue;
      }

      assert.ok(
        ALLOWED_TARGET_ROLES.get(sourceRole).has(targetRole),
        `${source.packageName} (${sourceRole}) must not depend on ${target.packageName} (${targetRole})`,
      );
    }
  }

  assert.deepEqual(
    [...observedOutboundExceptions.entries()].sort(),
    [...REVIEWED_OUTBOUND_ADAPTER_EDGES.entries()].sort(),
    'outbound-adapter dependency exceptions must be explicit and must not become stale',
  );
});

test('migrated use-case services are owned only by sona-application', () => {
  for (const relativePath of APPLICATION_SERVICE_PATHS) {
    const displayPath = relativePath.join('/');
    assert.ok(
      fs.existsSync(path.join(repoRoot, 'application', 'src', ...relativePath)),
      `application/src/${displayPath} must remain in the application layer`,
    );
    assert.ok(
      !fs.existsSync(path.join(repoRoot, 'core', 'src', ...relativePath)),
      `core/src/${displayPath} is a stale application-layer implementation`,
    );
  }
});

test('Core does not reintroduce application service structs', () => {
  const staleServices = rustSourceFiles('core/src').flatMap((relativePath) => {
    const source = read(relativePath);
    return [
      ...source.matchAll(/^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w*Service)\b/gmu),
    ].map((match) => ({
      relativePath,
      service: match[1],
    }));
  });

  assert.deepEqual(
    staleServices,
    [],
    `Core must keep use-case service structs in sona-application: ${JSON.stringify(staleServices)}`,
  );
});

test('crate role boundaries run in PR guardrails through the script-test glob', () => {
  const packageJson = JSON.parse(read('package.json'));
  assert.equal(
    packageJson.scripts['test:scripts'],
    'node --test --test-concurrency=1 scripts/*.test.js',
  );
  assert.match(
    read('.github', 'workflows', 'pr-guardrails.yml'),
    /- name: Run script tests[\s\S]*?run: pnpm run test:scripts/u,
  );
});

test('workspace crate paths match their reviewed role roots', () => {
  assert.deepEqual(
    [...ROLE_DIRECTORY_ROOTS.keys()].sort(),
    [...ALLOWED_TARGET_ROLES.keys()].sort(),
    'every architecture role must have one directory root',
  );

  for (const { memberPath, packageName, role } of workspacePackages()) {
    const expectedRoot = ROLE_DIRECTORY_ROOTS.get(role);
    assert.ok(expectedRoot, `${packageName} has no directory root for role ${role}`);
    assert.ok(
      memberPath === expectedRoot || memberPath.startsWith(`${expectedRoot}/`),
      `${packageName} (${role}) must live under ${expectedRoot}/, found ${memberPath}`,
    );
  }
});
