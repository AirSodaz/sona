#!/usr/bin/env node
import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

/**
 * Resolves the build channel based on explicit env or CI triggers.
 *
 * @param {Record<string, string | undefined>} env
 * @returns {'stable' | 'nightly' | 'ci' | 'dev'}
 */
export function resolveChannel(env = process.env) {
  const explicit = env.SONA_CHANNEL?.toLowerCase().trim();
  if (explicit === 'stable' || explicit === 'nightly' || explicit === 'ci' || explicit === 'dev') {
    return explicit;
  }

  // Release tag triggered
  if (env.GITHUB_REF?.startsWith('refs/tags/v')) {
    return 'stable';
  }

  // Nightly workflow
  if (
    env.SONA_NIGHTLY === '1' ||
    env.GITHUB_WORKFLOW?.toLowerCase().includes('nightly')
  ) {
    return 'nightly';
  }

  // CI build (PR or branch push)
  if (env.CI === 'true' || env.GITHUB_ACTIONS === 'true') {
    return 'ci';
  }

  return 'dev';
}

/**
 * Retrieves the short Git commit SHA safely.
 *
 * @param {string} cwd
 * @returns {string}
 */
export function getGitCommitSha(cwd = repoRoot) {
  try {
    const sha = execSync('git rev-parse --short=7 HEAD', {
      cwd,
      stdio: ['ignore', 'pipe', 'ignore'],
      encoding: 'utf8',
    }).trim();
    return sha || 'unknown';
  } catch {
    return 'unknown';
  }
}

/**
 * Checks whether the git workspace has uncommitted changes.
 *
 * @param {string} cwd
 * @returns {boolean}
 */
export function checkGitDirty(cwd = repoRoot) {
  try {
    const status = execSync('git status --porcelain', {
      cwd,
      stdio: ['ignore', 'pipe', 'ignore'],
      encoding: 'utf8',
    }).trim();
    return status.length > 0;
  } catch {
    return false;
  }
}

/**
 * Computes the full structured AppBuildInfo.
 *
 * @param {{
 *   env?: Record<string, string | undefined>;
 *   cwd?: string;
 *   packageVersion?: string;
 * }} [options]
 */
export function getBuildInfo(options = {}) {
  const env = options.env ?? process.env;
  const cwd = options.cwd ?? repoRoot;

  let semver = options.packageVersion;
  if (!semver) {
    try {
      const pkgPath = path.join(cwd, 'package.json');
      const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
      semver = pkg.version;
    } catch {
      semver = '0.0.0';
    }
  }

  const channel = resolveChannel(env);
  const commitSha = env.SONA_COMMIT_SHA || getGitCommitSha(cwd);
  const isDirty =
    channel === 'dev'
      ? env.SONA_IS_DIRTY !== undefined
        ? env.SONA_IS_DIRTY === 'true' || env.SONA_IS_DIRTY === '1'
        : checkGitDirty(cwd)
      : false;

  const buildNumber = env.GITHUB_RUN_NUMBER || env.SONA_BUILD_NUMBER || null;
  const buildTimestamp =
    env.SOURCE_DATE_EPOCH
      ? new Date(parseInt(env.SOURCE_DATE_EPOCH, 10) * 1000).toISOString()
      : env.SONA_BUILD_TIMESTAMP || new Date().toISOString();

  let displayVersion = semver;
  if (channel === 'stable') {
    displayVersion = semver;
  } else if (channel === 'nightly') {
    displayVersion = `${semver}-nightly.${buildNumber ?? '0'}`;
  } else if (channel === 'ci') {
    const numPart = buildNumber ? `ci.${buildNumber}` : 'ci';
    displayVersion = `${semver}+${numPart}.${commitSha}`;
  } else {
    // dev
    const dirtySuffix = isDirty ? '.dirty' : '';
    displayVersion = `${semver}+dev.${commitSha}${dirtySuffix}`;
  }

  const isUpdateSupported = channel === 'stable' || channel === 'nightly';

  return {
    semver,
    channel,
    commitSha,
    isDirty,
    buildNumber,
    buildTimestamp,
    displayVersion,
    isUpdateSupported,
  };
}

/**
 * Formats build info as Cargo build script rustc-env directives.
 *
 * @param {ReturnType<typeof getBuildInfo>} info
 * @returns {string}
 */
export function formatCargoDirectives(info) {
  const lines = [
    `cargo:rustc-env=SONA_BUILD_SEMVER=${info.semver}`,
    `cargo:rustc-env=SONA_BUILD_CHANNEL=${info.channel}`,
    `cargo:rustc-env=SONA_COMMIT_SHA=${info.commitSha}`,
    `cargo:rustc-env=SONA_IS_DIRTY=${info.isDirty}`,
    `cargo:rustc-env=SONA_BUILD_NUMBER=${info.buildNumber ?? ''}`,
    `cargo:rustc-env=SONA_BUILD_TIMESTAMP=${info.buildTimestamp}`,
    `cargo:rustc-env=SONA_DISPLAY_VERSION=${info.displayVersion}`,
    `cargo:rustc-env=SONA_IS_UPDATE_SUPPORTED=${info.isUpdateSupported}`,
  ];
  return lines.join('\n');
}

// When executed directly
if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(__filename)) {
  const isCargoMode = process.argv.includes('--cargo');
  const info = getBuildInfo();

  if (isCargoMode) {
    console.log(formatCargoDirectives(info));
  } else {
    console.log(JSON.stringify(info, null, 2));
  }
}
