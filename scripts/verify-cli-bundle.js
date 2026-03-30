import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const srcTauriDir = path.resolve(repoRoot, 'src-tauri');
const args = process.argv.slice(2);

function main() {
  const target = resolveBuildTarget(args);
  const bundleRoots = resolveBundleRoots(target);

  if (bundleRoots.length === 0) {
    throw new Error('No build bundle output was found under src-tauri/target/**/release/bundle.');
  }

  const bundledFiles = bundleRoots.flatMap((bundleRoot) => walkFiles(bundleRoot));
  const opaqueArtifacts = bundledFiles.filter((filePath) =>
    /\.(AppImage|deb|dmg|exe|msi|rpm)$/iu.test(filePath)
  );

  if (opaqueArtifacts.length === 0) {
    throw new Error('No packaged installer or bundle artifact was found under src-tauri/target/**/release/bundle.');
  }

  console.log(`[bundle] Verified packaged artifacts for ${target}`);
  console.log(`[bundle] Artifacts: ${opaqueArtifacts.map((filePath) => path.relative(repoRoot, filePath)).join(', ')}`);
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
    return 'aarch64-apple-darwin';
  }
  return 'x86_64-unknown-linux-gnu';
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

function resolveBundleRoots(target) {
  const candidateDirectories = [
    path.resolve(srcTauriDir, 'target', 'release', 'bundle'),
    path.resolve(srcTauriDir, 'target', target, 'release', 'bundle'),
  ];

  return [...new Set(candidateDirectories)].filter((directoryPath) =>
    fs.existsSync(directoryPath)
  );
}

function walkFiles(directoryPath) {
  const entries = fs.readdirSync(directoryPath, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const resolvedPath = path.resolve(directoryPath, entry.name);
    if (entry.isDirectory()) {
      return walkFiles(resolvedPath);
    }
    return [resolvedPath];
  });
}

main();
