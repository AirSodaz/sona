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
    productName: 'Sona Nightly',
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

function patchCargoToml(version) {
  const file = path.join(repoRoot, 'src-tauri', 'Cargo.toml');
  if (!fs.existsSync(file)) {
    throw new Error(`Cargo.toml not found at ${file}`);
  }
  const content = fs.readFileSync(file, 'utf8');
  const lines = content.split(/\r?\n/);
  let inPackageSection = false;
  let patched = false;

  const result = lines.map((line) => {
    const sectionMatch = line.match(/^\[(.+)\]\s*$/);
    if (sectionMatch) {
      inPackageSection = sectionMatch[1] === 'package';
      return line;
    }
    if (inPackageSection && !patched && /^\s*version\s*=\s*".*"\s*$/.test(line)) {
      patched = true;
      const leadingWhitespace = line.match(/^\s*/)[0];
      return `${leadingWhitespace}version = "${version}"`;
    }
    return line;
  });

  if (!patched) {
    throw new Error('Could not find a version line inside [package] in src-tauri/Cargo.toml');
  }

  // Preserve carriage returns if original content has them
  const lineEnding = content.includes('\r\n') ? '\r\n' : '\n';
  fs.writeFileSync(file, result.join(lineEnding));
  console.log(`[patch-channel] src-tauri/Cargo.toml -> version ${version}`);
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
  patchCargoToml(version);
  patchTauriConf(version, config);
  patchTauriWindowsConf(config);
}

main();
