import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import YAML from 'yaml';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function readWorkflow(name) {
  const workflowPath = path.join(repoRoot, '.github', 'workflows', name);
  assert.equal(fs.existsSync(workflowPath), true, `missing workflow: ${name}`);
  return YAML.parse(fs.readFileSync(workflowPath, 'utf8'));
}

function workflowStep(job, name) {
  const step = job?.steps?.find((candidate) => candidate.name === name);
  return step ?? assert.fail(`missing workflow step: ${name}`);
}

function workflowNeeds(job) {
  if (Array.isArray(job?.needs)) {
    return job.needs;
  }
  return job?.needs === undefined ? [] : [job.needs];
}

function assertAndroidReleaseDelivery(publishJob, releaseStepName) {
  const verification = workflowStep(publishJob, 'Verify Android release artifacts').run;
  for (const apk of [
    'app-arm64-v8a-debug.apk',
    'app-x86_64-debug.apk',
    'app-arm64-v8a-release-unsigned.apk',
    'app-x86_64-release-unsigned.apk',
  ]) {
    assert.match(verification, new RegExp(apk.replaceAll('.', '\\.'), 'u'));
  }
  assert.match(verification, /find all-artifacts -type f -name "\$apk"/u);
  assert.match(verification, /"\$count" -ne 1/u);
  assert.match(verification, /exit 1/u);

  const artifactPaths = workflowStep(publishJob, 'Collect release artifact paths');
  assert.equal(artifactPaths.id, 'release-artifacts');
  assert.match(
    artifactPaths.run,
    /\{\s+echo "paths<<EOF"\s+find all-artifacts -type f -print\s+echo "updater\.json"\s+echo "EOF"\s+\} >> "\$GITHUB_OUTPUT"/u,
  );

  const release = workflowStep(publishJob, releaseStepName);
  assert.equal(release.with.artifacts, '${{ steps.release-artifacts.outputs.paths }}');
  assert.equal(release.with.artifactErrorsFailBuild, true);
}

test('reusable Android workflow builds and uploads debug and release APKs', () => {
  const workflow = readWorkflow('android-client.yml');
  const workflowCallInput = workflow.on?.workflow_call?.inputs?.artifact_prefix;
  const workflowDispatchInput = workflow.on?.workflow_dispatch?.inputs?.artifact_prefix;

  assert.deepEqual(workflow.permissions, { contents: 'read' });
  assert.deepEqual(workflowCallInput, {
    description: 'Artifact name prefix',
    required: false,
    type: 'string',
    default: 'android-client',
  });
  assert.deepEqual(workflowDispatchInput, workflowCallInput);

  const job = workflow.jobs?.['build-android-client'];
  assert.equal(job?.['runs-on'], 'ubuntu-22.04');
  assert.equal(job?.['timeout-minutes'], 60);

  assert.equal(workflowStep(job, 'Checkout repository').uses, 'actions/checkout@v6');
  assert.equal(workflowStep(job, 'Node.js setup').uses, 'actions/setup-node@v6');
  assert.equal(workflowStep(job, 'Node.js setup').with['node-version'], 24);
  assert.equal(workflowStep(job, 'Java setup').uses, 'actions/setup-java@v5');
  assert.deepEqual(workflowStep(job, 'Java setup').with, {
    distribution: 'temurin',
    'java-version': 17,
  });
  const androidSdkStep = workflowStep(job, 'Android SDK setup');
  assert.equal(androidSdkStep.uses, 'android-actions/setup-android@v3');
  assert.equal(
    androidSdkStep.with.packages,
    'platforms;android-37.0 ndk;29.0.14206865',
  );
  assert.equal(workflowStep(job, 'Rust setup').uses, 'dtolnay/rust-toolchain@stable');
  assert.equal(
    workflowStep(job, 'Rust setup').with.targets,
    'aarch64-linux-android,x86_64-linux-android',
  );

  const androidNdkStep = workflowStep(job, 'Configure Android NDK');
  assert.match(
    androidNdkStep.run,
    /ANDROID_NDK_HOME=\$ANDROID_HOME\/ndk\/29\.0\.14206865/u,
  );
  assert.doesNotMatch(JSON.stringify(job.steps), /yes \| sdkmanager/u);

  const buildStep = workflowStep(job, 'Build and verify Android client');
  assert.deepEqual(buildStep.env, {
    SONA_ANDROID_ABIS: 'arm64-v8a,x86_64',
    SONA_ANDROID_BUILD_RELEASE: 'true',
  });
  assert.equal(buildStep.run, 'pnpm run verify:android-client');

  const expectedUploads = [
    {
      stepName: 'Upload ARM64 APKs',
      artifactName: '${{ inputs.artifact_prefix }}-arm64-v8a',
      apkPaths: [
        'platforms/android/client/app/build/outputs/apk/debug/app-arm64-v8a-debug.apk',
        'platforms/android/client/app/build/outputs/apk/release/app-arm64-v8a-release-unsigned.apk',
      ],
    },
    {
      stepName: 'Upload x86_64 APKs',
      artifactName: '${{ inputs.artifact_prefix }}-x86_64',
      apkPaths: [
        'platforms/android/client/app/build/outputs/apk/debug/app-x86_64-debug.apk',
        'platforms/android/client/app/build/outputs/apk/release/app-x86_64-release-unsigned.apk',
      ],
    },
  ];

  assert.equal(
    job.steps.filter((step) => step.uses === 'actions/upload-artifact@v7').length,
    expectedUploads.length,
  );
  for (const expected of expectedUploads) {
    const upload = workflowStep(job, expected.stepName);
    assert.equal(upload.uses, 'actions/upload-artifact@v7');
    assert.equal(upload.with.name, expected.artifactName);
    assert.deepEqual(upload.with.path.trim().split(/\s+/u), expected.apkPaths);
    assert.equal(upload.with['if-no-files-found'], 'error');
    assert.equal(upload.with['retention-days'], 14);
  }
});

test('PR Android build initializes Java and the SDK in the Rust backend job', () => {
  const workflow = readWorkflow('pr-guardrails.yml');
  const job = workflow.jobs?.['rust-backend'];

  const javaStep = workflowStep(job, 'Java setup');
  assert.equal(javaStep.uses, 'actions/setup-java@v5');
  assert.deepEqual(javaStep.with, {
    distribution: 'temurin',
    'java-version': 17,
  });

  const androidSdkStep = workflowStep(job, 'Android SDK setup');
  assert.equal(androidSdkStep.uses, 'android-actions/setup-android@v3');
  assert.equal(
    androidSdkStep.with.packages,
    'platforms;android-37.0 ndk;29.0.14206865',
  );

  const androidNdkStep = workflowStep(job, 'Configure Android NDK');
  assert.match(
    androidNdkStep.run,
    /ANDROID_NDK_HOME=\$ANDROID_HOME\/ndk\/29\.0\.14206865/u,
  );
  assert.doesNotMatch(JSON.stringify(job.steps), /yes \| sdkmanager/u);
});

test('stable release waits for Android and publishes all APKs', () => {
  const workflow = readWorkflow('release.yml');
  const androidJob = workflow.jobs?.['android-client'];
  const publishJob = workflow.jobs?.['publish-release'];

  assert.equal(androidJob?.uses, './.github/workflows/android-client.yml');
  assert.deepEqual(androidJob?.permissions, { contents: 'read' });
  assert.deepEqual(androidJob?.with, { artifact_prefix: 'release-assets-android' });
  assert.deepEqual(workflowNeeds(publishJob), ['build-tauri', 'android-client']);

  const download = workflowStep(publishJob, 'Download all artifacts');
  assert.equal(download.with.pattern, 'release-assets-*');
  assert.equal(download.with['merge-multiple'], true);

  const releaseNotes = workflowStep(publishJob, 'Extract Release Notes').run;
  assert.match(releaseNotes, /Android \(preview builds\)/u);
  assert.match(releaseNotes, /Debug-signed/u);
  assert.match(releaseNotes, /Unsigned release/u);
  assert.match(releaseNotes, /app-arm64-v8a-debug\.apk/u);
  assert.match(releaseNotes, /app-x86_64-debug\.apk/u);
  assert.match(releaseNotes, /app-arm64-v8a-release-unsigned\.apk/u);
  assert.match(releaseNotes, /app-x86_64-release-unsigned\.apk/u);
  assertAndroidReleaseDelivery(publishJob, 'Create or Update GitHub Release');
  assert.doesNotMatch(JSON.stringify(workflow.jobs), /pnpm run verify:android-client/u);
});

test('nightly release publishes APK assets without advertising them in notes', () => {
  const workflow = readWorkflow('nightly.yml');
  const androidJob = workflow.jobs?.['android-client'];
  const publishJob = workflow.jobs?.['publish-nightly'];

  assert.equal(androidJob?.needs, 'prepare');
  assert.equal(androidJob?.uses, './.github/workflows/android-client.yml');
  assert.deepEqual(androidJob?.permissions, { contents: 'read' });
  assert.deepEqual(androidJob?.with, { artifact_prefix: 'nightly-assets-android' });
  assert.deepEqual(
    workflowNeeds(publishJob),
    ['check-commits', 'prepare', 'build-tauri', 'android-client'],
  );
  assert.match(publishJob.if, /needs\.android-client\.result == 'success'/u);

  const download = workflowStep(publishJob, 'Download all artifacts');
  assert.equal(download.with.pattern, 'nightly-assets-*');
  assert.equal(download.with['merge-multiple'], true);

  const releaseBody = workflowStep(publishJob, 'Create or update nightly release').with.body;
  assert.doesNotMatch(releaseBody, /Android preview APKs/u);
  assert.doesNotMatch(releaseBody, /app-arm64-v8a-debug\.apk/u);
  assert.doesNotMatch(releaseBody, /app-x86_64-debug\.apk/u);
  assert.doesNotMatch(releaseBody, /app-arm64-v8a-release-unsigned\.apk/u);
  assert.doesNotMatch(releaseBody, /app-x86_64-release-unsigned\.apk/u);
  assertAndroidReleaseDelivery(publishJob, 'Create or update nightly release');
  assert.doesNotMatch(JSON.stringify(workflow.jobs), /pnpm run verify:android-client/u);
});
