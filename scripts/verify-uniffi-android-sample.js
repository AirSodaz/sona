#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const args = process.argv.slice(2);
const requireGradle = args.includes('--require-gradle');
const skipGradle = args.includes('--skip-gradle');
const downloadGradle = args.includes('--download-gradle');
const sampleProjectDir = path.join(repoRoot, 'platforms', 'android', 'sample-consumer');
const defaultAndroidAbi = 'arm64-v8a';
const defaultJniLibraryEntry = 'jni/arm64-v8a/libsona_uniffi_bind.so';
const samplePublicationVersion = '0.8.0';
const sampleMavenCoordinatePath = 'com/sona/sona-uniffi-bindings/0.8.0';

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

function commandExists(command) {
  const probe = process.platform === 'win32'
    ? spawnSync('where.exe', [command], { stdio: 'ignore' })
    : spawnSync('command', ['-v', command], { stdio: 'ignore', shell: true });
  return probe.status === 0;
}

function gradleCommand() {
  const wrapper = process.platform === 'win32'
    ? path.join(sampleProjectDir, 'gradlew.bat')
    : path.join(sampleProjectDir, 'gradlew');
  if (fs.existsSync(wrapper)) {
    return wrapper;
  }

  if (commandExists(process.platform === 'win32' ? 'gradle.bat' : 'gradle')) {
    return process.platform === 'win32' ? 'gradle.bat' : 'gradle';
  }
  if (commandExists('gradle')) {
    return 'gradle';
  }

  return null;
}

function runGradleTasks(gradleArgs, gradleEnv) {
  const gradle = gradleCommand();
  if (gradle) {
    run(gradle, gradleArgs, {
      cwd: sampleProjectDir,
      env: gradleEnv,
    });
    return true;
  }

  if (downloadGradle) {
    run('node', [
      path.join(repoRoot, 'scripts', 'run-managed-gradle.js'),
      '--project-dir',
      sampleProjectDir,
      '--',
      ...gradleArgs,
    ], {
      cwd: repoRoot,
      env: gradleEnv,
    });
    return true;
  }

  if (requireGradle) {
    throw new Error('Gradle is required but neither sample gradlew nor gradle is available on PATH');
  }

  console.warn('Skipping Gradle sample project check: no sample gradlew or gradle executable found');
  return false;
}

function splitList(value) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function selectedAndroidAbis() {
  return splitList(process.env.SONA_ANDROID_ABIS ?? defaultAndroidAbi);
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

    const compressionMethod = zipBuffer.readUInt16LE(offset + 10);
    const compressedSize = zipBuffer.readUInt32LE(offset + 20);
    const uncompressedSize = zipBuffer.readUInt32LE(offset + 24);
    const fileNameLength = zipBuffer.readUInt16LE(offset + 28);
    const extraFieldLength = zipBuffer.readUInt16LE(offset + 30);
    const fileCommentLength = zipBuffer.readUInt16LE(offset + 32);
    const localHeaderOffset = zipBuffer.readUInt32LE(offset + 42);
    const nameStart = offset + 46;
    const name = zipBuffer.toString('utf8', nameStart, nameStart + fileNameLength);

    entries.push({
      name,
      compressionMethod,
      compressedSize,
      uncompressedSize,
      localHeaderOffset,
    });

    offset = nameStart + fileNameLength + extraFieldLength + fileCommentLength;
  }

  return entries;
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

function readZipEntryData(zipBuffer, entry) {
  const offset = entry.localHeaderOffset;
  if (zipBuffer.readUInt32LE(offset) !== 0x04034b50) {
    throw new Error(`Invalid ZIP local file header for ${entry.name}`);
  }

  const fileNameLength = zipBuffer.readUInt16LE(offset + 26);
  const extraFieldLength = zipBuffer.readUInt16LE(offset + 28);
  const dataStart = offset + 30 + fileNameLength + extraFieldLength;
  const compressedData = zipBuffer.subarray(dataStart, dataStart + entry.compressedSize);

  if (entry.compressionMethod === 0) {
    return compressedData;
  }
  if (entry.compressionMethod === 8) {
    return zlib.inflateRawSync(compressedData);
  }

  throw new Error(`Unsupported ZIP compression method ${entry.compressionMethod} for ${entry.name}`);
}

function findRequiredZipEntry(entries, name, archiveLabel) {
  const entry = entries.find((item) => item.name === name);
  if (!entry) {
    throw new Error(`Missing ${name} in ${archiveLabel}`);
  }
  return entry;
}

function verifyAndroidSampleAarContents(aarPath) {
  if (!fs.existsSync(aarPath)) {
    throw new Error(`Missing Android sample AAR at ${aarPath}`);
  }

  const aarBuffer = fs.readFileSync(aarPath);
  const aarEntries = readZipEntries(aarBuffer);
  const aarEntryNames = new Set(aarEntries.map((entry) => entry.name));
  for (const abi of selectedAndroidAbis()) {
    const jniEntry = abi === defaultAndroidAbi
      ? defaultJniLibraryEntry
      : `jni/${abi}/libsona_uniffi_bind.so`;
    if (!aarEntryNames.has(jniEntry)) {
      throw new Error(`Missing ${jniEntry} in ${aarPath}`);
    }
  }

  const classesEntry = findRequiredZipEntry(aarEntries, 'classes.jar', aarPath);
  const classesBuffer = readZipEntryData(aarBuffer, classesEntry);
  const classEntries = readZipEntries(classesBuffer).map((entry) => entry.name);
  if (!classEntries.some((entry) => entry.startsWith('uniffi/sona_uniffi_bind/') && entry.endsWith('.class'))) {
    throw new Error('Missing compiled uniffi/sona_uniffi_bind/ classes in sample AAR classes.jar');
  }
  if (!classEntries.some((entry) => entry.startsWith('com/sona/uniffi/sample/SonaUniffiSmoke'))) {
    throw new Error('Missing compiled com/sona/uniffi/sample/SonaUniffiSmoke class in sample AAR classes.jar');
  }
}

function verifyAndroidSampleAar() {
  verifyAndroidSampleAarContents(path.join(
    sampleProjectDir,
    'sample-library',
    'build',
    'outputs',
    'aar',
    'sample-library-debug.aar',
  ));
}

function verifyAndroidConsumerAar() {
  const aarPath = path.join(
    sampleProjectDir,
    'consumer-library',
    'build',
    'outputs',
    'aar',
    'consumer-library-debug.aar',
  );
  if (!fs.existsSync(aarPath)) {
    throw new Error(`Missing Android consumer AAR at ${aarPath}`);
  }

  const aarBuffer = fs.readFileSync(aarPath);
  const aarEntries = readZipEntries(aarBuffer);
  const classesEntry = findRequiredZipEntry(aarEntries, 'classes.jar', aarPath);
  const classesBuffer = readZipEntryData(aarBuffer, classesEntry);
  const classEntries = readZipEntries(classesBuffer).map((entry) => entry.name);
  if (!classEntries.some((entry) => entry.startsWith('com/sona/uniffi/consumer/SonaUniffiConsumerSmoke'))) {
    throw new Error('Missing compiled com/sona/uniffi/consumer/SonaUniffiConsumerSmoke class in consumer AAR classes.jar');
  }
}

function verifyPomDependency(pomXml, coordinates) {
  const [groupId, artifactId] = coordinates.split(':');
  const dependencyPattern = new RegExp(
    `<dependency>[\\s\\S]*<groupId>${escapeRegExp(groupId)}</groupId>[\\s\\S]*`
      + `<artifactId>${escapeRegExp(artifactId)}</artifactId>[\\s\\S]*</dependency>`,
    'u',
  );
  if (!dependencyPattern.test(pomXml)) {
    throw new Error(`Missing ${coordinates} dependency in published POM`);
  }
}

function verifyGradleModuleDependency(variant, coordinates) {
  const [group, module] = coordinates.split(':');
  const dependencies = Array.isArray(variant.dependencies) ? variant.dependencies : [];
  const found = dependencies.some((dependency) => (
    dependency.group === group && dependency.module === module
  ));
  if (!found) {
    throw new Error(`Missing ${coordinates} dependency in published Gradle module metadata`);
  }
}

function findGradleModuleVariant(moduleMetadata, usage) {
  const variants = Array.isArray(moduleMetadata.variants) ? moduleMetadata.variants : [];
  const variant = variants.find((item) => item.attributes?.['org.gradle.usage'] === usage);
  if (!variant) {
    throw new Error(`Missing ${usage} variant in published Gradle module metadata`);
  }
  if (variant.attributes?.['org.gradle.libraryelements'] !== 'aar') {
    throw new Error(`${usage} variant must publish AAR library elements`);
  }
  const files = Array.isArray(variant.files) ? variant.files : [];
  const expectedAarName = `sona-uniffi-bindings-${samplePublicationVersion}.aar`;
  if (!files.some((file) => file.name === expectedAarName && file.url === expectedAarName)) {
    throw new Error(`Missing ${expectedAarName} file entry in ${usage} Gradle module metadata`);
  }
  return variant;
}

function verifyAndroidSampleGradleModuleMetadata(modulePath) {
  if (!fs.existsSync(modulePath)) {
    throw new Error(`Missing Android sample Gradle module metadata at ${modulePath}`);
  }

  const moduleMetadata = JSON.parse(fs.readFileSync(modulePath, 'utf8'));
  if (moduleMetadata.component?.group !== 'com.sona') {
    throw new Error('Published Gradle module metadata must use group com.sona');
  }
  if (moduleMetadata.component?.module !== 'sona-uniffi-bindings') {
    throw new Error('Published Gradle module metadata must use module sona-uniffi-bindings');
  }
  if (moduleMetadata.component?.version !== samplePublicationVersion) {
    throw new Error(`Published Gradle module metadata must use version ${samplePublicationVersion}`);
  }

  findGradleModuleVariant(moduleMetadata, 'java-api');
  const runtimeVariant = findGradleModuleVariant(moduleMetadata, 'java-runtime');
  verifyGradleModuleDependency(runtimeVariant, 'net.java.dev.jna:jna');
  verifyGradleModuleDependency(runtimeVariant, 'org.jetbrains.kotlinx:kotlinx-coroutines-core');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

function verifyAndroidSampleMavenPublication() {
  const publicationDir = path.join(
    sampleProjectDir,
    'sample-library',
    'build',
    'repo',
    ...sampleMavenCoordinatePath.split('/'),
  );
  const aarPath = path.join(publicationDir, `sona-uniffi-bindings-${samplePublicationVersion}.aar`);
  const pomPath = path.join(publicationDir, `sona-uniffi-bindings-${samplePublicationVersion}.pom`);
  const modulePath = path.join(publicationDir, `sona-uniffi-bindings-${samplePublicationVersion}.module`);

  verifyAndroidSampleAarContents(aarPath);
  if (!fs.existsSync(pomPath)) {
    throw new Error(`Missing Android sample Maven POM at ${pomPath}`);
  }

  const pomXml = fs.readFileSync(pomPath, 'utf8');
  verifyPomDependency(pomXml, 'net.java.dev.jna:jna');
  verifyPomDependency(pomXml, 'org.jetbrains.kotlinx:kotlinx-coroutines-core');
  verifyAndroidSampleGradleModuleMetadata(modulePath);
}

run('node', [
  path.join(repoRoot, 'scripts', 'generate-uniffi-kotlin.js'),
  '--profile',
  'debug',
  '--out-dir',
  path.join(repoRoot, 'target', 'uniffi-android-sample-kotlin-smoke'),
]);

run('node', [
  path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
  '--dry-run',
  '--abis',
  process.env.SONA_ANDROID_ABIS ?? 'arm64-v8a',
  '--out-dir',
  path.join(repoRoot, 'target', 'uniffi-android-sample-jni-smoke'),
]);

if (!skipGradle) {
  const sampleGradleArgs = [
    ':sample-library:assembleDebug',
    ':sample-library:publishDebugPublicationToSonaAndroidSampleRepository',
    '--quiet',
  ];
  const gradleEnv = {
    SONA_ANDROID_ABIS: process.env.SONA_ANDROID_ABIS ?? defaultAndroidAbi,
  };
  if (runGradleTasks(sampleGradleArgs, gradleEnv)) {
    verifyAndroidSampleAar();
    verifyAndroidSampleMavenPublication();
    runGradleTasks([
      ':consumer-library:assembleDebug',
      '--quiet',
    ], gradleEnv);
    verifyAndroidConsumerAar();
  }
}

console.log('Verified Sona UniFFI Android sample integration');
