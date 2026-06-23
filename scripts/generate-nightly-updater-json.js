// scripts/generate-nightly-updater-json.js
// Generates updater-nightly.json from the nightly build artifacts.
// Run after all platform builds complete, in the directory containing all artifacts.

const fs = require('fs');
const path = require('path');
const { version } = require('../package.json');

const artifactDir = process.argv[2] || 'nightly-artifacts';
const outputPath = process.argv[3] || 'nightly-artifacts/updater-nightly.json';

const platformMap = {
  // Windows
  '.msi': { identifier: 'windows-x86_64', arch: 'x86_64' },
  '.x64.msi': { identifier: 'windows-x86_64', arch: 'x86_64' },
  '.arm64.msi': { identifier: 'windows-aarch64', arch: 'aarch64' },
  '.exe': { identifier: 'windows-x86_64', arch: 'x86_64' },
  // macOS
  '.dmg': { identifier: 'darwin-aarch64', arch: 'aarch64' },
  '.app.tar.gz': { identifier: 'darwin-aarch64', arch: 'aarch64' },
  '.app.tar.gz.sig': { identifier: 'darwin-aarch64', arch: 'aarch64' },
  // Linux
  '.AppImage': { identifier: 'linux-x86_64', arch: 'x86_64' },
  '.deb': { identifier: 'linux-x86_64', arch: 'x86_64' },
  '.rpm': { identifier: 'linux-x86_64', arch: 'x86_64' },
};

function getPlatformKey(filename) {
  for (const [ext, info] of Object.entries(platformMap)) {
    if (filename.endsWith(ext)) return info.identifier;
  }
  // Try to detect from the parent directory name (artifact zip name)
  if (filename.includes('darwin') || filename.includes('macos') || filename.includes('apple')) {
    if (filename.includes('aarch64') || filename.includes('arm64')) return 'darwin-aarch64';
    if (filename.includes('x86_64') || filename.includes('amd64')) return 'darwin-x86_64';
  }
  if (filename.includes('linux')) return 'linux-x86_64';
  if (filename.includes('windows')) {
    if (filename.includes('arm64') || filename.includes('aarch64')) return 'windows-aarch64';
    return 'windows-x86_64';
  }
  return null;
}

const updaterData = {
  version,
  notes: `Nightly build from ${new Date().toISOString().split('T')[0]}`,
  pub_date: new Date().toISOString(),
  platforms: {},
};

const sigFiles = {};

// First pass: collect .sig files
const files = fs.readdirSync(artifactDir);
for (const file of files) {
  if (file.endsWith('.sig')) {
    const baseName = file.replace('.sig', '');
    sigFiles[baseName] = fs.readFileSync(path.join(artifactDir, file), 'utf-8').trim();
  }
}

// Second pass: build platform entries
for (const file of files) {
  if (file.endsWith('.sig') || file === 'updater-nightly.json') continue;

  const platformKey = getPlatformKey(file);
  if (!platformKey) continue;

  // Find signature for this file
  const sig = sigFiles[file] || sigFiles[file.replace(/\.(msi|exe|dmg|AppImage|deb|rpm|tar\.gz)$/, '')] || '';
  // On macOS, .dmg and .app.tar.gz share a .sig
  if (!sig && (file.endsWith('.dmg') || file.endsWith('.app.tar.gz'))) {
    const fallback = sigFiles[file.replace(/\.(dmg|app\.tar\.gz)$/, '')];
    if (fallback) sigFiles[file] = fallback;
  }

  updaterData.platforms[platformKey] = {
    signature: sig,
    url: `https://github.com/AirSodaz/sona/releases/download/nightly/${encodeURIComponent(file)}`,
  };
}

fs.writeFileSync(outputPath, JSON.stringify(updaterData, null, 2));
console.log(`Generated ${outputPath} with ${Object.keys(updaterData.platforms).length} platforms`);
