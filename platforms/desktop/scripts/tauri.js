import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';
import { resolveHostTarget } from './prepare-desktop-bundle.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../../..');
const tauriBinary = process.env.SONA_TAURI_BINARY ?? path.resolve(
  repoRoot,
  'platforms',
  'desktop',
  'frontend',
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'tauri.cmd' : 'tauri'
);

const desktopTauriConfig = path.join(repoRoot, 'platforms', 'desktop', 'tauri.conf.json');
const args = process.argv.slice(2);
let tauriArgs = withDesktopConfig(args);
let tauriEnvironment = process.env;
const command = tauriArgs[0];

if (command === 'build' || command === 'bundle') {
  const preparedBundle = prepareDesktopBundle(tauriArgs);
  tauriArgs = preparedBundle.args;
  tauriEnvironment = preparedBundle.environment;
}

const tauriResult = spawnSync(tauriBinary, tauriArgs, {
  cwd: repoRoot,
  stdio: 'inherit',
  shell: process.platform === 'win32',
  env: tauriEnvironment,
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

function prepareDesktopBundle(commandArgs) {
  const target = readFlagValue(commandArgs, '--target') ?? resolveHostTarget();
  const baseConfig = readFlagValue(commandArgs, '--config') ?? desktopTauriConfig;
  runRequired(
    'prepare desktop bundle',
    process.execPath,
    [
      process.env.SONA_DESKTOP_BUNDLE_PREPARER ?? path.resolve(__dirname, 'prepare-desktop-bundle.js'),
      '--repo-root',
      repoRoot,
      '--target',
      target,
      '--config',
      baseConfig,
    ],
    {
      cwd: repoRoot,
      stdio: 'inherit',
    },
  );

  const bundleConfig = path.join(
    repoRoot,
    'target',
    'desktop-bundle',
    target,
    'tauri.bundle.conf.json',
  );
  const environment = target.includes('apple')
    ? { ...process.env, SHERPA_ONNX_LIB_DIR: path.join(repoRoot, 'target', 'desktop-bundle', target, 'runtime-libs') }
    : process.env;
  return {
    args: withBundleConfig(commandArgs, bundleConfig),
    environment,
  };
}

function withBundleConfig(commandArgs, bundleConfig) {
  const [command, ...rest] = commandArgs;
  const retainedArgs = [];
  for (let index = 0; index < rest.length; index += 1) {
    const argument = rest[index];
    if (argument === '--config') {
      index += 1;
      continue;
    }
    if (argument.startsWith('--config=')) {
      continue;
    }
    retainedArgs.push(argument);
  }
  return [command, '--config', bundleConfig, ...retainedArgs];
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
