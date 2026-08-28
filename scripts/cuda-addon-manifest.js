#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const supportedPlatforms = ['windows-x86_64', 'linux-x86_64'];

function validateAddonVersion(version) {
  const normalized = String(version ?? '').trim();
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(normalized)) {
    throw new Error(`CUDA addon version must be a valid semantic version, got "${normalized}"`);
  }
  return normalized;
}

function validateReleaseTag(tag, expectedVersion) {
  const normalizedTag = String(tag ?? '').trim();
  const normalizedExpected = expectedVersion ? String(expectedVersion).trim() : null;
  const match = /^cuda-addon-v(?<version>\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/u.exec(normalizedTag);
  if (!match || !match.groups?.version) {
    throw new Error(`CUDA addon release tag must match "cuda-addon-v<semver>", got "${normalizedTag}"`);
  }
  const tagVersion = match.groups.version;
  if (normalizedExpected && tagVersion !== normalizedExpected) {
    throw new Error(`Release tag version "${tagVersion}" does not match expected version "${normalizedExpected}"`);
  }
  return tagVersion;
}

function computeSha256(filePath) {
  const content = fs.readFileSync(filePath);
  return crypto.createHash('sha256').update(content).digest('hex');
}

function createCudaAddonManifest({
  addonVersion,
  cudaVersion = '12.4',
  repo = 'AirSodaz/sona',
  artifactsDir = null,
  platformArtifacts = {},
  publishedAt = new Date().toISOString(),
}) {
  const version = validateAddonVersion(addonVersion);
  const tag = `cuda-addon-v${version}`;
  const downloadBase = `https://github.com/${repo}/releases/download/${tag}`;

  const platforms = {};

  for (const platform of supportedPlatforms) {
    let artifactInfo = platformArtifacts[platform];

    if (!artifactInfo && artifactsDir && fs.existsSync(artifactsDir)) {
      const filename = `sona-cuda-addon-v${version}-${platform === 'windows-x86_64' ? 'windows-x64' : 'linux-x64'}.tar.gz`;
      const filePath = path.join(artifactsDir, filename);

      if (fs.existsSync(filePath)) {
        const stats = fs.statSync(filePath);
        artifactInfo = {
          filename,
          sha256: computeSha256(filePath),
          sizeBytes: stats.size,
        };
      }
    }

    if (artifactInfo) {
      platforms[platform] = {
        filename: artifactInfo.filename,
        url: `${downloadBase}/${artifactInfo.filename}`,
        sha256: artifactInfo.sha256,
        sizeBytes: artifactInfo.sizeBytes,
      };
    }
  }

  return {
    schemaVersion: 1,
    addonVersion: version,
    cudaVersion: String(cudaVersion).trim(),
    publishedAt,
    platforms,
  };
}

function parseCliArgs(args) {
  const options = {};
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      const nextArg = args[i + 1];
      if (nextArg && !nextArg.startsWith('--')) {
        options[key] = nextArg;
        i += 1;
      } else {
        options[key] = true;
      }
    }
  }
  return options;
}

function runCli() {
  const args = process.argv.slice(2);
  const command = args[0];
  const options = parseCliArgs(args.slice(1));

  if (command === 'generate') {
    const manifest = createCudaAddonManifest({
      addonVersion: options.version || options['addon-version'],
      cudaVersion: options['cuda-version'] || '12.4',
      repo: options.repo || 'AirSodaz/sona',
      artifactsDir: options['artifacts-dir'] ? path.resolve(options['artifacts-dir']) : null,
    });
    const outputContent = JSON.stringify(manifest, null, 2);
    if (options.output) {
      const outputPath = path.resolve(options.output);
      fs.mkdirSync(path.dirname(outputPath), { recursive: true });
      fs.writeFileSync(outputPath, `${outputContent}\n`, 'utf8');
      console.log(`Wrote CUDA addon manifest to ${outputPath}`);
    } else {
      console.log(outputContent);
    }
    return;
  }

  if (command === 'validate-tag') {
    const version = validateReleaseTag(options.tag, options.version);
    console.log(`Valid tag for CUDA addon version: ${version}`);
    return;
  }

  console.error(`Unknown command "${command}". Supported: generate, validate-tag`);
  process.exit(1);
}

export {
  createCudaAddonManifest,
  validateAddonVersion,
  validateReleaseTag,
  computeSha256,
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runCli();
}
