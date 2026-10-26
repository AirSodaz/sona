import assert from 'node:assert/strict';
import test from 'node:test';
import {
  formatCargoDirectives,
  getBuildInfo,
  resolveChannel,
} from './get-build-info.mjs';

test('resolveChannel determines channel correctly by precedence', () => {
  // 1. Explicit SONA_CHANNEL override takes top priority
  assert.equal(resolveChannel({ SONA_CHANNEL: 'nightly', CI: 'true' }), 'nightly');
  assert.equal(resolveChannel({ SONA_CHANNEL: 'dev', CI: 'true' }), 'dev');
  assert.equal(resolveChannel({ SONA_CHANNEL: 'stable' }), 'stable');
  assert.equal(resolveChannel({ SONA_CHANNEL: 'ci' }), 'ci');

  // 2. Release tag
  assert.equal(
    resolveChannel({ GITHUB_REF: 'refs/tags/v1.0.0', CI: 'true' }),
    'stable',
  );

  // 3. Nightly workflow
  assert.equal(resolveChannel({ SONA_NIGHTLY: '1', CI: 'true' }), 'nightly');
  assert.equal(
    resolveChannel({ GITHUB_WORKFLOW: 'Nightly Release', CI: 'true' }),
    'nightly',
  );

  // 4. CI environment
  assert.equal(resolveChannel({ CI: 'true' }), 'ci');
  assert.equal(resolveChannel({ GITHUB_ACTIONS: 'true' }), 'ci');

  // 5. Default fallback to dev
  assert.equal(resolveChannel({}), 'dev');
});

test('getBuildInfo formats versions strictly according to SemVer 2.0 and specification', () => {
  // Stable channel
  const stable = getBuildInfo({
    packageVersion: '1.2.3',
    env: { SONA_CHANNEL: 'stable', SONA_COMMIT_SHA: 'abc1234' },
  });
  assert.equal(stable.channel, 'stable');
  assert.equal(stable.displayVersion, '1.2.3');
  assert.equal(stable.isUpdateSupported, true);

  // Nightly channel
  const nightly = getBuildInfo({
    packageVersion: '1.2.3',
    env: {
      SONA_CHANNEL: 'nightly',
      SONA_COMMIT_SHA: 'abc1234',
      GITHUB_RUN_NUMBER: '99',
    },
  });
  assert.equal(nightly.channel, 'nightly');
  assert.equal(nightly.displayVersion, '1.2.3-nightly.99');
  assert.equal(nightly.isUpdateSupported, true);

  // CI channel with build number
  const ciWithNum = getBuildInfo({
    packageVersion: '1.2.3',
    env: {
      SONA_CHANNEL: 'ci',
      SONA_COMMIT_SHA: 'abc1234',
      GITHUB_RUN_NUMBER: '42',
    },
  });
  assert.equal(ciWithNum.channel, 'ci');
  assert.equal(ciWithNum.displayVersion, '1.2.3+ci.42.abc1234');
  assert.equal(ciWithNum.isUpdateSupported, false);

  // CI channel without build number
  const ciWithoutNum = getBuildInfo({
    packageVersion: '1.2.3',
    env: {
      SONA_CHANNEL: 'ci',
      SONA_COMMIT_SHA: 'abc1234',
    },
  });
  assert.equal(ciWithoutNum.displayVersion, '1.2.3+ci.abc1234');
  assert.equal(ciWithoutNum.isUpdateSupported, false);

  // Dev channel (clean)
  const devClean = getBuildInfo({
    packageVersion: '1.2.3',
    env: {
      SONA_CHANNEL: 'dev',
      SONA_COMMIT_SHA: 'abc1234',
      SONA_IS_DIRTY: 'false',
    },
  });
  assert.equal(devClean.channel, 'dev');
  assert.equal(devClean.displayVersion, '1.2.3+dev.abc1234');
  assert.equal(devClean.isDirty, false);
  assert.equal(devClean.isUpdateSupported, false);

  // Dev channel (dirty)
  const devDirty = getBuildInfo({
    packageVersion: '1.2.3',
    env: {
      SONA_CHANNEL: 'dev',
      SONA_COMMIT_SHA: 'abc1234',
      SONA_IS_DIRTY: 'true',
    },
  });
  assert.equal(devDirty.channel, 'dev');
  assert.equal(devDirty.displayVersion, '1.2.3+dev.abc1234.dirty');
  assert.equal(devDirty.isDirty, true);
  assert.equal(devDirty.isUpdateSupported, false);
});

test('getBuildInfo falls back gracefully when git info is absent', () => {
  const fallback = getBuildInfo({
    packageVersion: '0.9.8',
    cwd: '/nonexistent/path',
    env: { SONA_CHANNEL: 'dev' },
  });
  assert.equal(fallback.commitSha, 'unknown');
  assert.equal(fallback.isDirty, false);
  assert.equal(fallback.displayVersion, '0.9.8+dev.unknown');
});

test('formatCargoDirectives outputs valid rustc-env directives', () => {
  const info = {
    semver: '0.9.8',
    channel: 'dev',
    commitSha: '3f2a1b0',
    isDirty: true,
    buildNumber: null,
    buildTimestamp: '2026-10-26T00:00:00.000Z',
    displayVersion: '0.9.8+dev.3f2a1b0.dirty',
    isUpdateSupported: false,
  };
  const directives = formatCargoDirectives(info);
  assert.match(directives, /cargo:rustc-env=SONA_BUILD_SEMVER=0\.9\.8/);
  assert.match(directives, /cargo:rustc-env=SONA_BUILD_CHANNEL=dev/);
  assert.match(directives, /cargo:rustc-env=SONA_COMMIT_SHA=3f2a1b0/);
  assert.match(directives, /cargo:rustc-env=SONA_IS_DIRTY=true/);
  assert.match(directives, /cargo:rustc-env=SONA_BUILD_NUMBER=/);
  assert.match(directives, /cargo:rustc-env=SONA_DISPLAY_VERSION=0\.9\.8\+dev\.3f2a1b0\.dirty/);
  assert.match(directives, /cargo:rustc-env=SONA_IS_UPDATE_SUPPORTED=false/);
});
