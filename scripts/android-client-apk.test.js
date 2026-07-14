import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

async function loadAndroidClientVerifier() {
  try {
    return await import('./android-client-apk.js');
  } catch (error) {
    assert.fail(`Android client APK verifier module is unavailable: ${error.message}`);
  }
}

function createOutputMetadata({
  applicationId,
  variantName,
  versionCode,
  versionName,
  abis,
  fileSuffix,
}) {
  return {
    version: 3,
    artifactType: {
      type: 'APK',
      kind: 'Directory',
    },
    applicationId,
    variantName,
    elements: abis.map((abi) => ({
      type: 'SINGLE',
      filters: [
        {
          filterType: 'ABI',
          value: abi,
        },
      ],
      attributes: [],
      versionCode,
      versionName,
      outputFile: `app-${abi}-${fileSuffix}.apk`,
    })),
    elementType: 'File',
  };
}

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
  const { verifyAndroidClientApk } = await loadAndroidClientVerifier();

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

test('Android build identity resolves stable defaults and explicit nightly values', async () => {
  const { resolveAndroidClientBuildIdentity } = await loadAndroidClientVerifier();

  assert.deepEqual(resolveAndroidClientBuildIdentity({}), {
    channel: 'stable',
    applicationId: 'com.sona.android',
    appName: 'Sona',
    versionCode: 1,
    versionName: '0.8.0',
  });

  assert.deepEqual(resolveAndroidClientBuildIdentity({
    SONA_ANDROID_CHANNEL: ' nightly ',
    SONA_ANDROID_VERSION_CODE: '123',
    SONA_ANDROID_VERSION_NAME: ' 0.8.0-123 ',
  }), {
    channel: 'nightly',
    applicationId: 'com.sona.android.nightly',
    appName: 'Sona Nightly',
    versionCode: 123,
    versionName: '0.8.0-123',
  });
});

test('Android build identity rejects invalid channel and version values', async () => {
  const { resolveAndroidClientBuildIdentity } = await loadAndroidClientVerifier();

  assert.throws(
    () => resolveAndroidClientBuildIdentity({ SONA_ANDROID_CHANNEL: 'preview' }),
    /SONA_ANDROID_CHANNEL must be stable or nightly/u,
  );
  assert.throws(
    () => resolveAndroidClientBuildIdentity({
      SONA_ANDROID_CHANNEL: 'nightly',
      SONA_ANDROID_VERSION_CODE: '123',
    }),
    /SONA_ANDROID_VERSION_NAME is required for nightly builds/u,
  );
  assert.throws(
    () => resolveAndroidClientBuildIdentity({
      SONA_ANDROID_CHANNEL: 'nightly',
      SONA_ANDROID_VERSION_NAME: '0.8.0-123',
    }),
    /SONA_ANDROID_VERSION_CODE is required for nightly builds/u,
  );
  for (const versionCode of ['0', '1.5', '2100000001']) {
    assert.throws(
      () => resolveAndroidClientBuildIdentity({
        SONA_ANDROID_CHANNEL: 'nightly',
        SONA_ANDROID_VERSION_CODE: versionCode,
        SONA_ANDROID_VERSION_NAME: '0.8.0-123',
      }),
      /SONA_ANDROID_VERSION_CODE must be an integer from 1 to 2100000000/u,
    );
  }
});

test('Android output metadata verification checks channel identity and every APK', async () => {
  const { verifyAndroidClientOutputMetadata } = await loadAndroidClientVerifier();
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-client-metadata-'));
  const metadataPath = path.join(fixtureRoot, 'output-metadata.json');
  const stableMetadata = createOutputMetadata({
    applicationId: 'com.sona.android',
    variantName: 'debug',
    versionCode: 1,
    versionName: '0.8.0',
    abis: ['arm64-v8a', 'x86_64'],
    fileSuffix: 'debug',
  });

  try {
    fs.writeFileSync(metadataPath, JSON.stringify(stableMetadata));
    assert.doesNotThrow(() => verifyAndroidClientOutputMetadata(metadataPath, {
      applicationId: 'com.sona.android',
      variantName: 'debug',
      versionCode: 1,
      versionName: '0.8.0',
      abis: ['arm64-v8a', 'x86_64'],
      fileSuffix: 'debug',
    }));

    const nightlyMetadata = createOutputMetadata({
      applicationId: 'com.sona.android.nightly',
      variantName: 'release',
      versionCode: 123,
      versionName: '0.8.0-123',
      abis: ['arm64-v8a'],
      fileSuffix: 'release-unsigned',
    });
    fs.writeFileSync(metadataPath, JSON.stringify(nightlyMetadata));
    assert.doesNotThrow(() => verifyAndroidClientOutputMetadata(metadataPath, {
      applicationId: 'com.sona.android.nightly',
      variantName: 'release',
      versionCode: 123,
      versionName: '0.8.0-123',
      abis: ['arm64-v8a'],
      fileSuffix: 'release-unsigned',
    }));

    assert.throws(
      () => verifyAndroidClientOutputMetadata(metadataPath, {
        applicationId: 'com.sona.android',
        variantName: 'release',
        versionCode: 123,
        versionName: '0.8.0-123',
        abis: ['arm64-v8a'],
        fileSuffix: 'release-unsigned',
      }),
      /Expected Android applicationId com\.sona\.android, found com\.sona\.android\.nightly/u,
    );

    const missingAbiMetadata = {
      ...nightlyMetadata,
      elements: [],
    };
    fs.writeFileSync(metadataPath, JSON.stringify(missingAbiMetadata));
    assert.throws(
      () => verifyAndroidClientOutputMetadata(metadataPath, {
        applicationId: 'com.sona.android.nightly',
        variantName: 'release',
        versionCode: 123,
        versionName: '0.8.0-123',
        abis: ['arm64-v8a'],
        fileSuffix: 'release-unsigned',
      }),
      /Expected output metadata for 1 Android ABI, found 0/u,
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
