#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function parseSemver(versionString) {
  const normalized = String(versionString ?? '').trim().replace(/^v/u, '');
  const match = /^(\d+)\.(\d+)\.(\d+)$/u.exec(normalized);
  if (!match) {
    throw new Error(
      `Invalid version "${versionString}". Expected semantic version in X.Y.Z format (e.g. 0.8.2)`,
    );
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    version: `${match[1]}.${match[2]}.${match[3]}`,
  };
}

export function computeNextVersion(currentVersion, bumpType) {
  const parsed = parseSemver(currentVersion);
  switch (bumpType) {
    case 'patch':
      return `${parsed.major}.${parsed.minor}.${parsed.patch + 1}`;
    case 'minor':
      return `${parsed.major}.${parsed.minor + 1}.0`;
    case 'major':
      return `${parsed.major + 1}.0.0`;
    default:
      return parseSemver(bumpType).version;
  }
}

export function updateJsonFile(filePath, updater) {
  const raw = fs.readFileSync(filePath, 'utf8');
  const json = JSON.parse(raw);
  updater(json);
  fs.writeFileSync(filePath, `${JSON.stringify(json, null, 2)}\n`, 'utf8');
}

export function replaceInFile(filePath, regex, replacement) {
  const content = fs.readFileSync(filePath, 'utf8');
  if (!regex.test(content)) {
    throw new Error(`Pattern ${regex} not found in ${filePath}`);
  }
  const updated = content.replace(regex, replacement);
  fs.writeFileSync(filePath, updated, 'utf8');
}

export function bumpVersionFiles(targetVersionInput, options = {}) {
  const root = options.repoRoot ?? defaultRepoRoot;
  const rootPackageJsonPath = path.join(root, 'package.json');
  const currentVersion = JSON.parse(fs.readFileSync(rootPackageJsonPath, 'utf8')).version;
  const targetVersion = computeNextVersion(currentVersion, targetVersionInput);

  // 1. Root package.json
  updateJsonFile(rootPackageJsonPath, (json) => {
    json.version = targetVersion;
  });

  // 2. Cargo.toml
  const cargoTomlPath = path.join(root, 'Cargo.toml');
  replaceInFile(
    cargoTomlPath,
    /(\[workspace\.package\][\s\S]*?\nversion\s*=\s*)"[^"]+"/u,
    `$1"${targetVersion}"`,
  );

  // 3. Desktop frontend package.json
  const frontendPackageJsonPath = path.join(root, 'platforms', 'desktop', 'frontend', 'package.json');
  updateJsonFile(frontendPackageJsonPath, (json) => {
    json.version = targetVersion;
  });

  // 4. Desktop tauri.conf.json
  const tauriConfPath = path.join(root, 'platforms', 'desktop', 'tauri.conf.json');
  replaceInFile(
    tauriConfPath,
    /(^\s*"version"\s*:\s*)"[^"]+"/mu,
    `$1"${targetVersion}"`,
  );

  // 5. Android app build.gradle.kts
  const androidAppGradlePath = path.join(root, 'platforms', 'android', 'client', 'app', 'build.gradle.kts');
  replaceInFile(
    androidAppGradlePath,
    /(suppliedAndroidVersionName\.ifEmpty\s*\{\s*")[^"]+("\s*\})/u,
    `$1${targetVersion}$2`,
  );

  // 6. Android sample-library build.gradle.kts
  const androidSampleLibraryPath = path.join(
    root,
    'platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts',
  );
  replaceInFile(
    androidSampleLibraryPath,
    /(^version\s*=\s*)"[^"]+"/mu,
    `$1"${targetVersion}"`,
  );

  // 7. Android consumer-library build.gradle.kts
  const androidConsumerLibraryPath = path.join(
    root,
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'build.gradle.kts',
  );
  replaceInFile(
    androidConsumerLibraryPath,
    /(com\.sona:sona-uniffi-bindings:)[^\s")]+/gu,
    `$1${targetVersion}`,
  );

  // 8. Android README.md
  const androidReadmePath = path.join(root, 'platforms', 'android', 'README.md');
  replaceInFile(
    androidReadmePath,
    /(com\.sona:sona-uniffi-bindings:)[^\s"`)]+/gu,
    `$1${targetVersion}`,
  );

  return {
    previousVersion: currentVersion,
    targetVersion,
  };
}

export function runCli(argv = process.argv.slice(2), options = {}) {
  const args = argv.filter((arg) => !arg.startsWith('--'));
  const noCargoCheck = argv.includes('--no-cargo-check');
  const noVerify = argv.includes('--no-verify');
  const root = options.repoRoot ?? defaultRepoRoot;

  if (args.length === 0) {
    console.error(
      'Usage: node scripts/bump-version.js <new-version | patch | minor | major> [--no-cargo-check] [--no-verify]',
    );
    process.exit(1);
  }

  const targetInput = args[0];
  const { previousVersion, targetVersion } = bumpVersionFiles(targetInput, { repoRoot: root });

  console.log(`Updated metadata from ${previousVersion} to ${targetVersion} in 8 configuration/doc files.`);

  if (!noCargoCheck) {
    console.log('Synchronizing Cargo.lock via `cargo check --workspace --tests`...');
    const cargoResult = spawnSync('cargo', ['check', '--workspace', '--tests'], {
      cwd: root,
      stdio: 'inherit',
    });
    if (cargoResult.status !== 0) {
      console.error('Failed to synchronize Cargo.lock');
      process.exit(cargoResult.status ?? 1);
    }
  }

  if (!noVerify) {
    console.log('Verifying version consistency...');
    const testResult = spawnSync(
      process.execPath,
      ['--test', 'scripts/version-consistency.test.js'],
      {
        cwd: root,
        stdio: 'inherit',
      },
    );
    if (testResult.status !== 0) {
      console.error('Version consistency verification failed');
      process.exit(testResult.status ?? 1);
    }
  }

  console.log(`\n✔ Successfully bumped Sona from ${previousVersion} to ${targetVersion}`);
  console.log('Suggested next commands:');
  console.log(`  git commit -am "chore: bump version to ${targetVersion}"`);
  console.log(`  git tag v${targetVersion}\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  runCli();
}
