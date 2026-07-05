import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const args = process.argv.slice(2);

function main() {
  const repoRoot = path.resolve(readFlagValue(args, '--repo-root') ?? path.resolve(__dirname, '..'));
  const targetDir = path.resolve(readFlagValue(args, '--target-dir') ?? path.join(repoRoot, 'target'));
  const target = resolveBuildTarget(args);
  const skipBuild = args.includes('--skip-build');

  if (target === 'universal-apple-darwin') {
    prepareUniversalMacOSCli({ repoRoot, targetDir, skipBuild });
    return;
  }

  if (!skipBuild) {
    runCargoBuild(repoRoot, target);
  }

  copyCliSidecar({
    sourcePath: path.join(targetDir, target, 'release', cliExecutableName(target)),
    targetPath: path.join(repoRoot, 'src-tauri', 'binaries', targetSpecificCliName(target)),
    target,
  });
}

function prepareUniversalMacOSCli({ repoRoot, targetDir, skipBuild }) {
  const universalTarget = 'universal-apple-darwin';
  const universalPath = path.join(
    targetDir,
    universalTarget,
    'release',
    cliExecutableName(universalTarget),
  );
  const targetPath = path.join(
    repoRoot,
    'src-tauri',
    'binaries',
    targetSpecificCliName(universalTarget),
  );

  if (skipBuild && fs.existsSync(universalPath)) {
    copyCliSidecar({ sourcePath: universalPath, targetPath, target: universalTarget });
    return;
  }

  const archTargets = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];
  for (const target of archTargets) {
    if (!skipBuild) {
      runCargoBuild(repoRoot, target);
    }
  }

  const archBinaries = archTargets.map((target) =>
    path.join(targetDir, target, 'release', cliExecutableName(target)),
  );
  for (const binary of archBinaries) {
    if (!fs.existsSync(binary)) {
      throw new Error(`Missing architecture-specific CLI binary for universal build: ${binary}`);
    }
  }

  fs.mkdirSync(path.dirname(universalPath), { recursive: true });
  const lipoResult = spawnSync('lipo', ['-create', ...archBinaries, '-output', universalPath], {
    cwd: repoRoot,
    stdio: 'inherit',
  });
  if (lipoResult.status !== 0) {
    throw new Error('Failed to create universal sona-cli binary with lipo.');
  }

  copyCliSidecar({ sourcePath: universalPath, targetPath, target: universalTarget });
}

function runCargoBuild(repoRoot, target) {
  const result = spawnSync(
    'cargo',
    ['build', '-p', 'sona-cli', '--release', '--target', target],
    {
      cwd: repoRoot,
      stdio: 'inherit',
    },
  );

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`cargo build -p sona-cli --release --target ${target} failed.`);
  }
}

function copyCliSidecar({ sourcePath, targetPath, target }) {
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Missing built sona-cli binary: ${sourcePath}`);
  }

  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.copyFileSync(sourcePath, targetPath);
  if (!isWindowsTarget(target)) {
    fs.chmodSync(targetPath, 0o755);
  }

  console.log(`[cli-sidecar] Copied ${path.basename(sourcePath)} to ${targetPath}`);
}

function resolveBuildTarget(commandArgs) {
  const explicitTarget = readFlagValue(commandArgs, '--target');
  if (explicitTarget) {
    return explicitTarget;
  }

  if (process.platform === 'win32') {
    return 'x86_64-pc-windows-msvc';
  }
  if (process.platform === 'darwin') {
    return os.arch() === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  }
  return os.arch() === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
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

function targetSpecificCliName(target) {
  return `sona-cli-${target}${cliExtension(target)}`;
}

function cliExecutableName(target) {
  return `sona-cli${cliExtension(target)}`;
}

function cliExtension(target) {
  return isWindowsTarget(target) ? '.exe' : '';
}

function isWindowsTarget(target) {
  return target.includes('windows');
}

main();
