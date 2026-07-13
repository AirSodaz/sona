import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

function createStoredZip(entryNames) {
  const localEntries = [];
  const centralEntries = [];
  let localOffset = 0;

  for (const entryName of entryNames) {
    const name = Buffer.from(entryName, 'utf8');
    const local = Buffer.alloc(30 + name.length);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(name.length, 26);
    name.copy(local, 30);
    localEntries.push(local);

    const central = Buffer.alloc(46 + name.length);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(localOffset, 42);
    name.copy(central, 46);
    centralEntries.push(central);
    localOffset += local.length;
  }

  const centralDirectory = Buffer.concat(centralEntries);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(entryNames.length, 8);
  eocd.writeUInt16LE(entryNames.length, 10);
  eocd.writeUInt32LE(centralDirectory.length, 12);
  eocd.writeUInt32LE(localOffset, 16);
  return Buffer.concat([...localEntries, centralDirectory, eocd]);
}

test('APK verification rejects every required native library under a foreign ABI', async () => {
  let verifyAndroidClientApk;
  try {
    ({ verifyAndroidClientApk } = await import('./android-client-apk.js'));
  } catch (error) {
    assert.fail(`Android client APK verifier module is unavailable: ${error.message}`);
  }

  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-client-apk-'));
  const apkPath = path.join(fixtureRoot, 'app-arm64-v8a-debug.apk');
  const requiredLibraries = [
    'libsona_uniffi_bind.so',
    'libsherpa-onnx-c-api.so',
    'libonnxruntime.so',
  ];
  const validEntries = requiredLibraries.map((library) => `lib/arm64-v8a/${library}`);

  try {
    fs.writeFileSync(apkPath, createStoredZip(validEntries));
    assert.doesNotThrow(() => verifyAndroidClientApk(apkPath, 'arm64-v8a'));

    for (const library of requiredLibraries) {
      fs.writeFileSync(
        apkPath,
        createStoredZip([...validEntries, `lib/x86_64/${library}`]),
      );
      assert.throws(
        () => verifyAndroidClientApk(apkPath, 'arm64-v8a'),
        new RegExp(`Unexpected lib/x86_64/${library.replaceAll('.', '\\.')}`, 'u'),
      );
    }

    fs.writeFileSync(
      apkPath,
      createStoredZip([...validEntries, 'lib/armeabi-v7a/libonnxruntime.so']),
    );
    assert.throws(
      () => verifyAndroidClientApk(apkPath, 'arm64-v8a'),
      /Unexpected lib\/armeabi-v7a\/libonnxruntime\.so/u,
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
