import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const args = process.argv.slice(2);

function main() {
  const repoRoot = path.resolve(readFlagValue(args, '--repo-root') ?? path.resolve(__dirname, '../../..'));
  const target = resolveBuildTarget(args);
  const configPath = path.resolve(
    repoRoot,
    readFlagValue(args, '--config') ?? path.join('target', 'desktop-bundle', target, 'tauri.bundle.conf.json'),
  );
  const bundleRoots = resolveBundleRoots(repoRoot, target, args);
  if (bundleRoots.length === 0) {
    throw new Error('No build bundle output was found under target/**/release/bundle.');
  }

  verifyInstallerArtifacts(bundleRoots, target);
  const config = verifyTauriBundleConfig(configPath, target);
  verifyStagedSidecar(config, configPath, target, 'ffmpeg');
  verifyStagedSidecar(config, configPath, target, 'sona-cli');
  verifyStagedRuntimeLibraries(config, configPath, target);
  verifyCanonicalAppBundle(bundleRoots, target);

  console.log(`[bundle] Verified packaged artifacts for ${target}`);
}

function resolveBuildTarget(commandArgs) {
  const explicitTarget = readFlagValue(commandArgs, '--target');
  if (explicitTarget) return explicitTarget;
  if (process.platform === 'win32') return process.arch === 'arm64' ? 'aarch64-pc-windows-msvc' : 'x86_64-pc-windows-msvc';
  if (process.platform === 'darwin') return process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  return process.arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
}

function readFlagValue(commandArgs, flagName) {
  for (let index = 0; index < commandArgs.length; index += 1) {
    const value = commandArgs[index];
    if (value === flagName) return commandArgs[index + 1] ?? null;
    if (value.startsWith(`${flagName}=`)) return value.slice(flagName.length + 1);
  }
  return null;
}

function resolveBundleRoots(repoRoot, target, commandArgs) {
  const explicitBundleRoot = readFlagValue(commandArgs, '--bundle-root');
  if (explicitBundleRoot) {
    return [path.resolve(repoRoot, explicitBundleRoot)].filter((directoryPath) => fs.existsSync(directoryPath));
  }
  const releaseDir = readFlagValue(commandArgs, '--target')
    ? path.resolve(repoRoot, 'target', target, 'release')
    : path.resolve(repoRoot, 'target', 'release');
  return [path.join(releaseDir, 'bundle')].filter((directoryPath) => fs.existsSync(directoryPath));
}

function verifyInstallerArtifacts(bundleRoots, target) {
  const installers = bundleRoots.flatMap((bundleRoot) => walkFiles(bundleRoot))
    .filter((filePath) => isInstallerArtifact(filePath, target));
  if (installers.length === 0) {
    throw new Error(`No installer artifact was found for ${target}.`);
  }
  console.log(`[bundle] Verified installer artifacts: ${installers.join(', ')}`);
}

function isInstallerArtifact(filePath, target) {
  const name = path.basename(filePath);
  if (target.includes('windows')) return /(?:setup\.exe|\.msi)$/iu.test(name);
  if (target.includes('apple')) return /\.dmg$/iu.test(name);
  return /\.(AppImage|deb|rpm)$/iu.test(name);
}

function verifyTauriBundleConfig(configPath, target) {
  if (!fs.existsSync(configPath)) {
    throw new Error(`Generated Tauri bundle configuration was not found: ${configPath}`);
  }
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const externalBins = config.bundle?.externalBin;
  if (!Array.isArray(externalBins)) {
    throw new Error('Generated Tauri configuration must declare bundle.externalBin.');
  }
  for (const name of ['sona-cli', 'ffmpeg']) {
    if (!externalBins.some((entry) => path.basename(normalizeConfigPath(entry)) === name)) {
      throw new Error(`Generated Tauri configuration must declare ${name} through bundle.externalBin.`);
    }
  }
  if (hasLegacyResourcePath(JSON.stringify(config.bundle))) {
    throw new Error('Generated Tauri configuration must reject legacy resources/cli and resources/shared_libs directories.');
  }
  verifyRuntimePlacement(config, target);
  return config;
}

function verifyRuntimePlacement(config, target) {
  if (target.includes('windows')) {
    const resources = config.bundle.resources;
    if (
      !resources
      || Array.isArray(resources)
      || !Object.entries(resources).some(([source, destination]) => {
        return normalizeConfigPath(source).endsWith('/runtime-libs/*') && destination === '';
      })
    ) {
      throw new Error('Generated Tauri configuration must map staged DLLs to the Windows resource root.');
    }
    return;
  }
  if (target.includes('apple')) {
    verifyRuntimeFileMap(config.bundle.macOS?.files, 'Frameworks', 'macOS');
    return;
  }
  if (target.includes('linux')) {
    for (const format of ['deb', 'rpm', 'appimage']) {
      verifyRuntimeFileMap(config.bundle.linux?.[format]?.files, 'usr/lib/sona', `linux.${format}`);
    }
    return;
  }
  throw new Error(`Unsupported desktop bundle target: ${target}`);
}

function verifyRuntimeFileMap(files, destinationDir, label) {
  if (!files || !Object.entries(files).some(([destination, source]) => {
    return normalizeConfigPath(destination).startsWith(`${destinationDir}/`)
      && normalizeConfigPath(source).includes('/runtime-libs/');
  })) {
    throw new Error(`Generated Tauri configuration must map runtime libraries through bundle.${label}.files.`);
  }
}

function verifyStagedSidecar(config, configPath, target, name) {
  const sidecarBase = config.bundle.externalBin
    .find((entry) => path.basename(normalizeConfigPath(entry)) === name);
  const basePath = resolveConfigPath(configPath, sidecarBase);
  const sidecarPath = `${basePath}-${target}${target.includes('windows') ? '.exe' : ''}`;
  if (!fs.existsSync(sidecarPath)) {
    throw new Error(`Missing staged ${name} sidecar for ${target}: ${sidecarPath}`);
  }
  console.log(`[bundle] Verified staged ${name} sidecar: ${sidecarPath}`);
}

function verifyStagedRuntimeLibraries(config, configPath, target) {
  const runtimeFiles = configuredRuntimeFiles(config, configPath, target);
  const names = runtimeFiles.filter((filePath) => fs.existsSync(filePath)).map((filePath) => path.basename(filePath));
  const missingLibraries = requiredRuntimeLibraries(target).filter((library) => {
    return typeof library === 'string' ? !names.includes(library) : !names.some((name) => library.test(name));
  });
  if (missingLibraries.length > 0) {
    throw new Error(`Missing staged runtime libraries for ${target}: ${missingLibraries.map(String).join(', ')}. Found: ${names.join(', ')}`);
  }
  console.log(`[bundle] Verified staged runtime libraries: ${names.join(', ')}`);
}

function configuredRuntimeFiles(config, configPath, target) {
  if (target.includes('windows')) {
    const source = Object.keys(config.bundle.resources)
      .find((entry) => normalizeConfigPath(entry).endsWith('/runtime-libs/*'));
    const runtimeLibDir = resolveConfigPath(configPath, source.slice(0, -2));
    return fs.readdirSync(runtimeLibDir).map((name) => path.join(runtimeLibDir, name));
  }
  if (target.includes('apple')) return Object.values(config.bundle.macOS.files).map((source) => resolveConfigPath(configPath, source));
  return Object.values(config.bundle.linux.deb.files).map((source) => resolveConfigPath(configPath, source));
}

function verifyCanonicalAppBundle(bundleRoots, target) {
  const appRoot = findCanonicalAppRoot(bundleRoots, target);
  if (!appRoot) {
    throw new Error(`No canonical application bundle was found for ${target}.`);
  }
  const layout = nativeAppLayout(appRoot, target);
  const missingSidecars = layout.sidecars.filter((filePath) => !fs.existsSync(filePath));
  if (missingSidecars.length > 0) {
    throw new Error(`Canonical application bundle is missing sidecars for ${target}: ${missingSidecars.join(', ')}`);
  }
  const runtimeNames = fs.existsSync(layout.runtimeDir) ? fs.readdirSync(layout.runtimeDir) : [];
  const missingLibraries = requiredRuntimeLibraries(target).filter((library) => {
    return typeof library === 'string' ? !runtimeNames.includes(library) : !runtimeNames.some((name) => library.test(name));
  });
  if (missingLibraries.length > 0) {
    throw new Error(`Canonical application bundle is missing runtime libraries for ${target}: ${missingLibraries.map(String).join(', ')}`);
  }
  console.log(`[bundle] Verified canonical app bundle: ${appRoot}`);
}

function findCanonicalAppRoot(bundleRoots, target) {
  if (target.includes('windows')) {
    return bundleRoots
      .map((bundleRoot) => path.dirname(bundleRoot))
      .find((releaseDir) => fs.existsSync(path.join(releaseDir, 'sona.exe'))) ?? null;
  }
  const files = bundleRoots.flatMap((bundleRoot) => walkFiles(bundleRoot));
  if (target.includes('apple')) {
    const executable = files.find((filePath) => normalizeConfigPath(filePath).endsWith('.app/Contents/MacOS/Sona'));
    return executable ? path.dirname(path.dirname(path.dirname(executable))) : null;
  }
  const executable = files.find((filePath) => normalizeConfigPath(filePath).endsWith('/usr/bin/sona'));
  return executable ? path.dirname(path.dirname(path.dirname(executable))) : null;
}

function nativeAppLayout(appRoot, target) {
  if (target.includes('windows')) {
    return {
      sidecars: [path.join(appRoot, 'sona-cli.exe'), path.join(appRoot, 'ffmpeg.exe')],
      runtimeDir: appRoot,
    };
  }
  if (target.includes('apple')) {
    const contents = path.join(appRoot, 'Contents');
    return {
      sidecars: [path.join(contents, 'MacOS', 'sona-cli'), path.join(contents, 'MacOS', 'ffmpeg')],
      runtimeDir: path.join(contents, 'Frameworks'),
    };
  }
  const root = path.join(appRoot, 'usr');
  return {
    sidecars: [path.join(root, 'bin', 'sona-cli'), path.join(root, 'bin', 'ffmpeg')],
    runtimeDir: path.join(root, 'lib', 'sona'),
  };
}

function requiredRuntimeLibraries(target) {
  if (target.includes('windows')) return ['sherpa-onnx-c-api.dll', 'onnxruntime.dll'];
  if (target.includes('apple')) return ['libsherpa-onnx-c-api.dylib', 'libonnxruntime.dylib'];
  return [/^libsherpa-onnx-c-api\.so/u, /^libonnxruntime\.so/u];
}

function hasLegacyResourcePath(value) {
  const normalized = normalizeConfigPath(value);
  return normalized.includes('resources/cli') || normalized.includes('resources/shared_libs');
}

function normalizeConfigPath(value) {
  return String(value).replaceAll('\\', '/').replace(/^\.\//u, '');
}

function resolveConfigPath(configPath, configValue) {
  return path.isAbsolute(configValue) ? configValue : path.resolve(path.dirname(configPath), configValue);
}

function walkFiles(directoryPath) {
  const entries = fs.readdirSync(directoryPath, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const resolvedPath = path.resolve(directoryPath, entry.name);
    return entry.isDirectory() ? walkFiles(resolvedPath) : [resolvedPath];
  });
}

main();
