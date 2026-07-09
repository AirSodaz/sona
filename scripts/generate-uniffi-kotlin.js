#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const args = process.argv.slice(2);

function readOption(name, fallback) {
  const index = args.indexOf(name);
  if (index === -1) {
    return fallback;
  }
  const value = args[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function run(command, commandArgs) {
  const result = spawnSync(command, commandArgs, {
    cwd: repoRoot,
    env: process.env,
    stdio: 'inherit',
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${commandArgs.join(' ')} failed with exit code ${result.status}`);
  }
}

function dynamicLibraryName(crateName) {
  if (process.platform === 'win32') {
    return `${crateName}.dll`;
  }
  if (process.platform === 'darwin') {
    return `lib${crateName}.dylib`;
  }
  return `lib${crateName}.so`;
}

const profile = readOption('--profile', 'release');
const releaseFlag = profile === 'release' ? ['--release'] : [];
const targetDir = path.resolve(
  readOption('--target-dir', process.env.CARGO_TARGET_DIR ?? path.join(repoRoot, 'target')),
);
const profileDir = profile === 'release' ? 'release' : 'debug';
const outDir = path.resolve(
  readOption(
    '--out-dir',
    path.join(repoRoot, 'platforms', 'android', 'generated', 'source', 'uniffi', 'main', 'kotlin'),
  ),
);
const skipBuild = args.includes('--skip-build');
const cargo = process.env.CARGO ?? 'cargo';

if (!skipBuild) {
  run(cargo, ['build', '-p', 'sona-uniffi-bind', ...releaseFlag]);
}

const libraryPath = path.join(targetDir, profileDir, dynamicLibraryName('sona_uniffi_bind'));
if (!fs.existsSync(libraryPath)) {
  throw new Error(`Missing UniFFI library at ${libraryPath}. Run cargo build -p sona-uniffi-bind first.`);
}

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });
run(cargo, [
  'run',
  '-p',
  'sona-uniffi-bindgen',
  '--',
  'generate',
  '--library',
  libraryPath,
  '--language',
  'kotlin',
  '--no-format',
  '--out-dir',
  outDir,
]);

console.log(`Generated Sona UniFFI Kotlin bindings in ${outDir}`);
