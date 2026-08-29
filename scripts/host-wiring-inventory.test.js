import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  HOST_CAPABILITY_MATRIX,
  HOST_PACKAGE_NAMES,
  capabilityIsWired,
  hostPackage,
  hostProductionSource,
  hostRuntimeDependencySet,
} from './architecture-policy.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function patternMatches(source, pattern) {
  return new RegExp(pattern, 'u').test(source);
}

test('host production sources wire every yes capability and omit non-yes capabilities', () => {
  for (const capability of HOST_CAPABILITY_MATRIX) {
    for (const packageName of HOST_PACKAGE_NAMES) {
      const status = capability.status[packageName];
      const source = hostProductionSource(packageName);
      const runtimeDependencies = hostRuntimeDependencySet(packageName);
      const matched = capability.wiringPatterns.filter((pattern) =>
        patternMatches(source, pattern),
      );

      if (capabilityIsWired(status)) {
        assert.ok(
          matched.length > 0,
          `${packageName} status for ${capability.id} is yes, so production src must match one of: ${capability.wiringPatterns.join(', ')}`,
        );
        for (const dependency of capability.packages) {
          assert.ok(
            runtimeDependencies.has(dependency),
            `${packageName} wires ${capability.id} in source but is missing Cargo dependency ${dependency}`,
          );
        }
      } else {
        assert.deepEqual(
          matched,
          [],
          `${packageName} status for ${capability.id} is ${status}, so production src must not match wiring markers: ${matched.join(', ')}`,
        );
        for (const dependency of capability.packages) {
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

test('host wiring inventory stays aligned with reviewed composition roots', () => {
  const expectedRoots = new Map([
    [
      'sona',
      [
        'platforms/desktop/src/app/setup.rs',
        'platforms/desktop/src/platform/',
      ],
    ],
    [
      'sona-cli',
      ['platforms/cli/src/lib.rs'],
    ],
    [
      'sona-uniffi-bind',
      [
        'platforms/uniffi/src/application_context.rs',
        'platforms/uniffi/src/lib.rs',
      ],
    ],
  ]);

  for (const packageName of HOST_PACKAGE_NAMES) {
    const pkg = hostPackage(packageName);
    assert.equal(pkg.role ?? 'host', 'host');
    assert.ok(fs.existsSync(path.join(repoRoot, pkg.memberPath, 'src')));

    for (const rootPath of expectedRoots.get(packageName)) {
      const absolute = path.join(repoRoot, ...rootPath.split('/'));
      assert.ok(
        fs.existsSync(absolute),
        `${packageName} composition root path must exist: ${rootPath}`,
      );
    }
  }

});
