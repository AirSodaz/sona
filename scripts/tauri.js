import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const tauriBinary = path.resolve(
  repoRoot,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'tauri.cmd' : 'tauri'
);

const desktopTauriConfig = path.join(repoRoot, 'platforms', 'desktop', 'tauri.conf.json');
const args = process.argv.slice(2);
const tauriArgs = withDesktopConfig(args);
const command = tauriArgs[0];
const UNIVERSAL_MACOS_TARGET = 'universal-apple-darwin';
const UNIVERSAL_MACOS_SOURCE_TARGETS = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

if (command === 'build' || command === 'bundle') {
  prepareBundleResources(tauriArgs);
}

const tauriResult = spawnSync(tauriBinary, tauriArgs, {
  cwd: repoRoot,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

if (tauriResult.error) {
  console.error("Failed to spawn tauri:", tauriResult.error);
  process.exit(1);
}

process.exit(tauriResult.status ?? 1);

function withDesktopConfig(commandArgs) {
  const [command, ...rest] = commandArgs;
  const supportsConfig = ['dev', 'build', 'bundle'].includes(command);
  const hasConfig = rest.some((argument) => argument === '--config' || argument.startsWith('--config='));

  if (!supportsConfig || hasConfig) {
    return commandArgs;
  }

  return [command, '--config', desktopTauriConfig, ...commandArgs.slice(1)];
}

function prepareBundleResources(tauriArgs) {
  runRequired(
    'setup ffmpeg sidecar',
    process.execPath,
    [path.resolve(__dirname, 'setup-ffmpeg.js')],
    {
      cwd: repoRoot,
      stdio: 'inherit',
    },
  );

  if (process.env.SONA_SKIP_CLI_RESOURCE_PREP === '1') {
    return;
  }

  buildStandaloneCli(tauriArgs);
  stageStandaloneCliResource(tauriArgs);
}

function buildStandaloneCli(tauriArgs) {
  const target = readFlagValue(tauriArgs, '--target');
  if (target === UNIVERSAL_MACOS_TARGET) {
    for (const sourceTarget of UNIVERSAL_MACOS_SOURCE_TARGETS) {
      runRequired('build standalone sona-cli', 'cargo', [
        'build',
        '-p',
        'sona-cli',
        '--release',
        '--target',
        sourceTarget,
      ]);
    }
    return;
  }

  const cargoArgs = ['build', '-p', 'sona-cli', '--release'];
  if (target) {
    cargoArgs.push('--target', target);
  }
  runRequired('build standalone sona-cli', 'cargo', cargoArgs);
}

function stageStandaloneCliResource(tauriArgs) {
  const target = readFlagValue(tauriArgs, '--target');
  const setupArgs = [path.resolve(__dirname, 'setup-sona-cli-resource.js')];
  if (target) {
    setupArgs.push('--target', target);
  }

  runRequired('stage standalone sona-cli resource', process.execPath, setupArgs, {
    cwd: repoRoot,
    stdio: 'inherit',
  });
}

function readFlagValue(commandArgs, flagName) {
  for (let index = 0; index < commandArgs.length; index += 1) {
    const value = commandArgs[index];
    if (value === flagName) {
      return commandArgs[index + 1] ?? null;
    }
    if (value.startsWith(`${flagName}=`)) {
      return value.slice(flagName.length + 1);
    }
  }
  return null;
}

function runRequired(label, executable, commandArgs, options = {}) {
  const result = spawnSync(executable, commandArgs, {
    cwd: repoRoot,
    stdio: 'inherit',
    shell: process.platform === 'win32' && executable !== process.execPath,
    ...options,
  });

  if (result.error) {
    console.error(`Failed to ${label}:`, result.error);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
