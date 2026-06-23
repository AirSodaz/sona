// scripts/generate-nightly-updater-json.js
// Generates updater-nightly.json from the nightly build artifacts.
// Run after all platform builds complete, in the directory containing all artifacts.

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf-8'));
const { version } = pkg;

// Extensions that are architecture-agnostic: the platform key is derived from
// the arch token in the filename (aarch64/arm64 → aarch64, x86_64/amd64/x64 → x86_64).
// Generic .msi/.exe/.dmg/.app.tar.gz deliberately map to null so that a missing
// arch token is rejected rather than silently mislabelled (see getPlatformKey).
const repo = process.env.GITHUB_REPOSITORY || 'AirSodaz/sona';

function archFromFilename(filename) {
  if (filename.includes('aarch64') || filename.includes('arm64')) return 'aarch64';
  if (filename.includes('x86_64') || filename.includes('amd64') || filename.includes('x64')) return 'x86_64';
  if (filename.includes('universal')) return 'universal';
  return null;
}

export function getPlatformKey(filename) {
  if (filename.endsWith('.app.tar.gz')) {
    const arch = archFromFilename(filename);
    if (arch === 'aarch64') return 'darwin-aarch64';
    if (arch === 'x86_64') return 'darwin-x86_64';
    return null;
  }

  if (filename.endsWith('.exe')) {
    if (!filename.includes('-setup')) return null;
    const arch = archFromFilename(filename);
    if (arch === 'aarch64') return 'windows-aarch64';
    if (arch === 'x86_64') return 'windows-x86_64';
    return null;
  }

  if (filename.endsWith('.AppImage')) {
    const arch = archFromFilename(filename);
    if (arch === 'x86_64') return 'linux-x86_64';
    return null;
  }

  if (filename.endsWith('.zip')) {
    if (filename.includes('darwin') || filename.includes('macos') || filename.includes('apple')) {
      const arch = archFromFilename(filename);
      if (arch === 'aarch64' || arch === 'x86_64') return `darwin-${arch}`;
      if (arch === 'universal') return 'darwin-aarch64';
    }
    if (filename.includes('linux')) return 'linux-x86_64';
    if (filename.includes('windows')) {
      const arch = archFromFilename(filename);
      if (arch === 'aarch64') return 'windows-aarch64';
      if (arch === 'x86_64') return 'windows-x86_64';
    }
  }

  return null;
}

function getSignatureForFile(file, sigFiles) {
  let sig = sigFiles[file] || sigFiles[file.replace(/\.(msi|exe|dmg|AppImage|deb|rpm|tar\.gz)$/, '')] || '';
  if (!sig && (file.endsWith('.dmg') || file.endsWith('.app.tar.gz'))) {
    sig = sigFiles[file.replace(/\.(dmg|app\.tar\.gz)$/, '')] || '';
  }
  return sig;
}

export function generateUpdaterJson(artifactDir) {
  const updaterData = {
    version,
    notes: `Nightly build from ${new Date().toISOString().split('T')[0]}`,
    pub_date: new Date().toISOString(),
    platforms: {},
  };

  const sigFiles = {};

  const files = fs.readdirSync(artifactDir);
  for (const file of files) {
    if (file.endsWith('.sig')) {
      const baseName = file.replace('.sig', '');
      sigFiles[baseName] = fs.readFileSync(path.join(artifactDir, file), 'utf-8').trim();
    }
  }

  for (const file of files) {
    if (file.endsWith('.sig') || file === 'updater-nightly.json') continue;

    const platformKey = getPlatformKey(file);
    if (!platformKey) continue;

    const sig = getSignatureForFile(file, sigFiles);

    if (!sig) {
      throw new Error(`No signature found for ${file}`);
    }

    if (updaterData.platforms[platformKey]) {
      throw new Error(`Duplicate platform key "${platformKey}" for file "${file}"`);
    }

    updaterData.platforms[platformKey] = {
      signature: sig,
      url: `https://github.com/${repo}/releases/download/nightly/${encodeURIComponent(file)}`,
    };
  }

  return updaterData;
}

function main() {
  const artifactDir = process.argv[2] || 'nightly-artifacts';
  const outputPath = process.argv[3] || 'nightly-artifacts/updater-nightly.json';

  const updaterData = generateUpdaterJson(artifactDir);
  fs.writeFileSync(outputPath, JSON.stringify(updaterData, null, 2));
  console.log(`Generated ${outputPath} with ${Object.keys(updaterData.platforms).length} platforms`);
}

const isMainModule = process.argv[1] && (
  process.argv[1] === fileURLToPath(import.meta.url) ||
  path.basename(process.argv[1]) === 'generate-nightly-updater-json.js'
);
if (isMainModule) {
  main();
}
