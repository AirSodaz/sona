import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  ALLOWED_TARGET_ROLES,
  EXPECTED_ROLES,
  HISTORICAL_PATH_ROLE_OVERRIDES,
  REVIEWED_OUTBOUND_ADAPTER_EDGES,
  workspacePackages,
} from './architecture-policy.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8').replace(/\r\n/gu, '\n');
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

test('stable architecture guides publish the reviewed roles and exceptions', () => {
  const guides = [
    {
      source: read('docs', 'architecture.md'),
      title: '# Sona Architecture',
      navigation: [
        '[English](architecture.md)',
        '[简体中文](architecture.zh-CN.md)',
        '[Project README](../README.md)',
        '[Contributing](../CONTRIBUTING.md)',
      ],
    },
    {
      source: read('docs', 'architecture.zh-CN.md'),
      title: '# Sona 架构',
      navigation: [
        '[English](architecture.md)',
        '[简体中文](architecture.zh-CN.md)',
        '[项目 README](../README.zh-CN.md)',
        '[参与贡献](../CONTRIBUTING.md)',
      ],
    },
  ];
  const anchors = [
    'architecture-roles',
    'directory-vs-role',
    'dependency-direction',
    'composition-roots',
    'host-capability-matrix',
    'error-boundaries',
    'compatibility-policy',
    'reviewed-exceptions',
    'compatibility-debt',
    'verification',
  ];

  for (const { source: guide, title, navigation } of guides) {
    assert.ok(guide.startsWith(`${title}\n`), `${title} must be the top-level title`);
    for (const link of navigation) {
      assert.ok(guide.includes(link), `${title} must link to ${link}`);
    }
    for (const anchor of anchors) {
      assert.match(guide, new RegExp(`<a id="${anchor}"></a>`, 'u'));
    }
    for (const [packageName, role] of EXPECTED_ROLES) {
      assert.match(
        guide,
        new RegExp(`\\|\\s*\`${packageName}\`\\s*\\|\\s*${role}\\s*\\|`, 'u'),
      );
    }
    for (const [edge, reason] of REVIEWED_OUTBOUND_ADAPTER_EDGES) {
      assert.match(guide, new RegExp(edge, 'u'));
      assert.match(guide, new RegExp(reason, 'u'));
    }

    assert.match(
      guide,
      /`adapters\/sync\/`[\s\S]*?`sona-sync`[\s\S]*?application/u,
      `${title} must map adapters/sync to the application role`,
    );
    assert.match(
      guide,
      /`adapters\/uniffi_bind\/`[\s\S]*?`sona-uniffi-bind`[\s\S]*?host/u,
      `${title} must map adapters/uniffi_bind to the host role`,
    );
    assert.match(
      guide,
      /SqliteApplicationContext/u,
      `${title} must document the shared SqliteApplicationContext composition type`,
    );
    assert.match(
      guide,
      /\|\s*Sync[\s\S]*?\|\s*yes\s*\|\s*out of scope\s*\|\s*yes\s*\|/u,
      `${title} must publish the three-host Sync capability matrix row`,
    );
    assert.match(
      guide,
      /core\/src\/project/u,
      `${title} must record that the empty Project core module was removed`,
    );
    assert.match(
      guide,
      /history_update_project_assignments/u,
      `${title} must inventory Project-named host compatibility leaves`,
    );
    assert.match(
      guide,
      /tempfile/u,
      `${title} must document the intentional sona-sqlite tempfile production dependency`,
    );
  }

  for (const readme of [read('README.md'), read('README.zh-CN.md')]) {
    assert.match(readme, /docs\/architecture\.md/u);
    assert.match(readme, /docs\/architecture\.zh-CN\.md/u);
  }
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

test('historical path role overrides keep soft-migration READMEs and metadata aligned', () => {
  const packagesByName = new Map(
    workspacePackages().map((pkg) => [pkg.packageName, pkg]),
  );

  assert.ok(
    HISTORICAL_PATH_ROLE_OVERRIDES.length >= 2,
    'reviewed historical path overrides must stay explicit',
  );

  for (const override of HISTORICAL_PATH_ROLE_OVERRIDES) {
    assert.equal(
      EXPECTED_ROLES.get(override.packageName),
      override.role,
      `${override.packageName} registry role must match historical override`,
    );

    const pkg = packagesByName.get(override.packageName);
    assert.ok(pkg, `${override.packageName} must remain a workspace member`);
    assert.equal(pkg.memberPath, override.memberPath);
    assert.equal(pkg.roles[0], override.role);

    const readmePath = path.join(repoRoot, ...override.readmeRelative.split('/'));
    assert.ok(
      fs.existsSync(readmePath),
      `${override.readmeRelative} must document the historical path role mismatch`,
    );
    const readme = fs.readFileSync(readmePath, 'utf8').replace(/\r\n/gu, '\n');
    assert.match(
      readme,
      new RegExp(`\\b${override.packageName}\\b`, 'u'),
      `${override.readmeRelative} must name ${override.packageName}`,
    );
    assert.match(
      readme,
      new RegExp(`\\b${override.role}\\b`, 'u'),
      `${override.readmeRelative} must state role ${override.role}`,
    );
    assert.match(
      readme,
      /historical/iu,
      `${override.readmeRelative} must mark the path as historical`,
    );
    assert.match(
      readme,
      /docs\/architecture\.md/u,
      `${override.readmeRelative} must link to the English architecture guide`,
    );
    assert.match(
      readme,
      /docs\/architecture\.zh-CN\.md/u,
      `${override.readmeRelative} must link to the Chinese architecture guide`,
    );
  }

  const contributing = read('CONTRIBUTING.md');
  assert.match(
    contributing,
    /Do not\s+infer a crate.s architecture role from its folder name/iu,
  );
  for (const override of HISTORICAL_PATH_ROLE_OVERRIDES) {
    assert.match(
      contributing,
      new RegExp(
        `${override.memberPath.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')}[\\s\\S]*?${override.packageName}[\\s\\S]*?${override.role}`,
        'u',
      ),
      'CONTRIBUTING.md must inventory historical path role mismatches',
    );
  }
});
