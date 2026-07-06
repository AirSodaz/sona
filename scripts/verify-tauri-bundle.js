import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const args = process.argv.slice(2);

function main() {
  const repoRoot = path.resolve(readFlagValue(args, '--repo-root') ?? path.resolve(__dirname, '..'));
  const target = resolveBuildTarget(args);
  const bundleRoots = resolveBundleRoots(repoRoot, target, args);

  if (bundleRoots.length === 0) {
    throw new Error('No build bundle output was found under target/**/release/bundle.');
  }

  const bundledFiles = bundleRoots.flatMap((bundleRoot) => walkFiles(bundleRoot));
  const opaqueArtifacts = bundledFiles.filter((filePath) =>
    /\.(AppImage|deb|dmg|exe|msi|rpm)$/iu.test(filePath)
  );

  if (opaqueArtifacts.length === 0) {
    throw new Error('No packaged installer or bundle artifact was found under target/**/release/bundle.');
  }

  verifyTauriBundleConfig(repoRoot);
  verifyFfmpegSidecar(repoRoot, target);
  verifyStandaloneCliResource(repoRoot, target);
  verifySharedLibraries(repoRoot, target);

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

function resolveBundleRoots(repoRoot, target, commandArgs) {
  const explicitBundleRoot = readFlagValue(commandArgs, '--bundle-root');
  if (explicitBundleRoot) {
    return [path.resolve(repoRoot, explicitBundleRoot)].filter((directoryPath) =>
      fs.existsSync(directoryPath)
    );
  }

  const candidateDirectories = [
    path.resolve(repoRoot, 'target', 'release', 'bundle'),
    path.resolve(repoRoot, 'target', target, 'release', 'bundle'),
  ];

  return [...new Set(candidateDirectories)].filter((directoryPath) =>
    fs.existsSync(directoryPath)
  );
}

function verifyTauriBundleConfig(repoRoot) {
  const configPath = path.resolve(repoRoot, 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const externalBins = config.bundle?.externalBin ?? [];
  if (!externalBins.some((entry) => normalizeConfigPath(entry) === 'binaries/ffmpeg')) {
    throw new Error('src-tauri/tauri.conf.json must include bundle.externalBin entry "binaries/ffmpeg".');
  }

  const resources = config.bundle?.resources ?? [];
  const resourceEntries = Array.isArray(resources)
    ? resources
    : Object.entries(resources).flatMap(([source, target]) => [source, target]);

  if (!resourceEntries.some((entry) => normalizeConfigPath(entry).includes('resources/shared_libs'))) {
    throw new Error('src-tauri/tauri.conf.json must include bundle.resources entry for resources/shared_libs.');
  }
  if (!resourceEntries.some((entry) => normalizeConfigPath(entry).includes('resources/cli'))) {
    throw new Error('src-tauri/tauri.conf.json must include bundle.resources entry for resources/cli.');
  }
}

function verifyFfmpegSidecar(repoRoot, target) {
  const sidecarPath = path.resolve(
    repoRoot,
    'src-tauri',
    'binaries',
    `ffmpeg-${target}${target.includes('windows') ? '.exe' : ''}`,
  );

  if (!fs.existsSync(sidecarPath)) {
    throw new Error(`Missing packaged ffmpeg sidecar for ${target}: ${path.relative(repoRoot, sidecarPath)}`);
  }

  console.log(`[bundle] Verified packaged ffmpeg sidecar: ${path.relative(repoRoot, sidecarPath)}`);
}

function verifyStandaloneCliResource(repoRoot, target) {
  if (target === 'universal-apple-darwin') {
    console.log('[bundle] Skipping standalone CLI resource check for universal Apple bundle');
    return;
  }

  const binaryName = target.includes('windows') ? 'sona-cli.exe' : 'sona-cli';
  const cliPath = path.resolve(repoRoot, 'src-tauri', 'resources', 'cli', binaryName);

  if (!fs.existsSync(cliPath)) {
    throw new Error(`Missing standalone CLI resource for ${target}: ${path.relative(repoRoot, cliPath)}`);
  }

  console.log(`[bundle] Verified standalone CLI resource: ${path.relative(repoRoot, cliPath)}`);
}

function verifySharedLibraries(repoRoot, target) {
  const sharedLibsDir = path.resolve(repoRoot, 'src-tauri', 'resources', 'shared_libs');
  const entries = fs.existsSync(sharedLibsDir) ? fs.readdirSync(sharedLibsDir) : [];
  const missingLibraries = requiredSharedLibraries(target).filter((library) =>
    typeof library === 'string'
      ? !entries.includes(library)
      : !entries.some((entry) => library.test(entry))
  );

  if (missingLibraries.length > 0) {
    throw new Error(
      `Missing shared libraries for ${target}: ${missingLibraries.map(String).join(', ')}. Found: ${entries.join(', ')}`
    );
  }

  console.log(`[bundle] Verified shared libraries: ${entries.join(', ')}`);
}

function requiredSharedLibraries(target) {
  if (target.includes('windows')) {
    return ['sherpa-onnx-c-api.dll', 'onnxruntime.dll'];
  }
  if (target.includes('apple')) {
    return ['libsherpa-onnx-c-api.dylib', 'libonnxruntime.dylib'];
  }
  return [/^libsherpa-onnx-c-api\.so/, /^libonnxruntime\.so/];
}

function normalizeConfigPath(value) {
  return String(value).replaceAll('\\', '/').replace(/^\.\//u, '');
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
