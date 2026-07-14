import fs from 'node:fs';

const supportedAndroidAbis = new Set(['arm64-v8a', 'x86_64']);
const requiredNativeLibraries = new Set([
  'libsona_uniffi_bind.so',
  'libsherpa-onnx-c-api.so',
  'libonnxruntime.so',
]);

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

export {
  readZipEntries,
  verifyAndroidClientApk,
};
