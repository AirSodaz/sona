import fs from 'node:fs';

const supportedAndroidAbis = new Set(['arm64-v8a', 'x86_64']);
const maximumAndroidVersionCode = 2100000000;
const requiredNativeLibraries = new Set([
  'libsona_uniffi_bind.so',
  'libsherpa-onnx-c-api.so',
  'libonnxruntime.so',
]);

function trimmedEnvironmentValue(environment, name) {
  const value = environment[name];
  return value === undefined || value === null ? '' : String(value).trim();
}

function parseAndroidVersionCode(value) {
  if (!/^\d+$/u.test(value)) {
    throw new Error(
      `SONA_ANDROID_VERSION_CODE must be an integer from 1 to ${maximumAndroidVersionCode}`,
    );
  }
  const versionCode = Number(value);
  if (versionCode < 1 || versionCode > maximumAndroidVersionCode) {
    throw new Error(
      `SONA_ANDROID_VERSION_CODE must be an integer from 1 to ${maximumAndroidVersionCode}`,
    );
  }
  return versionCode;
}

function resolveAndroidClientBuildIdentity(environment = process.env) {
  const channel = trimmedEnvironmentValue(environment, 'SONA_ANDROID_CHANNEL') || 'stable';
  if (channel !== 'stable' && channel !== 'nightly') {
    throw new Error('SONA_ANDROID_CHANNEL must be stable or nightly');
  }

  const suppliedVersionName = trimmedEnvironmentValue(
    environment,
    'SONA_ANDROID_VERSION_NAME',
  );
  const suppliedVersionCode = trimmedEnvironmentValue(
    environment,
    'SONA_ANDROID_VERSION_CODE',
  );
  if (channel === 'nightly' && suppliedVersionName.length === 0) {
    throw new Error('SONA_ANDROID_VERSION_NAME is required for nightly builds');
  }
  if (channel === 'nightly' && suppliedVersionCode.length === 0) {
    throw new Error('SONA_ANDROID_VERSION_CODE is required for nightly builds');
  }

  return {
    channel,
    applicationId: channel === 'nightly' ? 'com.sona.android.nightly' : 'com.sona.android',
    appName: channel === 'nightly' ? 'Sona Nightly' : 'Sona',
    versionCode: parseAndroidVersionCode(suppliedVersionCode || '1'),
    versionName: suppliedVersionName || '0.8.0',
  };
}

function findEndOfCentralDirectory(zipBuffer) {
  const minimumOffset = Math.max(0, zipBuffer.length - 65557);
  for (let offset = zipBuffer.length - 22; offset >= minimumOffset; offset -= 1) {
    if (zipBuffer.readUInt32LE(offset) === 0x06054b50) {
      return offset;
    }
  }
  throw new Error('Invalid ZIP file: end of central directory not found');
}

function readZipEntries(zipBuffer) {
  const eocdOffset = findEndOfCentralDirectory(zipBuffer);
  const entryCount = zipBuffer.readUInt16LE(eocdOffset + 10);
  const centralDirectoryOffset = zipBuffer.readUInt32LE(eocdOffset + 16);
  const entries = [];
  let offset = centralDirectoryOffset;

  for (let index = 0; index < entryCount; index += 1) {
    if (zipBuffer.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error(`Invalid ZIP central directory entry at offset ${offset}`);
    }
    const fileNameLength = zipBuffer.readUInt16LE(offset + 28);
    const extraFieldLength = zipBuffer.readUInt16LE(offset + 30);
    const fileCommentLength = zipBuffer.readUInt16LE(offset + 32);
    const nameStart = offset + 46;
    entries.push(zipBuffer.toString('utf8', nameStart, nameStart + fileNameLength));
    offset = nameStart + fileNameLength + extraFieldLength + fileCommentLength;
  }

  return entries;
}

function verifyAndroidClientApk(apkPath, abi) {
  if (!supportedAndroidAbis.has(abi)) {
    throw new Error(`Android client does not deliver an APK for ABI ${abi}`);
  }
  if (!fs.existsSync(apkPath)) {
    throw new Error(`Missing Android client APK at ${apkPath}`);
  }

  const entries = new Set(readZipEntries(fs.readFileSync(apkPath)));
  for (const library of requiredNativeLibraries) {
    const entry = `lib/${abi}/${library}`;
    if (!entries.has(entry)) {
      throw new Error(`Missing ${entry} in ${apkPath}`);
    }
  }

  for (const entry of entries) {
    const match = /^lib\/([^/]+)\/([^/]+)$/u.exec(entry);
    if (match && requiredNativeLibraries.has(match[2]) && match[1] !== abi) {
      throw new Error(`Unexpected ${entry} in ${apkPath}`);
    }
  }
}

function verifyAndroidClientOutputMetadata(metadataPath, expected) {
  if (!fs.existsSync(metadataPath)) {
    throw new Error(`Missing Android output metadata at ${metadataPath}`);
  }

  const metadata = JSON.parse(fs.readFileSync(metadataPath, 'utf8'));
  if (metadata.applicationId !== expected.applicationId) {
    throw new Error(
      `Expected Android applicationId ${expected.applicationId}, found ${metadata.applicationId}`,
    );
  }
  if (metadata.variantName !== expected.variantName) {
    throw new Error(
      `Expected Android variant ${expected.variantName}, found ${metadata.variantName}`,
    );
  }

  const elements = Array.isArray(metadata.elements) ? metadata.elements : [];
  const expectedAbis = [...new Set(expected.abis)];
  if (elements.length !== expectedAbis.length) {
    throw new Error(
      `Expected output metadata for ${expectedAbis.length} Android ABI, found ${elements.length}`,
    );
  }

  const elementsByAbi = new Map();
  for (const element of elements) {
    const filters = Array.isArray(element.filters) ? element.filters : [];
    const abiFilters = filters.filter((filter) => filter.filterType === 'ABI');
    if (abiFilters.length !== 1 || typeof abiFilters[0].value !== 'string') {
      throw new Error(`Android output metadata entry is missing exactly one ABI filter`);
    }
    const abi = abiFilters[0].value;
    if (elementsByAbi.has(abi)) {
      throw new Error(`Android output metadata contains duplicate ABI ${abi}`);
    }
    elementsByAbi.set(abi, element);
  }

  for (const abi of expectedAbis) {
    if (!supportedAndroidAbis.has(abi)) {
      throw new Error(`Android client does not deliver an APK for ABI ${abi}`);
    }
    const element = elementsByAbi.get(abi);
    if (!element) {
      throw new Error(`Missing Android output metadata for ABI ${abi}`);
    }
    if (element.versionCode !== expected.versionCode) {
      throw new Error(
        `Expected Android versionCode ${expected.versionCode} for ${abi}, found ${element.versionCode}`,
      );
    }
    if (element.versionName !== expected.versionName) {
      throw new Error(
        `Expected Android versionName ${expected.versionName} for ${abi}, found ${element.versionName}`,
      );
    }
    const expectedFile = `app-${abi}-${expected.fileSuffix}.apk`;
    if (element.outputFile !== expectedFile) {
      throw new Error(
        `Expected Android output file ${expectedFile} for ${abi}, found ${element.outputFile}`,
      );
    }
  }

  for (const abi of elementsByAbi.keys()) {
    if (!expectedAbis.includes(abi)) {
      throw new Error(`Unexpected Android output metadata for ABI ${abi}`);
    }
  }
}

export {
  readZipEntries,
  resolveAndroidClientBuildIdentity,
  verifyAndroidClientApk,
  verifyAndroidClientOutputMetadata,
};
