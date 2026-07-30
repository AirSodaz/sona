#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const maximumAndroidVersionCode = 2100000000;
const supportedChannels = new Set(['stable', 'nightly']);

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

function createAndroidUpdateManifest({ channel, versionName, versionCode }) {
  const normalizedChannel = String(channel ?? '').trim();
  if (!supportedChannels.has(normalizedChannel)) {
    throw new Error('Android update channel must be stable or nightly');
  }
  const normalizedVersionName = String(versionName ?? '').trim();
  if (normalizedVersionName.length === 0 || normalizedVersionName.length > 100) {
    throw new Error('Android update versionName must contain 1 to 100 characters');
  }
  return {
    schemaVersion: 1,
    channel: normalizedChannel,
    versionName: normalizedVersionName,
    versionCode: parseVersionCode(versionCode),
  };
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
  const manifest = createAndroidUpdateManifest({
    channel: optionValue(commandArgs, '--channel'),
    versionName: optionValue(commandArgs, '--version-name'),
    versionCode: optionValue(commandArgs, '--version-code'),
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
  createAndroidUpdateManifest,
  runCli,
  validateReleaseTagVersion,
};
