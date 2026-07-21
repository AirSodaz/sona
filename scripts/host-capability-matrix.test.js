import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  HOST_CAPABILITY_MATRIX,
  HOST_PACKAGE_NAMES,
  capabilityIsWired,
  hostCapabilityStatuses,
  hostRuntimeDependencySet,
  workspacePackages,
} from './architecture-policy.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8').replace(/\r\n/gu, '\n');
}

const VALID_STATUSES = new Set(['yes', 'no', 'out of scope']);

test('host capability matrix covers every reviewed host and capability status', () => {
  assert.deepEqual(
    HOST_PACKAGE_NAMES.slice().sort(),
    ['sona', 'sona-cli', 'sona-uniffi-bind'].sort(),
    'the three product hosts must stay explicit in the capability matrix',
  );

  const packageNames = new Set(workspacePackages().map(({ packageName }) => packageName));
  const ids = new Set();

  for (const capability of HOST_CAPABILITY_MATRIX) {
    assert.equal(typeof capability.id, 'string');
    assert.ok(capability.id.length > 0, 'capability id must be non-empty');
    assert.equal(ids.has(capability.id), false, `duplicate capability id: ${capability.id}`);
    ids.add(capability.id);

    assert.ok(capability.enLabel, `${capability.id} must declare an English matrix label`);
    assert.ok(capability.zhLabel, `${capability.id} must declare a Chinese matrix label`);
    assert.ok(
      Array.isArray(capability.packages) && capability.packages.length > 0,
      `${capability.id} must list at least one workspace package`,
    );
    assert.ok(
      Array.isArray(capability.wiringPatterns) && capability.wiringPatterns.length > 0,
      `${capability.id} must list production wiring markers`,
    );

    for (const packageName of HOST_PACKAGE_NAMES) {
      const status = capability.status[packageName];
      assert.ok(
        VALID_STATUSES.has(status),
        `${capability.id}/${packageName} must use a reviewed status, got ${status}`,
      );
    }

    for (const dependency of capability.packages) {
      assert.ok(
        packageNames.has(dependency),
        `${capability.id} references unknown workspace package ${dependency}`,
      );
    }
  }
});

test('host Cargo manifests match the reviewed capability matrix dependencies', () => {
  for (const capability of HOST_CAPABILITY_MATRIX) {
    for (const packageName of HOST_PACKAGE_NAMES) {
      const status = capability.status[packageName];
      const runtimeDependencies = hostRuntimeDependencySet(packageName);

      for (const dependency of capability.packages) {
        if (capabilityIsWired(status)) {
          assert.ok(
            runtimeDependencies.has(dependency),
            `${packageName} status for ${capability.id} is ${status}, so Cargo.toml must depend on ${dependency}`,
          );
        } else {
          assert.equal(
            runtimeDependencies.has(dependency),
            false,
            `${packageName} status for ${capability.id} is ${status}, so Cargo.toml must not depend on ${dependency}`,
          );
        }
      }
    }
  }
});

test('CLI Sync and intentional host gaps stay encoded as non-yes matrix statuses', () => {
  const sync = HOST_CAPABILITY_MATRIX.find((capability) => capability.id === 'sync');
  assert.ok(sync, 'sync capability must remain in the matrix');
  assert.equal(sync.status['sona-cli'], 'out of scope');
  assert.deepEqual(hostCapabilityStatuses(sync), ['yes', 'out of scope', 'yes']);

  const onlineAsr = HOST_CAPABILITY_MATRIX.find((capability) => capability.id === 'online-asr');
  assert.equal(onlineAsr.status['sona-cli'], 'no');

  const modelDownloads = HOST_CAPABILITY_MATRIX.find(
    (capability) => capability.id === 'model-downloads',
  );
  assert.equal(modelDownloads.status['sona-uniffi-bind'], 'no');

  const mediaDetector = HOST_CAPABILITY_MATRIX.find(
    (capability) => capability.id === 'media-detector',
  );
  assert.equal(mediaDetector.status['sona-uniffi-bind'], 'no');

  const apiServer = HOST_CAPABILITY_MATRIX.find((capability) => capability.id === 'api-server');
  assert.equal(apiServer.status['sona-uniffi-bind'], 'no');

  const tsBind = HOST_CAPABILITY_MATRIX.find((capability) => capability.id === 'ts-bind');
  assert.deepEqual(hostCapabilityStatuses(tsBind), ['yes', 'no', 'no']);
});

test('stable architecture guides publish the host capability matrix', () => {
  const guides = [
    {
      source: read('docs', 'architecture.md'),
      title: 'English',
      labels: HOST_CAPABILITY_MATRIX.map((capability) => capability.enLabel),
    },
    {
      source: read('docs', 'architecture.zh-CN.md'),
      title: 'Chinese',
      labels: HOST_CAPABILITY_MATRIX.map((capability) => capability.zhLabel),
    },
  ];

  for (const { source, title, labels } of guides) {
    assert.match(source, /<a id="host-capability-matrix"><\/a>/u);
    for (const label of labels) {
      const escaped = label.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
      assert.match(
        source,
        new RegExp(`\\|\\s*${escaped}\\s*\\|`, 'u'),
        `${title} architecture guide must publish matrix row ${label}`,
      );
    }

    assert.match(
      source,
      /\|\s*Sync[\s\S]*?\|\s*yes\s*\|\s*out of scope\s*\|\s*yes\s*\|/u,
      `${title} architecture guide must keep the Sync matrix row`,
    );
    assert.match(
      source,
      /host-capability-matrix\.test\.js/u,
      `${title} architecture guide must document the host capability matrix verification command`,
    );
  }
});
