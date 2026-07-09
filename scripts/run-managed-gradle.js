#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import https from 'node:https';
import path from 'node:path';
import { pipeline } from 'node:stream/promises';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const DEFAULT_GRADLE_VERSION = '9.6.1';
const MAX_DOWNLOAD_ATTEMPTS = 3;
const GRADLE_DISTRIBUTION_SHA256 = {
  '9.6.1': '9c0f7faeeb306cb14e4279a3e084ca6b596894089a0638e68a07c945a32c9e14',
};

const args = process.argv.slice(2);
const delimiterIndex = args.indexOf('--');
const runnerArgs = delimiterIndex === -1 ? args : args.slice(0, delimiterIndex);
const gradleArgs = delimiterIndex === -1 ? [] : args.slice(delimiterIndex + 1);

function readOption(name, fallback) {
  const index = runnerArgs.indexOf(name);
  if (index === -1) {
    return fallback;
  }
  const value = runnerArgs[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function commandExists(command) {
  const probe = process.platform === 'win32'
    ? spawnSync('where.exe', [command], { stdio: 'ignore' })
    : spawnSync('command', ['-v', command], { stdio: 'ignore', shell: true });
  return probe.status === 0;
}

function curlCommand() {
  if (process.platform === 'win32' && commandExists('curl.exe')) {
    return 'curl.exe';
  }
  return commandExists('curl') ? 'curl' : null;
}

function curlDownloadFile(curl, url, destination) {
  const temporaryDestination = `${destination}.tmp`;
  fs.rmSync(temporaryDestination, { force: true });
  const result = spawnSync(curl, [
    '--fail',
    '--location',
    '--retry',
    String(MAX_DOWNLOAD_ATTEMPTS),
    '--connect-timeout',
    '30',
    '--output',
    temporaryDestination,
    url,
  ], {
    cwd: repoRoot,
    env: process.env,
    stdio: 'inherit',
  });

  if (result.status !== 0) {
    fs.rmSync(temporaryDestination, { force: true });
    throw new Error(`${curl} failed to download ${url} with exit code ${result.status}`);
  }

  fs.renameSync(temporaryDestination, destination);
}

function downloadFile(url, destination, redirectsRemaining = 5) {
  return new Promise((resolve, reject) => {
    https.get(url, (response) => {
      if (
        response.statusCode >= 300
        && response.statusCode < 400
        && response.headers.location
      ) {
        response.resume();
        if (redirectsRemaining <= 0) {
          reject(new Error(`Too many redirects while downloading ${url}`));
          return;
        }
        resolve(downloadFile(new URL(response.headers.location, url).toString(), destination, redirectsRemaining - 1));
        return;
      }

      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Failed to download ${url}: HTTP ${response.statusCode}`));
        return;
      }

      const temporaryDestination = `${destination}.tmp`;
      const output = fs.createWriteStream(temporaryDestination);
      pipeline(response, output)
        .then(() => {
          fs.renameSync(temporaryDestination, destination);
          resolve();
        })
        .catch((error) => {
          fs.rmSync(temporaryDestination, { force: true });
          reject(error);
        });
    }).on('error', reject);
  });
}

function wait(milliseconds) {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

async function downloadWithRetry(url, destination) {
  const curl = curlCommand();
  if (curl) {
    try {
      curlDownloadFile(curl, url, destination);
      return;
    } catch (error) {
      console.warn(`${error.message}. Falling back to Node HTTPS download...`);
    }
  }

  let lastError = null;
  for (let attempt = 1; attempt <= MAX_DOWNLOAD_ATTEMPTS; attempt += 1) {
    try {
      fs.rmSync(`${destination}.tmp`, { force: true });
      await downloadFile(url, destination);
      return;
    } catch (error) {
      lastError = error;
      fs.rmSync(`${destination}.tmp`, { force: true });
      if (attempt < MAX_DOWNLOAD_ATTEMPTS) {
        console.warn(`Gradle download attempt ${attempt} failed: ${error.message}. Retrying...`);
        await wait(attempt * 1000);
      }
    }
  }

  throw lastError;
}

async function downloadGradleDistribution(gradleVersion, destination) {
  const distributionUrls = [
    `https://services.gradle.org/distributions/gradle-${gradleVersion}-bin.zip`,
    `https://downloads.gradle.org/distributions/gradle-${gradleVersion}-bin.zip`,
  ];
  let lastError = null;

  for (const [index, distributionUrl] of distributionUrls.entries()) {
    try {
      console.log(`Downloading Gradle ${gradleVersion} from ${distributionUrl}`);
      await downloadWithRetry(distributionUrl, destination);
      return;
    } catch (error) {
      lastError = error;
      fs.rmSync(`${destination}.tmp`, { force: true });
      if (index < distributionUrls.length - 1) {
        console.warn(`Gradle download from ${distributionUrl} failed: ${error.message}. Trying fallback URL...`);
      }
    }
  }

  throw lastError;
}

function sha256File(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

function verifyChecksum(filePath, expectedChecksum) {
  const actualChecksum = sha256File(filePath);
  if (actualChecksum !== expectedChecksum) {
    throw new Error(`Gradle distribution checksum mismatch for ${filePath}: expected ${expectedChecksum}, got ${actualChecksum}`);
  }
}

function run(command, commandArgs, options = {}) {
  const result = spawnSync(command, commandArgs, {
    cwd: options.cwd ?? repoRoot,
    env: {
      ...process.env,
      ...options.env,
    },
    stdio: 'inherit',
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${commandArgs.join(' ')} failed with exit code ${result.status}`);
  }
}

function extractDistribution(zipPath, cacheDir, gradleVersion, installDir) {
  fs.rmSync(installDir, { recursive: true, force: true });
  fs.mkdirSync(cacheDir, { recursive: true });

  if (process.platform === 'win32') {
    run('powershell', [
      '-NoProfile',
      '-Command',
      `Expand-Archive -LiteralPath '${zipPath.replace(/'/gu, "''")}' -DestinationPath '${cacheDir.replace(/'/gu, "''")}' -Force`,
    ]);
  } else {
    run('unzip', ['-q', '-o', zipPath, '-d', cacheDir]);
  }

  const gradleExecutable = path.join(installDir, 'bin', process.platform === 'win32' ? 'gradle.bat' : 'gradle');
  if (!fs.existsSync(gradleExecutable)) {
    throw new Error(`Managed Gradle ${gradleVersion} did not extract ${gradleExecutable}`);
  }
  if (process.platform !== 'win32') {
    fs.chmodSync(gradleExecutable, 0o755);
  }
}

async function ensureGradleDistribution(gradleVersion, cacheDir, distributionZip) {
  const expectedChecksum = GRADLE_DISTRIBUTION_SHA256[gradleVersion];
  if (!expectedChecksum) {
    throw new Error(`No pinned checksum for Gradle ${gradleVersion}`);
  }

  const installDir = path.join(cacheDir, `gradle-${gradleVersion}`);
  const gradleExecutable = path.join(installDir, 'bin', process.platform === 'win32' ? 'gradle.bat' : 'gradle');
  if (fs.existsSync(gradleExecutable)) {
    return gradleExecutable;
  }

  fs.mkdirSync(cacheDir, { recursive: true });
  const zipPath = path.join(cacheDir, `gradle-${gradleVersion}-bin.zip`);
  if (distributionZip) {
    if (!fs.existsSync(distributionZip)) {
      throw new Error(`Gradle distribution zip does not exist: ${distributionZip}`);
    }
    console.log(`Using Gradle distribution zip from ${distributionZip}`);
    fs.copyFileSync(distributionZip, zipPath);
  } else if (!fs.existsSync(zipPath)) {
    await downloadGradleDistribution(gradleVersion, zipPath);
  }
  verifyChecksum(zipPath, expectedChecksum);
  extractDistribution(zipPath, cacheDir, gradleVersion, installDir);
  return gradleExecutable;
}

const gradleVersion = readOption('--gradle-version', process.env.SONA_GRADLE_VERSION ?? DEFAULT_GRADLE_VERSION);
const cacheDir = path.resolve(
  readOption('--cache-dir', process.env.SONA_GRADLE_CACHE_DIR ?? path.join(repoRoot, 'target', 'managed-gradle')),
);
const distributionZipOption = readOption('--distribution-zip', process.env.SONA_GRADLE_DISTRIBUTION_ZIP ?? null);
const distributionZip = distributionZipOption ? path.resolve(distributionZipOption) : null;
const projectDir = path.resolve(readOption('--project-dir', process.cwd()));

if (gradleArgs.length === 0) {
  throw new Error('Pass Gradle arguments after --, for example: -- :sample-library:tasks --quiet');
}

const gradleExecutable = await ensureGradleDistribution(gradleVersion, cacheDir, distributionZip);
const result = spawnSync(gradleExecutable, ['--project-dir', projectDir, ...gradleArgs], {
  cwd: projectDir,
  env: process.env,
  shell: process.platform === 'win32',
  stdio: 'inherit',
});

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);
