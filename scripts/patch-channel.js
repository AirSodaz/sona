#!/usr/bin/env node
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

const CHANNEL_CONFIG = {
  nightly: {
    identifier: 'com.asoda.sona.nightly',
    productName: 'Sona-Nightly',
    updaterEndpoints: [
      'https://github.com/AirSodaz/sona/releases/download/nightly/updater.json',
    ],
  },
};

function readArg(name) {
  const idx = process.argv.indexOf(`--${name}`);
  if (idx === -1) return undefined;
  const val = process.argv[idx + 1];
  if (val === undefined || val.startsWith('--')) {
    return undefined;
  }
  return val;
}

function patchPackageJson(version) {
  const file = path.join(repoRoot, 'package.json');
  if (!fs.existsSync(file)) {
    throw new Error(`package.json not found at ${file}`);
  }
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  json.version = version;
  fs.writeFileSync(file, JSON.stringify(json, null, 2) + '\n');
  console.log(`[patch-channel] package.json -> version ${version}`);
}

function patchWorkspaceCargoToml(version) {
  const file = path.join(repoRoot, 'Cargo.toml');
  if (!fs.existsSync(file)) {
    throw new Error(`Cargo.toml not found at ${file}`);
  }
  const content = fs.readFileSync(file, 'utf8');
  const lines = content.split(/\r?\n/);
  let inWorkspacePackageSection = false;
  let patched = false;

  const result = lines.map((line) => {
    const sectionMatch = line.match(/^\[(.+)\]\s*$/);
    if (sectionMatch) {
      inWorkspacePackageSection = sectionMatch[1] === 'workspace.package';
      return line;
    }
    if (inWorkspacePackageSection && !patched && /^\s*version\s*=\s*".*"\s*$/.test(line)) {
      patched = true;
      const leadingWhitespace = line.match(/^\s*/)[0];
      return `${leadingWhitespace}version = "${version}"`;
    }
    return line;
  });

  if (!patched) {
    throw new Error('Could not find a version line inside [workspace.package] in Cargo.toml');
  }

  // Preserve carriage returns if original content has them
  const lineEnding = content.includes('\r\n') ? '\r\n' : '\n';
  fs.writeFileSync(file, result.join(lineEnding));
  console.log(`[patch-channel] Cargo.toml (workspace) -> version ${version}`);
}

function patchTauriConf(version, config) {
  const file = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
  if (!fs.existsSync(file)) {
    throw new Error(`tauri.conf.json not found at ${file}`);
  }
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  json.version = version;
  json.identifier = config.identifier;
  json.productName = config.productName;
  if (json.plugins && json.plugins.updater) {
    json.plugins.updater.endpoints = config.updaterEndpoints;
  } else {
    console.warn('Warning: plugins or plugins.updater config is missing in tauri.conf.json');
  }
  fs.writeFileSync(file, JSON.stringify(json, null, 2) + '\n');
  console.log(`[patch-channel] src-tauri/tauri.conf.json -> identifier ${config.identifier}, productName "${config.productName}"`);
  console.log(`[patch-channel] src-tauri/tauri.conf.json -> updater endpoint ${config.updaterEndpoints[0]}`);
}

function patchTauriWindowsConf(config) {
  const file = path.join(repoRoot, 'src-tauri', 'tauri.windows.conf.json');
  if (fs.existsSync(file)) {
    const json = JSON.parse(fs.readFileSync(file, 'utf8'));
    json.identifier = config.identifier;
    fs.writeFileSync(file, JSON.stringify(json, null, 2) + '\n');
    console.log(`[patch-channel] src-tauri/tauri.windows.conf.json -> identifier ${config.identifier}`);
  } else {
    console.log(`[patch-channel] src-tauri/tauri.windows.conf.json not found, skipping.`);
  }
}

function copyNightlyIcons() {
  const srcDir = path.join(repoRoot, 'src-tauri', 'icons-nightly');
  const destDir = path.join(repoRoot, 'src-tauri', 'icons');
  if (!fs.existsSync(srcDir) || !fs.statSync(srcDir).isDirectory()) {
    throw new Error(`Nightly icons directory not found at ${srcDir}`);
  }
  fs.cpSync(srcDir, destDir, { recursive: true, force: true });
  console.log(`[patch-channel] Copied nightly icons from ${srcDir} to ${destDir}`);
}

function main() {
  const channel = readArg('channel');
  const version = readArg('version');

  if (!channel || !version) {
    console.error('Usage: node scripts/patch-channel.js --channel <name> --version <semver>');
    process.exit(1);
  }

  const config = CHANNEL_CONFIG[channel];
  if (!config) {
    console.error(`Unknown channel "${channel}". Known channels: ${Object.keys(CHANNEL_CONFIG).join(', ')}`);
    process.exit(1);
  }

  patchPackageJson(version);
  patchWorkspaceCargoToml(version);
  patchTauriConf(version, config);
  patchTauriWindowsConf(config);

  if (channel === 'nightly') {
    copyNightlyIcons();
  }
}

main();
