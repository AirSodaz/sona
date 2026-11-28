#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const maximumAndroidVersionCode = 2100000000;
const supportedChannels = new Set(['stable', 'nightly']);
const supportedAbis = ['arm64-v8a', 'x86_64'];
const supportedSigningKinds = new Set(['debug', 'release']);

function parseVersionCode(value) {
  const normalized = String(value ?? '').trim();
  if (!/^\d+$/u.test(normalized)) {
    throw new Error(
      `Android update versionCode must be an integer from 1 to ${maximumAndroidVersionCode}`,
    );
  }
  const versionCode = Number(normalized);
  if (versionCode < 1 || versionCode > maximumAndroidVersionCode) {
    throw new Error(
      `Android update versionCode must be an integer from 1 to ${maximumAndroidVersionCode}`,
    );
  }
  return versionCode;
}

function sha256File(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

function findApkForAbi(artifactsDir, abi, signingKind) {
  const preferredNames = signingKind === 'release'
    ? [`app-${abi}-release.apk`, `app-${abi}-debug.apk`]
    : [`app-${abi}-debug.apk`];

  const queue = [artifactsDir];
  const matches = [];
  while (queue.length > 0) {
    const currentDir = queue.shift();
    for (const entry of fs.readdirSync(currentDir, { withFileTypes: true })) {
      const fullPath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        queue.push(fullPath);
      } else if (entry.isFile() && preferredNames.includes(entry.name)) {
        matches.push(fullPath);
      }
    }
  }

  for (const preferredName of preferredNames) {
    const found = matches.find((filePath) => path.basename(filePath) === preferredName);
    if (found) {
      return found;
    }
  }
  return null;
}

function buildAssetsFromArtifactsDir({ artifactsDir, downloadBase, signing }) {
  const normalizedBase = String(downloadBase ?? '').trim().replace(/\/+$/u, '');
  if (!normalizedBase.startsWith('https://')) {
    throw new Error('Android update downloadBase must start with https://');
  }
  const resolvedDir = path.resolve(artifactsDir);
  if (!fs.existsSync(resolvedDir)) {
    throw new Error(`Android artifacts directory does not exist: ${resolvedDir}`);
  }

  const assets = {};
  for (const abi of supportedAbis) {
    const apkPath = findApkForAbi(resolvedDir, abi, signing);
    if (!apkPath) {
      throw new Error(`Missing installable ${abi} APK under ${resolvedDir}`);
    }
    const stat = fs.statSync(apkPath);
    if (!stat.isFile() || stat.size <= 0) {
      throw new Error(`Invalid APK file size for ${apkPath}`);
    }
    assets[abi] = {
      url: `${normalizedBase}/${path.basename(apkPath)}`,
      sha256: sha256File(apkPath),
      sizeBytes: stat.size,
    };
  }
  return assets;
}

function validateAssets(assets) {
  if (assets === undefined) {
    return undefined;
  }
  if (!assets || typeof assets !== 'object' || Array.isArray(assets)) {
    throw new Error('Android update assets must be an object keyed by ABI');
  }
  const normalized = {};
  for (const [abi, asset] of Object.entries(assets)) {
    if (!supportedAbis.includes(abi)) {
      throw new Error(`Unsupported Android update asset ABI: ${abi}`);
    }
    if (!asset || typeof asset !== 'object') {
      throw new Error(`Invalid Android update asset entry for ${abi}`);
    }
    const url = String(asset.url ?? '').trim();
    if (!url.startsWith('https://')) {
      throw new Error(`Android update asset url for ${abi} must start with https://`);
    }
    const sha256 = String(asset.sha256 ?? '').trim().toLowerCase();
    if (!/^[0-9a-f]{64}$/u.test(sha256)) {
      throw new Error(`Android update asset sha256 for ${abi} must be 64 hex characters`);
    }
    const sizeBytes = Number(asset.sizeBytes);
    if (!Number.isSafeInteger(sizeBytes) || sizeBytes <= 0) {
      throw new Error(`Android update asset sizeBytes for ${abi} must be a positive integer`);
    }
    normalized[abi] = { url, sha256, sizeBytes };
  }
  return normalized;
}

function createAndroidUpdateManifest({
  channel,
  versionName,
  versionCode,
  pubDate,
  notes,
  signing,
  assets,
}) {
  const normalizedChannel = String(channel ?? '').trim();
  if (!supportedChannels.has(normalizedChannel)) {
    throw new Error('Android update channel must be stable or nightly');
  }
  const normalizedVersionName = String(versionName ?? '').trim();
  if (normalizedVersionName.length === 0 || normalizedVersionName.length > 100) {
    throw new Error('Android update versionName must contain 1 to 100 characters');
  }
  const manifest = {
    schemaVersion: 1,
    channel: normalizedChannel,
    versionName: normalizedVersionName,
    versionCode: parseVersionCode(versionCode),
  };

  if (pubDate !== undefined && String(pubDate).trim().length > 0) {
    manifest.pubDate = String(pubDate).trim();
  }
  if (notes !== undefined && String(notes).trim().length > 0) {
    manifest.notes = String(notes).trim().slice(0, 8192);
  }
  if (signing !== undefined && String(signing).trim().length > 0) {
    const normalizedSigning = String(signing).trim();
    if (!supportedSigningKinds.has(normalizedSigning)) {
      throw new Error('Android update signing must be debug or release');
    }
    manifest.signing = normalizedSigning;
  }
  const validatedAssets = validateAssets(assets);
  if (validatedAssets && Object.keys(validatedAssets).length > 0) {
    manifest.assets = validatedAssets;
  }
  return manifest;
}

function validateReleaseTagVersion(releaseTag, projectVersion) {
  const normalizedTag = String(releaseTag ?? '').trim();
  const normalizedProjectVersion = String(projectVersion ?? '').trim();
  if (normalizedProjectVersion.length === 0) {
    throw new Error('Project version is required');
  }
  const expectedTag = `v${normalizedProjectVersion}`;
  if (normalizedTag !== expectedTag) {
    throw new Error(`Release tag ${normalizedTag || '<empty>'} must equal ${expectedTag}`);
  }
  return normalizedProjectVersion;
}

function optionValue(args, name) {
  const index = args.indexOf(name);
  if (index === -1 || !args[index + 1] || args[index + 1].startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return args[index + 1];
}

function optionalOptionValue(args, name) {
  const index = args.indexOf(name);
  if (index === -1) {
    return undefined;
  }
  if (!args[index + 1] || args[index + 1].startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return args[index + 1];
}

function runCli(args = process.argv.slice(2)) {
  const [command, ...commandArgs] = args;
  if (command === 'validate-release') {
    const version = validateReleaseTagVersion(
      optionValue(commandArgs, '--tag'),
      optionValue(commandArgs, '--project-version'),
    );
    process.stdout.write(`${version}\n`);
    return;
  }
  if (command !== 'generate') {
    throw new Error('Expected generate or validate-release command');
  }
  const outputPath = path.resolve(optionValue(commandArgs, '--output'));
  const artifactsDir = optionalOptionValue(commandArgs, '--artifacts-dir');
  const downloadBase = optionalOptionValue(commandArgs, '--download-base');
  const signing = optionalOptionValue(commandArgs, '--signing');
  const pubDate = optionalOptionValue(commandArgs, '--pub-date');
  const notesArg = optionalOptionValue(commandArgs, '--notes');
  const notesFile = optionalOptionValue(commandArgs, '--notes-file');
  const notes = notesFile ? fs.readFileSync(path.resolve(notesFile), 'utf8') : notesArg;
  const assets = artifactsDir
    ? buildAssetsFromArtifactsDir({
        artifactsDir,
        downloadBase,
        signing: signing ?? 'debug',
      })
    : undefined;

  const manifest = createAndroidUpdateManifest({
    channel: optionValue(commandArgs, '--channel'),
    versionName: optionValue(commandArgs, '--version-name'),
    versionCode: optionValue(commandArgs, '--version-code'),
    pubDate,
    notes,
    signing: artifactsDir ? (signing ?? 'debug') : signing,
    assets,
  });
  fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
}

const isDirectExecution = process.argv[1]
  && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirectExecution) {
  try {
    runCli();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

export {
  buildAssetsFromArtifactsDir,
  createAndroidUpdateManifest,
  runCli,
  validateReleaseTagVersion,
};
