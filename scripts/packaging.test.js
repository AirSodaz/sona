import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import YAML from 'yaml';

const sourceCache = new Map();
const read = (...segments) => {
  const fp = path.join(repoRoot, ...segments);
  if (!sourceCache.has(fp)) sourceCache.set(fp, fs.readFileSync(fp, 'utf8'));
  return sourceCache.get(fp);
};
const exists = (...segments) => fs.existsSync(path.join(repoRoot, ...segments));

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const desktopCrateSegments = ['platforms', 'desktop'];
const desktopCratePath = (...segments) => path.join(repoRoot, ...desktopCrateSegments, ...segments);
const node = process.execPath;

function makeTempRepo() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-packaging-'));
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'resources', 'cli'), { recursive: true });
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'resources', 'shared_libs'), { recursive: true });
  return root;
}

function writeTauriConfig(root) {
  fs.writeFileSync(
    path.join(root, 'platforms', 'desktop', 'tauri.conf.json'),
    JSON.stringify(
      {
        bundle: {
          resources: ['resources/shared_libs/*', 'resources/cli/*'],
          externalBin: ['binaries/ffmpeg'],
        },
      },
      null,
      2,
    ),
  );
}

function rustFilesUnder(root) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      return rustFilesUnder(fullPath);
    }
    return entry.isFile() && entry.name.endsWith('.rs') ? [fullPath] : [];
  });
}

function readCargoDependencyNames(cargoTomlPath, sectionName) {
  return readCargoDependencyEntries(cargoTomlPath, sectionName)
    .flatMap(({ name, packageName }) => packageName ? [name, packageName] : [name])
    .filter((name, index, names) => names.indexOf(name) === index)
    .sort();
}

function readCargoDependencySpec(cargoTomlPath, sectionName, dependencyName) {
  return readCargoDependencyEntries(cargoTomlPath, sectionName)
    .find(({ name }) => name === dependencyName)?.spec ?? '';
}

function readCargoDependencyEntries(cargoTomlPath, sectionName) {
  const lines = fs.readFileSync(cargoTomlPath, 'utf8').split(/\r?\n/u);
  const entries = [];
  let inSection = false;

  for (const line of lines) {
    const trimmed = line.trim();
    const sectionMatch = trimmed.match(/^\[([^[\]]+)\]$/u);

    if (sectionMatch) {
      inSection = sectionMatch[1] === sectionName || sectionMatch[1].endsWith(`.${sectionName}`);
      continue;
    }
    if (!inSection || trimmed === '' || trimmed.startsWith('#')) {
      continue;
    }

    const dependencyMatch = trimmed.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/u);
    if (dependencyMatch) {
      entries.push({
        name: dependencyMatch[1],
        spec: dependencyMatch[2],
        packageName: dependencyMatch[2].match(/\bpackage\s*=\s*"([^"]+)"/u)?.[1],
      });
    }
  }

  return entries;
}

function assertCargoDependencyVersionAndFeature(
  cargoTomlPath,
  dependencyName,
  expectedVersion,
  expectedFeature,
) {
  const dependencySpec = readCargoDependencySpec(cargoTomlPath, 'dependencies', dependencyName);
  const version = dependencySpec.match(/\bversion\s*=\s*"([^"]+)"/u)?.[1];
  const featureSpec = dependencySpec.match(/\bfeatures\s*=\s*\[([^\]]*)\]/u)?.[1] ?? '';
  const features = [...featureSpec.matchAll(/"([^"]+)"/gu)].map((match) => match[1]);

  assert.equal(version, expectedVersion);
  assert.ok(features.includes(expectedFeature), `${dependencyName} must enable ${expectedFeature}`);
}

function stripRustComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//gu, '')
    .split(/\r?\n/u)
    .map(stripRustLineComment)
    .join('\n');
}

function readRustFunctionBlock(source, functionName) {
  const signature = new RegExp(`\\bfn\\s+${functionName}\\s*\\(`, 'u').exec(source);
  if (!signature) {
    return '';
  }

  const openingBrace = source.indexOf('{', signature.index);
  if (openingBrace === -1) {
    return '';
  }

  let braceDepth = 0;
  for (let index = openingBrace; index < source.length; index += 1) {
    if (source[index] === '{') {
      braceDepth += 1;
    } else if (source[index] === '}') {
      braceDepth -= 1;
      if (braceDepth === 0) {
        return source.slice(signature.index, index + 1);
      }
    }
  }

  return '';
}

function readJavaScriptFunctionBlock(source, functionName) {
  const signature = new RegExp(`\\bfunction\\s+${functionName}\\s*\\(`, 'u').exec(source);
  if (!signature) {
    return '';
  }

  const openingBrace = source.indexOf('{', signature.index);
  if (openingBrace === -1) {
    return '';
  }

  let braceDepth = 0;
  for (let index = openingBrace; index < source.length; index += 1) {
    if (source[index] === '{') {
      braceDepth += 1;
    } else if (source[index] === '}') {
      braceDepth -= 1;
      if (braceDepth === 0) {
        return source.slice(signature.index, index + 1);
      }
    }
  }

  return '';
}

function readKotlinFunctionItem(source, functionName) {
  const normalizedSource = source.replace(/\r\n/gu, '\n');
  const signature = new RegExp(`^([ \\t]*)fun\\s+${functionName}\\b`, 'mu').exec(normalizedSource);
  if (!signature) {
    return '';
  }

  const functionIndent = signature[1].length;
  const remainingSource = normalizedSource.slice(signature.index);
  const lines = remainingSource.split('\n');
  let endOffset = lines[0].length;

  for (let index = 1; index < lines.length; index += 1) {
    const line = lines[index];
    const lineStartOffset = endOffset + 1;
    const trimmed = line.trim();
    const lineIndent = line.match(/^[ \\t]*/u)?.[0].length ?? 0;

    if (trimmed !== '' && lineIndent <= functionIndent) {
      return remainingSource.slice(0, endOffset);
    }
    endOffset = lineStartOffset + line.length;
  }

  return remainingSource;
}

function assertStreamingAsrArchitecture(uniffiCargoPath, streamingBridge) {
  const dependencyNames = readCargoDependencyNames(uniffiCargoPath, 'dependencies');
  const onlineAsrSpec = readCargoDependencySpec(uniffiCargoPath, 'dependencies', 'sona-online-asr');
  const uncommentedStreamingBridge = stripRustComments(streamingBridge);
  const factoryBlock = readRustFunctionBlock(
    uncommentedStreamingBridge,
    'create_online_asr_streaming_session',
  );

  assert.ok(dependencyNames.includes('sona-online-asr'));
  assert.ok(!dependencyNames.includes('sona-local-asr'));
  assert.match(onlineAsrSpec, /\bpath\s*=\s*"\.\.\/online_asr"/u);
  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');
  assert.match(
    uncommentedStreamingBridge,
    /#\[uniffi::export\(foreign\)\]\s*pub trait FfiAsrStreamingObserver\b/u,
  );
  assert.match(
    uncommentedStreamingBridge,
    /#\[derive\(uniffi::Object\)\]\s*pub struct FfiAsrStreamingSession\s*\{[^}]*\binner\s*:\s*Arc<dyn AsrStreamingSession>\s*,?[^}]*\}/u,
  );
  assert.notEqual(factoryBlock, '', 'missing create_online_asr_streaming_session function body');
  assert.match(
    factoryBlock,
    /^\s*VOLCENGINE_DOUBAO_PROVIDER_ID\s*=>\s*sona_online_asr::create_volcengine_streaming_session\s*\(/mu,
  );
  assert.doesNotMatch(uncommentedStreamingBridge, /VolcengineStreamingSession/u);
}

function assertAndroidStreamingSmoke(kotlinSmoke) {
  for (const generatedImport of [
    'FfiAsrInferenceMetric',
    'FfiAsrModelLoadMetric',
    'FfiAsrStreamingObserver',
    'FfiAsrStreamingSession',
    'FfiAsrTranscriptUpdateEvent',
    'createOnlineAsrStreamingSession',
  ]) {
    assert.match(kotlinSmoke, new RegExp(`import uniffi\\.sona_uniffi_bind\\.${generatedImport}`, 'u'));
  }

  assert.match(
    kotlinSmoke,
    /override fun onTranscriptUpdate\(event: FfiAsrTranscriptUpdateEvent\)\s*\{\s*latestTranscriptUpdate\s*=\s*event\b[^}]*\}/u,
  );
  assert.match(
    kotlinSmoke,
    /override fun onModelLoad\(metric: FfiAsrModelLoadMetric\)\s*\{\s*latestModelLoad\s*=\s*metric\b[^}]*\}/u,
  );
  assert.match(
    kotlinSmoke,
    /override fun onLiveInference\(metric: FfiAsrInferenceMetric\)\s*\{\s*latestLiveInference\s*=\s*metric\b[^}]*\}/u,
  );

  const createStreamingSession = readKotlinFunctionItem(kotlinSmoke, 'createStreamingSession');
  assert.notEqual(createStreamingSession, '', 'missing createStreamingSession function');
  assert.match(createStreamingSession, /fun createStreamingSession\(\): FfiAsrStreamingSession/u);
  assert.match(createStreamingSession, /createOnlineAsrStreamingSession\(/u);
  assert.match(createStreamingSession, /instanceId\s*=\s*"android-live-1"/u);
  assert.match(createStreamingSession, /requestJson\s*=\s*streamingRequestJson/u);
  assert.match(createStreamingSession, /observer\s*=\s*RecordingAsrObserver\(\)/u);
  assert.doesNotMatch(createStreamingSession, /\.start\s*\(|\bstart\s*\(\s*\)/u);
}

function scanRustSourcePolicyViolations(root, policies) {
  return rustFilesUnder(root)
    .sort()
    .flatMap((filePath) => {
      const relativePath = path.relative(repoRoot, filePath);
      return fs.readFileSync(filePath, 'utf8')
        .split(/\r?\n/u)
        .flatMap((line, index) => {
          const sourceLine = stripRustLineComment(line);
          return policies
            .filter(([pattern]) => pattern.test(sourceLine))
            .map(([, label]) => `${relativePath}:${index + 1}: ${label}: ${line.trim()}`);
        });
    });
}

function stripRustLineComment(line) {
  return line.replace(/\/\/.*$/u, '');
}

function readWorkflowStep(workflowName, stepName) {
  return readWorkflowBuildTauriSteps(workflowName)
    .find((step) => step.name === stepName) ?? assert.fail(`Missing workflow step: ${workflowName} ${stepName}`);
}

function readWorkflowStepIndex(workflowName, stepName) {
  const index = readWorkflowBuildTauriSteps(workflowName)
    .findIndex((step) => step.name === stepName);

  return index === -1 ? assert.fail(`Missing workflow step: ${workflowName} ${stepName}`) : index;
}

function readWorkflowBuildTauriSteps(workflowName) {
  const workflowPath = path.join(repoRoot, '.github', 'workflows', workflowName);
  const workflow = YAML.parse(fs.readFileSync(workflowPath, 'utf8'));
  const steps = workflow?.jobs?.['build-tauri']?.steps;

  assert.ok(Array.isArray(steps), `${workflowName} must define jobs.build-tauri.steps`);
  return steps;
}

test('tauri bundle verification requires ffmpeg sidecar and shared libraries', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'binaries'), { recursive: true });
  fs.mkdirSync(path.join(root, 'platforms', 'desktop', 'resources', 'shared_libs'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'platforms', 'desktop', 'binaries', 'ffmpeg-x86_64-pc-windows-msvc.exe'),
    'ffmpeg',
  );
  fs.writeFileSync(
    path.join(root, 'platforms', 'desktop', 'resources', 'shared_libs', 'sherpa-onnx-c-api.dll'),
    'sherpa',
  );
  fs.writeFileSync(
    path.join(root, 'platforms', 'desktop', 'resources', 'shared_libs', 'onnxruntime.dll'),
    'onnxruntime',
  );
  fs.writeFileSync(path.join(root, 'platforms', 'desktop', 'resources', 'cli', 'sona-cli.exe'), 'cli');
  writeTauriConfig(root);
  const bundleRoot = path.join(root, 'target', target, 'release', 'bundle', 'nsis');
  fs.mkdirSync(bundleRoot, { recursive: true });
  fs.writeFileSync(path.join(bundleRoot, 'Sona_0.8.0_x64-setup.exe'), 'installer');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'verify-tauri-bundle.js'),
      '--repo-root',
      root,
      '--target',
      target,
      '--bundle-root',
      bundleRoot,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified packaged ffmpeg sidecar/);
  assert.match(result.stdout, /Verified standalone CLI resource/);
  assert.match(result.stdout, /Verified shared libraries/);
});

test('desktop host is a direct platform crate with explicit CLI config', () => {
  const wrapper = read(...desktopCrateSegments, 'scripts', 'tauri.js');

  assert.equal(exists('platforms', 'desktop', 'Cargo.toml'), true);
  assert.equal(exists('src-tauri'), false);
  assert.match(wrapper, /const desktopTauriConfig = path\.join\(repoRoot, 'platforms', 'desktop', 'tauri\.conf\.json'\);/u);
  assert.match(wrapper, /function withDesktopConfig\(commandArgs\)/u);
  assert.match(wrapper, /return \[command, '--config', desktopTauriConfig, \.\.\.commandArgs\.slice\(1\)\];/u);
});

test('desktop frontend and Tauri configuration are colocated', () => {
  const frontend = (...segments) => path.join(repoRoot, 'platforms', 'desktop', 'frontend', ...segments);
  const desktopConfig = JSON.parse(read(...desktopCrateSegments, 'tauri.conf.json'));
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const rootPackage = JSON.parse(read('package.json'));
  const frontendPackage = JSON.parse(fs.readFileSync(frontend('package.json'), 'utf8'));

  assert.equal(fs.existsSync(frontend('src', 'main.tsx')), true);
  assert.equal(fs.existsSync(frontend('public', 'audio-processor.js')), true);
  assert.equal(fs.existsSync(frontend('vite.config.ts')), true);
  assert.equal(fs.existsSync(path.join(repoRoot, 'src')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'public')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'index.html')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'vite.config.ts')), false);
  assert.equal(fs.existsSync(frontend('index.html')), true);
  assert.equal(desktopConfig.build.frontendDist, 'frontend/dist');
  for (const command of [desktopConfig.build.beforeDevCommand, desktopConfig.build.beforeBuildCommand]) {
    assert.equal(typeof command, 'object');
    assert.equal(typeof command.script, 'string');
    assert.equal(command.cwd, 'frontend');
  }
  assert.match(desktopLib, /"frontend\/src\/bindings\.ts"/u);
  assert.ok(frontendPackage.dependencies['@tauri-apps/api']);
  assert.equal(rootPackage.dependencies?.['@tauri-apps/api'], undefined);
  assert.equal(rootPackage.scripts.tauri, 'node platforms/desktop/scripts/tauri.js');
  assert.equal(exists('platforms', 'desktop', 'scripts', 'tauri.js'), true);
  assert.equal(exists('scripts', 'tauri.js'), false);
});

test('desktop tauri crate no longer bundles sona-cli sidecar artifacts', () => {
  const libRs = read(...desktopCrateSegments, 'src', 'lib.rs');
  const cargoToml = read(...desktopCrateSegments, 'Cargo.toml');
  const tauriConfig = read(...desktopCrateSegments, 'tauri.conf.json');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const tauriScript = read(...desktopCrateSegments, 'scripts', 'tauri.js');
  const oldCliSidecarScript = ['prepare', 'cli', 'sidecar'].join('-');
  const oldCliBundleScript = ['verify', 'cli', 'bundle'].join('-');

  assert.equal(exists(...desktopCrateSegments, 'src', 'cli'), false);
  assert.doesNotMatch(cargoToml, /^clap\s*=/mu);
  assert.doesNotMatch(cargoToml, /^clap_complete\s*=/mu);
  assert.doesNotMatch(tauriConfig, /binaries\/sona-cli/u);
  assert.doesNotMatch(tauriScript, new RegExp(oldCliSidecarScript, 'u'));
  assert.doesNotMatch(prWorkflow, new RegExp(`${oldCliSidecarScript}|${oldCliBundleScript}`, 'u'));

  const desktopCliCoreReferences = rustFilesUnder(desktopCratePath('src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /sona_core::cli_|OfflineTranscribeCliOptions/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopCliCoreReferences, []);
});

test('standalone CLI resource staging copies the sona-cli binary into the desktop bundle resources', () => {
  const root = makeTempRepo();
  const target = 'x86_64-pc-windows-msvc';
  const releaseDir = path.join(root, 'target', target, 'release');
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, 'sona-cli.exe'), 'cli');

  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'setup-sona-cli-resource.js'),
      '--repo-root',
      root,
      '--target',
      target,
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Staged standalone CLI resource/);

  const resourceDir = path.join(root, 'platforms', 'desktop', 'resources', 'cli');
  assert.equal(fs.existsSync(path.join(resourceDir, 'sona-cli.exe')), true);
  assert.equal(fs.existsSync(path.join(resourceDir, 'sherpa-onnx-c-api.dll')), false);
  assert.equal(fs.existsSync(path.join(resourceDir, 'onnxruntime.dll')), false);
});

test('standalone CLI resource staging declares macOS universal CLI support', () => {
  const stagingScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'setup-sona-cli-resource.js'),
    'utf8',
  );
  const verifierScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'verify-tauri-bundle.js'),
    'utf8',
  );

  assert.match(stagingScript, /universal-apple-darwin/u);
  assert.match(stagingScript, /aarch64-apple-darwin/u);
  assert.match(stagingScript, /x86_64-apple-darwin/u);
  assert.match(stagingScript, /\blipo\b/u);
  assert.doesNotMatch(verifierScript, /Skipping standalone CLI resource check for universal Apple bundle/u);
});

test('tauri build wrapper builds and stages standalone CLI resources before desktop packaging', () => {
  const tauriScript = read(...desktopCrateSegments, 'scripts', 'tauri.js');

  assert.match(tauriScript, /cargo/u);
  assert.match(tauriScript, /sona-cli/u);
  assert.match(tauriScript, /setup-sona-cli-resource\.js/u);
  assert.ok(
    tauriScript.indexOf('prepareBundleResources(tauriArgs);') < tauriScript.indexOf('spawnSync(tauriBinary'),
    'sona-cli resource staging must happen before invoking the Tauri CLI',
  );
});

test('standalone CLI resolves shared libraries from same-platform desktop resources', () => {
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const cliBuild = read('platforms', 'cli', 'build.rs');
  const cliMain = read('platforms', 'cli', 'src', 'main.rs');
  const runtimeFsCargo = read('adapters', 'runtime_fs', 'Cargo.toml');
  const runtimeFsLib = read('adapters', 'runtime_fs', 'src', 'lib.rs');

  assert.match(cliCargo, /^build\s*=\s*"build\.rs"/mu);
  assert.doesNotMatch(cliCargo, /Win32_System_LibraryLoader/u);
  assert.match(runtimeFsCargo, /Win32_System_LibraryLoader/u);
  assert.match(cliBuild, /SHERPA_ONNX_LIB_DIR/u);
  assert.match(cliBuild, /\$ORIGIN\/\.\.\/shared_libs/u);
  assert.match(cliBuild, /@loader_path\/\.\.\/shared_libs/u);
  assert.match(cliBuild, /delayimp\.lib/u);
  assert.match(cliBuild, /\/DELAYLOAD:sherpa-onnx-c-api\.dll/u);
  assert.ok(
    cliMain.indexOf('sona_runtime_fs::init_cli_shared_library_directory();') < cliMain.indexOf('sona_cli::run_cli_from_args'),
    'sona-cli must configure shared library lookup before invoking ASR-backed commands',
  );
  assert.match(runtimeFsLib, /pub fn init_cli_shared_library_directory/u);
  assert.match(runtimeFsLib, /pub fn init_tauri_shared_library_directory/u);
  assert.match(runtimeFsLib, /SetDllDirectoryW/u);
  assert.match(runtimeFsLib, /\.\.\/shared_libs/u);
  assert.doesNotMatch(cliMain, /SetDllDirectoryW|PCWSTR|OsStrExt|\.\.\/shared_libs/u);
  assert.doesNotMatch(cliMain, /tauri/u);
});

test('standalone CLI keeps local ASR implementation behind its adapter boundary', () => {
  const cliSrcRoot = path.join(repoRoot, 'platforms', 'cli', 'src');
  const adapterPath = path.join(cliSrcRoot, 'asr_adapter.rs');
  const cliLib = fs.readFileSync(path.join(cliSrcRoot, 'lib.rs'), 'utf8');
  const adapter = fs.existsSync(adapterPath) ? fs.readFileSync(adapterPath, 'utf8') : '';
  const localAsrReferencesOutsideAdapter = rustFilesUnder(cliSrcRoot)
    .filter((filePath) => filePath !== adapterPath)
    .flatMap((filePath) => {
      const relativePath = path.relative(repoRoot, filePath);
      return fs.readFileSync(filePath, 'utf8')
        .split(/\r?\n/u)
        .flatMap((line, index) => {
          const sourceLine = stripRustLineComment(line);
          return /sona_local_asr::|LocalBatchAsrAdapter|sherpa_onnx::/u.test(sourceLine)
            ? [`${relativePath}:${index + 1}: ${line.trim()}`]
            : [];
        });
    });

  assert.deepEqual(localAsrReferencesOutsideAdapter, []);
  assert.match(cliLib, /\bmod asr_adapter;/u);
  assert.match(adapter, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
});

test('desktop Tauri DLL directory setup is delegated to the runtime filesystem adapter', () => {
  const tauriLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const runtimeFsLib = read('adapters', 'runtime_fs', 'src', 'lib.rs');

  assert.match(tauriLib, /pub fn init_dll_directory\(\)\s*\{\s*sona_runtime_fs::init_tauri_shared_library_directory\(\);\s*\}/u);
  assert.doesNotMatch(tauriLib, /SetDllDirectoryW|PCWSTR|OsStrExt|resources"\)\.join\("shared_libs/u);
  assert.match(runtimeFsLib, /pub fn tauri_shared_library_directory_candidates/u);
  assert.match(runtimeFsLib, /pub fn init_tauri_shared_library_directory/u);
});

test('generated standalone CLI resources are ignored by git', () => {
  const rootIgnore = read('.gitignore');
  const tauriIgnore = read(...desktopCrateSegments, '.gitignore');
  const ignoreRules = `${rootIgnore}\n${tauriIgnore}`;

  assert.match(ignoreRules, /platforms\/desktop\/resources\/cli\/sona-cli\b/u);
  assert.match(ignoreRules, /platforms\/desktop\/resources\/cli\/sona-cli\.exe\b/u);
  assert.doesNotMatch(ignoreRules, /platforms\/desktop\/resources\/cli\/\.gitkeep/u);
});

test('release workflows stage standalone CLI into the same-platform desktop installer resources', () => {
  assert.equal(exists('scripts', 'package-sona-cli.js'), false);

  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const workflow = read('.github', 'workflows', workflowName);

    assert.match(workflow, /cargo build -p sona-cli --release \$\{\{ matrix\.args \}\}/u);
    assert.match(workflow, /cargo build -p sona-cli --release --target aarch64-apple-darwin/u);
    assert.match(workflow, /cargo build -p sona-cli --release --target x86_64-apple-darwin/u);
    assert.match(workflow, /node scripts\/setup-sona-cli-resource\.js \$\{\{ matrix\.args \}\}/u);
    assert.doesNotMatch(workflow, /Stage standalone CLI resource[\s\S]*matrix\.args != '--target universal-apple-darwin'/u);
    assert.doesNotMatch(workflow, /node scripts\/package-sona-cli\.js/u);
    assert.doesNotMatch(workflow, /target\/\*\*\/release\/sona-cli-\*\.tar\.gz/u);
  }
});

test('release workflows build CLI and desktop installer from the same job resources', () => {
  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const buildCliStep = readWorkflowStep(workflowName, 'Build standalone CLI');
    const buildUniversalCliStep = readWorkflowStep(workflowName, 'Build standalone CLI (macOS universal)');
    const stageCliStep = readWorkflowStep(workflowName, 'Stage standalone CLI resource');
    const buildAppStep = readWorkflowStep(workflowName, 'Build the app');
    const verifyBundleStep = readWorkflowStep(workflowName, 'Verify Tauri bundle and shared libraries');

    assert.equal(buildCliStep.if, "${{ matrix.args != '--target universal-apple-darwin' }}");
    assert.match(buildCliStep.run, /cargo build -p sona-cli --release \$\{\{ matrix\.args \}\}/u);
    assert.equal(buildCliStep.env.LD_LIBRARY_PATH, '${{ env.SHERPA_ONNX_LIB_DIR }}');
    assert.equal(buildUniversalCliStep.if, "${{ matrix.args == '--target universal-apple-darwin' }}");
    assert.match(buildUniversalCliStep.run, /cargo build -p sona-cli --release --target aarch64-apple-darwin/u);
    assert.match(buildUniversalCliStep.run, /cargo build -p sona-cli --release --target x86_64-apple-darwin/u);
    assert.equal(buildUniversalCliStep.env.LD_LIBRARY_PATH, '${{ env.SHERPA_ONNX_LIB_DIR }}');
    assert.equal(stageCliStep.run, 'node scripts/setup-sona-cli-resource.js ${{ matrix.args }}');
    assert.equal(buildAppStep.env.LD_LIBRARY_PATH, '${{ env.SHERPA_ONNX_LIB_DIR }}');
    assert.equal(buildAppStep.env.SONA_SKIP_CLI_RESOURCE_PREP, '1');
    assert.equal(buildAppStep.run, 'node platforms/desktop/scripts/tauri.js build ${{ matrix.args }}');
    assert.equal(verifyBundleStep.run, 'node scripts/verify-tauri-bundle.js ${{ matrix.args }}');
    assert.ok(readWorkflowStepIndex(workflowName, 'Build standalone CLI') < readWorkflowStepIndex(workflowName, 'Stage standalone CLI resource'));
    assert.ok(readWorkflowStepIndex(workflowName, 'Build standalone CLI (macOS universal)') < readWorkflowStepIndex(workflowName, 'Stage standalone CLI resource'));
    assert.ok(readWorkflowStepIndex(workflowName, 'Stage standalone CLI resource') < readWorkflowStepIndex(workflowName, 'Build the app'));
    assert.ok(readWorkflowStepIndex(workflowName, 'Build the app') < readWorkflowStepIndex(workflowName, 'Verify Tauri bundle and shared libraries'));
  }
});

test('CLI documentation describes standalone sona-cli packaging only', () => {
  const readme = read('README.md');
  const readmeZh = read('README.zh-CN.md');
  const cliGuide = read('docs', 'cli.md');
  const cliGuideZh = read('docs', 'cli.zh-CN.md');
  const apiGuide = read('docs', 'api.md');
  const apiGuideZh = read('docs', 'api.zh-CN.md');
  const docs = `${readme}\n${readmeZh}\n${cliGuide}\n${cliGuideZh}\n${apiGuide}\n${apiGuideZh}`;

  assert.match(readme, /cargo run -p sona-cli -- transcribe/u);
  assert.match(readmeZh, /cargo run -p sona-cli -- transcribe/u);
  assert.match(readme, /pnpm run build:sona-cli/u);
  assert.match(readmeZh, /pnpm run build:sona-cli/u);
  assert.match(cliGuide, /### `transcribe`/u);
  assert.match(cliGuide, /sona-cli transcribe/u);

  assert.doesNotMatch(docs, /main desktop executable/u);
  assert.doesNotMatch(docs, /Sona\.exe transcribe/u);
  assert.doesNotMatch(docs, /Contents\/MacOS\/Sona transcribe/u);
  assert.doesNotMatch(docs, /cargo run --manifest-path src-tauri\/Cargo\.toml/u);
  assert.doesNotMatch(docs, /not part of the current standalone surface yet/u);
  assert.doesNotMatch(docs, /\bsona serve\b/u);
  assert.match(docs, /\bsona-cli serve\b/u);
});

test('core crate does not keep sona-cli config template surface', () => {
  const coreLib = read('core', 'src', 'lib.rs');

  assert.equal(exists('core', 'src', 'cli_config.rs'), false);
  assert.equal(exists('core', 'tests', 'cli_config.rs'), false);
});

test('core crate exposes serve runtime without cli runtime surface', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreRuntime = read('core', 'src', 'runtime', 'mod.rs');

  assert.match(coreLib, /^pub mod runtime;/mu);
  assert.match(coreRuntime, /^pub mod serve;/mu);
  assert.equal(exists('core', 'src', 'cli_runtime.rs'), false);
  assert.equal(exists('core', 'tests', 'cli_runtime.rs'), false);
});

test('core path port default API does not expose test adapters', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const corePaths = read('core', 'src', 'runtime', 'paths.rs');
  const corePathPort = read('core', 'src', 'ports', 'path.rs');
  const tauriPlatformPaths = read(...desktopCrateSegments, 'src', 'platform', 'paths.rs');

  assert.doesNotMatch(coreCargo, /^test-utils\s*=/mu);
  assert.match(
    tauriCargo,
    /^sona-core\s*=\s*\{\s*path = "\.\.\/\.\.\/core", features = \["specta"\]\s*\}/mu,
  );
  assert.doesNotMatch(tauriCargo, /^sona-core\s*=\s*\{\s*path = "\.\.\/\.\.\/core", features = \["test-utils"\]\s*\}/mu);
  assert.match(corePaths, /^pub use crate::ports::path::\{PathKind, PathProvider\};$/mu);
  assert.doesNotMatch(corePaths, /MockPathProvider/u);
  assert.doesNotMatch(corePathPort, /MockPathProvider/u);
  assert.match(tauriPlatformPaths, /#\[cfg\(test\)\]\s*pub struct MockPathProvider/u);
  assert.match(tauriPlatformPaths, /impl PathProvider for MockPathProvider/u);
});

test('core crate dependency surface remains domain-only', () => {
  const coreCargoPath = path.join(repoRoot, 'core', 'Cargo.toml');
  const runtimeDependencyNames = [
    'axum',
    'base64',
    'bzip2',
    'cpal',
    'directories',
    'dirs',
    'glob',
    'hex',
    'hound',
    'hyper',
    'rand',
    'reqwest',
    'rusqlite',
    'sha2',
    'sherpa-onnx',
    'sqlx',
    'tar',
    'tauri',
    'tokio',
    'ureq',
    'uuid',
    'walkdir',
    'zip',
  ];

  const dependencies = readCargoDependencyNames(coreCargoPath, 'dependencies');
  const runtimeDependencies = dependencies.filter((name) => runtimeDependencyNames.includes(name));

  assert.deepEqual(runtimeDependencies, []);
  assert.doesNotMatch(readCargoDependencySpec(coreCargoPath, 'dependencies', 'chrono'), /\bclock\b/u);
});

test('core source does not call adapter runtime side effects directly', () => {
  const violations = scanRustSourcePolicyViolations(path.join(repoRoot, 'core', 'src'), [
    [/\.exists\(/u, 'filesystem path probing'],
    [/\.metadata\(/u, 'filesystem metadata'],
    [/\bstd::env\b/u, 'environment access'],
    [/\bstd::fs\b/u, 'filesystem access'],
    [/\bstd::process\b/u, 'process spawning'],
    [/\btokio::fs\b/u, 'async filesystem access'],
    [/\btokio::process\b/u, 'async process spawning'],
    [/\breqwest::/u, 'HTTP client'],
    [/\brusqlite::/u, 'SQLite access'],
    [/\bsqlx::/u, 'SQL access'],
    [/\btauri::/u, 'desktop framework'],
    [/\bsherpa_onnx::/u, 'local ASR runtime'],
    [/\bcpal::/u, 'audio device runtime'],
    [/\bhound::/u, 'WAV file IO'],
    [/\bwalkdir::/u, 'filesystem walking'],
    [/\brand::/u, 'random generation'],
    [/\buuid::|Uuid::new_v4/u, 'ID generation'],
    [/\bSystemTime::now\b/u, 'system clock'],
    [/\bInstant::now\b/u, 'monotonic clock'],
    [/\b(?:chrono::)?(?:Utc|Local)::now\b/u, 'wall clock'],
  ]);

  assert.deepEqual(violations, []);
});

test('shared api server invokes local batch ASR through the core transcriber port', () => {
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const tauriServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const apiCargo = read('adapters', 'api_server', 'Cargo.toml');
  const apiServer = read('adapters', 'api_server', 'src', 'lib.rs');

  assert.match(tauriCargo, /^sona-local-asr\s*=\s*\{ path = "\.\.\/\.\.\/adapters\/local_asr" \}/mu);
  assert.match(apiCargo, /^sona-local-asr\s*=\s*\{\s*path = "\.\.\/local_asr" \}/mu);
  assert.match(apiServer, /use sona_core::ports::asr::\{[\s\S]*BatchTranscriber/u);
  assert.match(apiServer, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.match(apiServer, /\.transcribe\(plan\)/u);
  assert.match(tauriServer, /use sona_api_server::\{[\s\S]*start_api_server_runtime/u);
  assert.doesNotMatch(tauriServer, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.doesNotMatch(apiServer, /run_offline_transcription/u);
  assert.doesNotMatch(apiServer, /use crate::integrations::asr::transcribe_batch_with_progress;/u);
  assert.doesNotMatch(apiServer, /LocalSherpaAdapter::offline_plan_to_batch_request/u);
});

test('webdav network implementation lives in a dedicated adapter crate', () => {
  const workspaceCargo = read('Cargo.toml');
  const coreCargo = read('core', 'Cargo.toml');
  const coreLib = read('core', 'src', 'lib.rs');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemRs = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformWebdavPath = desktopCratePath('src', 'platform', 'webdav.rs');
  const webdavAdapter = read('adapters', 'webdav', 'src', 'lib.rs');
  const webdavCargo = read('adapters', 'webdav', 'Cargo.toml');

  assert.match(workspaceCargo, /"adapters\/webdav"/u);
  assert.match(tauriCargo, /sona-webdav\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/webdav" \}/u);
  assert.match(webdavCargo, /^roxmltree\s*=/mu);
  assert.match(webdavCargo, /^urlencoding\s*=/mu);
  assert.match(webdavCargo, /^url\s*=/mu);
  assert.doesNotMatch(webdavCargo, /^sona-core\s*=/mu);
  assert.equal(fs.existsSync(platformWebdavPath), true);
  const platformWebdav = fs.readFileSync(platformWebdavPath, 'utf8');
  assert.match(platformMod, /^pub mod webdav;/mu);
  assert.match(platformWebdav, /pub use sona_webdav::\{[\s\S]*RemoteBackupEntry/u);
  assert.match(platformWebdav, /pub use sona_webdav::\{[\s\S]*WebDavConfigPayload/u);
  assert.match(platformWebdav, /pub use sona_webdav::\{[\s\S]*WebDavConnectionResult/u);
  assert.match(platformWebdav, /pub async fn test_connection/u);
  assert.match(platformWebdav, /pub async fn list_backups/u);
  assert.match(platformWebdav, /pub async fn upload_backup/u);
  assert.match(platformWebdav, /pub async fn download_backup/u);
  assert.match(platformWebdav, /sona_webdav::webdav_test_connection/u);
  assert.match(platformWebdav, /sona_webdav::webdav_download_backup/u);
  assert.doesNotMatch(systemRs, /sona_webdav/u);
  assert.match(systemRs, /crate::platform::webdav::test_connection\(config\)\.await/u);
  assert.match(systemRs, /crate::platform::webdav::list_backups\(config\)\.await/u);
  assert.match(systemRs, /crate::platform::webdav::upload_backup\(config, local_archive_path\)\.await/u);
  assert.match(systemRs, /crate::platform::webdav::download_backup\(config, href, output_path\)\.await/u);
  assert.doesNotMatch(systemRs, /sona_core::webdav/u);
  assert.doesNotMatch(systemRs, /sona_core::webdav::webdav_/u);
  assert.doesNotMatch(coreCargo, /^roxmltree\s*=/mu);
  assert.doesNotMatch(coreCargo, /^urlencoding\s*=/mu);
  assert.doesNotMatch(tauriCargo, /^urlencoding\s*=/mu);
  assert.equal(exists('core', 'src', 'webdav.rs'), false);
  assert.match(webdavAdapter, /pub struct WebDavConfigPayload/u);
  assert.match(webdavAdapter, /pub struct RemoteBackupEntry/u);
  assert.match(webdavAdapter, /pub struct WebDavConnectionResult/u);
  assert.match(webdavAdapter, /pub fn parse_propfind_entries/u);
  assert.match(webdavAdapter, /pub async fn webdav_test_connection/u);
  assert.match(webdavAdapter, /pub async fn webdav_list_backups/u);
  assert.match(webdavAdapter, /pub async fn webdav_upload_backup/u);
  assert.match(webdavAdapter, /pub async fn webdav_download_backup/u);
});

test('tar.bz2 archive filesystem operations live in archive adapter', () => {
  const workspaceCargo = read('Cargo.toml');
  const coreCargo = read('core', 'Cargo.toml');
  const coreLib = read('core', 'src', 'lib.rs');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformArchivePath = desktopCratePath('src', 'platform', 'archive.rs');
  const archiveCommand = read(...desktopCrateSegments, 'src', 'commands', 'archive.rs');
  const archiveAdapter = read('adapters', 'archive', 'src', 'lib.rs');

  assert.match(workspaceCargo, /"adapters\/archive"/u);
  assert.match(tauriCargo, /sona-archive\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/archive" \}/u);
  assert.doesNotMatch(tauriCargo, /^bzip2\s*=/mu);
  assert.doesNotMatch(tauriCargo, /^tar\s*=/mu);
  assert.doesNotMatch(coreCargo, /^bzip2\s*=/mu);
  assert.doesNotMatch(coreCargo, /^tar\s*=/mu);
  assert.doesNotMatch(coreLib, /^pub mod archive;/mu);
  assert.equal(exists('core', 'src', 'archive.rs'), false);
  assert.equal(fs.existsSync(platformArchivePath), true);
  const platformArchive = fs.readFileSync(platformArchivePath, 'utf8');
  assert.match(platformMod, /^pub mod archive;/mu);
  assert.match(platformArchive, /const EXTRACT_PROGRESS_EVENT: &str = "extract-progress"/u);
  assert.match(platformArchive, /pub async fn extract_tar_bz2/u);
  assert.match(platformArchive, /pub async fn create_tar_bz2/u);
  assert.match(platformArchive, /tauri::async_runtime::spawn_blocking/u);
  assert.match(platformArchive, /sona_archive::extract_tar_bz2/u);
  assert.match(platformArchive, /sona_archive::create_tar_bz2/u);
  assert.match(platformArchive, /app\.emit\(EXTRACT_PROGRESS_EVENT, path_str\)/u);
  assert.match(archiveCommand, /crate::platform::archive::extract_tar_bz2\(app, archive_path, target_dir\)\.await/u);
  assert.match(archiveCommand, /crate::platform::archive::create_tar_bz2\(source_dir, archive_path\)\.await/u);
  assert.doesNotMatch(archiveCommand, /sona_archive/u);
  assert.doesNotMatch(archiveCommand, /tauri::async_runtime::spawn_blocking/u);
  assert.doesNotMatch(archiveCommand, /EXTRACT_PROGRESS_EVENT/u);
  assert.doesNotMatch(archiveCommand, /sona_core::archive/u);
  assert.match(archiveAdapter, /pub fn extract_tar_bz2/u);
  assert.match(archiveAdapter, /pub fn create_tar_bz2/u);
});

test('model download runtime implementation lives in a dedicated adapter crate', () => {
  const workspaceCargo = read('Cargo.toml');
  const coreCargo = read('core', 'Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformModelDownloadsPath = desktopCratePath('src', 'platform', 'model_downloads.rs');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const cliModels = read('platforms', 'cli', 'src', 'models.rs');
  const desktopDownloads = fs.readFileSync(
    desktopCratePath('src', 'commands', 'downloads.rs'),
    'utf8',
  );
  const coreModelDownloads = read('core', 'src', 'models', 'downloads.rs');
  const coreModelCatalog = read('core', 'src', 'models', 'catalog.rs');
  const adapterLib = read('adapters', 'model_downloads', 'src', 'lib.rs');
  const adapterModels = read('adapters', 'model_downloads', 'src', 'models.rs');
  const adapterDownloads = read('adapters', 'model_downloads', 'src', 'downloads.rs');

  assert.match(workspaceCargo, /"adapters\/model_downloads"/u);
  assert.match(tauriCargo, /sona-model-downloads\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/model_downloads" \}/u);
  assert.match(cliCargo, /sona-model-downloads\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/model_downloads" \}/u);
  assert.match(cliModels, /use sona_model_downloads::\{download_model, installed_model_is_valid, remove_model_install_path\}/u);
  assert.equal(fs.existsSync(platformModelDownloadsPath), true);
  const platformModelDownloads = fs.readFileSync(platformModelDownloadsPath, 'utf8');
  assert.match(platformMod, /^pub mod model_downloads;/mu);
  assert.match(desktopLib, /crate::platform::model_downloads::DownloadState::new\(\)/u);
  assert.match(platformModelDownloads, /pub struct DownloadState/u);
  assert.match(platformModelDownloads, /DownloadClient/u);
  assert.doesNotMatch(platformModelDownloads, /pub downloads:/u);
  assert.doesNotMatch(platformModelDownloads, /pub client:/u);
  assert.match(platformModelDownloads, /fn client\(&self\) -> &DownloadClient/u);
  assert.match(platformModelDownloads, /async fn insert_download/u);
  assert.match(platformModelDownloads, /async fn remove_download/u);
  assert.match(platformModelDownloads, /async fn notify_download/u);
  assert.match(platformModelDownloads, /const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress"/u);
  assert.match(
    platformModelDownloads,
    /state\s*\.client\(\)\s*\.download_file\(&url, &temp_path, notify, Some\(progress_cb\)\)/u,
  );
  assert.match(platformModelDownloads, /complete_download_file/u);
  assert.match(platformModelDownloads, /temporary_download_path/u);
  assert.match(desktopDownloads, /crate::platform::model_downloads::cancel_download\(state, id\)\.await/u);
  assert.match(desktopDownloads, /crate::platform::model_downloads::has_active_downloads\(state\)\.await/u);
  assert.match(desktopDownloads, /crate::platform::model_downloads::download_file\(/u);
  assert.doesNotMatch(desktopDownloads, /sona_model_downloads/u);
  assert.doesNotMatch(desktopDownloads, /DownloadClient/u);
  assert.doesNotMatch(desktopDownloads, /DOWNLOAD_PROGRESS_EVENT/u);
  assert.doesNotMatch(desktopDownloads, /temporary_download_path|complete_download_file/u);
  assert.doesNotMatch(desktopDownloads, /reqwest::Client|Client::builder|adapter_download_file/u);
  assert.doesNotMatch(coreModelDownloads, /pub async fn download_model/u);
  assert.doesNotMatch(coreModelDownloads, /reqwest|tokio::fs|sha256_file|tar::|bzip2::/u);
  assert.doesNotMatch(coreModelCatalog, /std::fs::/u);
  assert.doesNotMatch(coreModelCatalog, /pub fn remove_model_install_path/u);
  assert.doesNotMatch(coreCargo, /^hex\s*=/mu);
  assert.doesNotMatch(coreCargo, /^sha2\s*=/mu);
  assert.equal(exists('core', 'src', 'downloads.rs'), false);
  assert.match(adapterLib, /pub use downloads::/u);
  assert.match(adapterLib, /DownloadClient/u);
  assert.match(adapterLib, /remove_model_install_path/u);
  assert.match(adapterDownloads, /pub struct DownloadClient/u);
  assert.match(adapterDownloads, /impl DownloadClient/u);
  assert.match(adapterDownloads, /user_agent\("Sona\/1\.0"\)/u);
  assert.match(adapterDownloads, /pub async fn download_file/u);
  assert.match(adapterModels, /pub fn remove_model_install_path/u);
});

test('runtime filesystem operations live in a dedicated adapter crate', () => {
  const workspaceCargo = read('Cargo.toml');
  const coreCargo = read('core', 'Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const runtimeFsLib = read('adapters', 'runtime_fs', 'src', 'lib.rs');
  const cliInitConfig = read('platforms', 'cli', 'src', 'init_config.rs');
  const cliModels = read('platforms', 'cli', 'src', 'models.rs');
  const cliTranscribe = read('platforms', 'cli', 'src', 'transcribe.rs');
  const coreRuntimeConfig = read('core', 'src', 'runtime', 'config.rs');
  const coreTranscribeRuntime = read('core', 'src', 'transcription', 'runtime.rs');
  const corePaths = read('core', 'src', 'runtime', 'paths.rs');
  const coreRuntime = read('core', 'src', 'runtime', 'environment.rs');
  const coreRecoveryNormalization = fs.readFileSync(
    path.join(repoRoot, 'core', 'src', 'recovery', 'normalization.rs'),
    'utf8',
  );
  const coreProject = read('core', 'src', 'project', 'mod.rs');
  const corePresetModels = read('core', 'src', 'models', 'preset_models.rs');
  const coreModelCatalog = read('core', 'src', 'models', 'catalog.rs');
  const coreFsPort = read('core', 'src', 'ports', 'fs.rs');
  const coreFileUtils = read('core', 'src', 'runtime', 'file_utils.rs');
  const sqliteProject = read('adapters', 'sqlite', 'src', 'project.rs');
  const sqliteLegacyMigration = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'sqlite', 'src', 'legacy_migration.rs'),
    'utf8',
  );
  const tauriRecoveryRepository = fs.readFileSync(
    desktopCratePath('src', 'platform', 'recovery_repository.rs'),
    'utf8',
  );
  const desktopRuntimeStatus = fs.readFileSync(
    desktopCratePath('src', 'platform', 'runtime_status.rs'),
    'utf8',
  );
  const desktopPresetModels = fs.readFileSync(
    desktopCratePath('src', 'platform', 'preset_models.rs'),
    'utf8',
  );
  const desktopDiagnostics = fs.readFileSync(
    desktopCratePath('src', 'platform', 'diagnostics.rs'),
    'utf8',
  );

  assert.match(workspaceCargo, /"adapters\/runtime_fs"/u);
  assert.match(tauriCargo, /sona-runtime-fs\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/runtime_fs" \}/u);
  assert.match(cliCargo, /sona-runtime-fs\s*=\s*\{\s*path = "\.\.\/\.\.\/adapters\/runtime_fs" \}/u);
  assert.match(
    prWorkflow,
    /cargo test -p sona-core -p sona-api-server -p sona-archive -p sona-export -p sona-local-asr -p sona-media-detector -p sona-model-downloads -p sona-online-llm -p sona-online-asr -p sona-runtime-fs -p sona-webdav -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
  );

  assert.doesNotMatch(coreCargo, /^glob\s*=/mu);
  assert.doesNotMatch(coreCargo, /^walkdir\s*=/mu);
  assert.doesNotMatch(coreRuntimeConfig, /std::fs::read_to_string/u);
  assert.doesNotMatch(coreTranscribeRuntime, /glob::|walkdir::|\.is_file\(\)|\.exists\(\)/u);
  assert.doesNotMatch(corePaths, /std::env|\.exists\(\)/u);
  assert.doesNotMatch(coreRuntime, /std::fs::metadata/u);
  assert.doesNotMatch(coreRecoveryNormalization, /std::fs::metadata/u);
  assert.doesNotMatch(coreRecoveryNormalization, /FsSourcePathStatusProvider/u);
  assert.doesNotMatch(coreRecoveryNormalization, /SystemTime::now|pub fn now_ms/u);
  assert.match(coreRecoveryNormalization, /snapshot_from_items_with_timestamp/u);
  assert.match(coreRecoveryNormalization, /snapshot_from_value_with_source_paths_at/u);
  assert.doesNotMatch(coreProject, /current_time_millis|crate::(?:runtime::)?file_utils/u);
  assert.match(coreProject, /normalize_project_value_with_timestamp/u);
  assert.match(coreProject, /normalize_project_record_for_import_with_timestamp/u);
  assert.doesNotMatch(corePresetModels, /\.metadata\(\)|\.exists\(\)|is_preset_model_installed_at/u);
  assert.doesNotMatch(coreModelCatalog, /is_preset_model_installed_at/u);
  assert.doesNotMatch(coreFsPort, /RealFileSystem|std::fs::/u);
  assert.doesNotMatch(coreFileUtils, /FileSystem|write_json_pretty_atomic_with|remove_path_if_exists_with/u);
  assert.match(runtimeFsLib, /pub fn ensure_directory_exists/u);
  assert.match(desktopRuntimeStatus, /sona_runtime_fs::ensure_directory_exists\(&log_dir\)/u);
  assert.doesNotMatch(desktopRuntimeStatus, /std::fs::create_dir_all/u);
  assert.match(desktopPresetModels, /sona_runtime_fs::ensure_directory_exists\(&models_dir\)/u);
  assert.doesNotMatch(desktopPresetModels, /std::fs::create_dir_all/u);
  assert.match(desktopDiagnostics, /sona_runtime_fs::ensure_directory_exists\(&models_dir\)/u);
  assert.doesNotMatch(desktopDiagnostics, /std::fs::create_dir_all/u);
  assert.doesNotMatch(coreFileUtils, /SystemTime::now|current_time_millis/u);
  assert.doesNotMatch(
    sqliteProject,
    /sona_core::(?:runtime::)?file_utils::current_time_millis/u,
  );
  assert.match(sqliteLegacyMigration, /normalize_project_value_with_timestamp/u);
  assert.match(tauriRecoveryRepository, /fn now_ms\(\) -> u64/u);
  assert.match(tauriRecoveryRepository, /crate::platform::time::unix_timestamp_millis\(\)/u);
  assert.doesNotMatch(tauriRecoveryRepository, /SystemTime::now|UNIX_EPOCH/u);
  assert.match(tauriRecoveryRepository, /sona_runtime_fs::ensure_directory_exists\(&recovery_dir\)/u);
  assert.doesNotMatch(tauriRecoveryRepository, /fs::create_dir_all|std::fs::create_dir_all/u);
  assert.match(tauriRecoveryRepository, /snapshot_from_items_with_timestamp/u);
  assert.match(tauriRecoveryRepository, /snapshot_from_value_with_source_paths_at/u);
  assert.doesNotMatch(
    runtimeFsLib,
    /sona_core::(?:runtime::)?file_utils::\{[^}]*write_json_pretty_atomic_with/u,
  );
  assert.doesNotMatch(
    runtimeFsLib,
    /sona_core::(?:runtime::)?file_utils::\{[^}]*remove_path_if_exists_with/u,
  );

  assert.match(runtimeFsLib, /pub fn load_transcribe_config_file/u);
  assert.match(runtimeFsLib, /pub fn write_cli_config_template_file/u);
  assert.match(runtimeFsLib, /pub fn path_exists/u);
  assert.match(cliInitConfig, /sona_runtime_fs::write_cli_config_template_file\(&path, &content, args\.force\)/u);
  assert.doesNotMatch(cliInitConfig, /std::fs|\bfs::write\(|create_dir_all|\.exists\(\)/u);
  assert.match(cliModels, /sona_runtime_fs::path_exists\(&install_path\)/u);
  assert.match(cliModels, /sona_runtime_fs::path_exists\(&resolved\.install_path\)/u);
  assert.doesNotMatch(cliModels, /\.exists\(\)/u);
  assert.match(runtimeFsLib, /pub fn write_transcript_output_file/u);
  assert.match(cliTranscribe, /sona_runtime_fs::write_transcript_output_file\(&path, &output\)/u);
  assert.doesNotMatch(cliTranscribe, /fs::write\(/u);
  assert.match(runtimeFsLib, /pub fn load_serve_config_file/u);
  assert.match(runtimeFsLib, /pub fn load_legacy_settings_app_config/u);
  assert.match(runtimeFsLib, /pub fn default_desktop_models_dir/u);
  assert.match(runtimeFsLib, /pub fn resolve_runtime_path_status/u);
  assert.match(runtimeFsLib, /pub struct FsSourcePathStatusProvider/u);
  assert.match(runtimeFsLib, /pub fn resolve_batch_input_source/u);
  assert.match(runtimeFsLib, /pub fn plan_batch_output_files/u);
  assert.match(runtimeFsLib, /pub fn is_preset_model_installed_at/u);
  assert.match(runtimeFsLib, /pub struct RealFileSystem/u);
  assert.match(runtimeFsLib, /fn write_binary_atomic/u);
  assert.match(runtimeFsLib, /fn replace_path_atomically/u);
});

test('pr guardrails run adapter tests with core bindings and standalone CLI', () => {
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.match(
    prWorkflow,
    /cargo test -p sona-core -p sona-api-server -p sona-archive -p sona-export -p sona-local-asr -p sona-media-detector -p sona-model-downloads -p sona-online-llm -p sona-online-asr -p sona-runtime-fs -p sona-webdav -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
  );
  assert.match(prWorkflow, /cargo test -p sona-core --test preset_models/u);
  assert.match(prWorkflow, /rustup target add aarch64-linux-android/u);
  assert.match(prWorkflow, /yes \| sdkmanager --licenses/u);
  assert.match(prWorkflow, /sdkmanager "ndk;29\.0\.14206865"/u);
  assert.match(prWorkflow, /ANDROID_NDK_HOME=\$ANDROID_HOME\/ndk\/29\.0\.14206865/u);
  assert.match(prWorkflow, /SONA_ANDROID_ABIS:\s*arm64-v8a/u);
  assert.match(prWorkflow, /pnpm run verify:android-uniffi:gradle/u);
  assert.doesNotMatch(prWorkflow, /core::preset_models::tests/u);
});

test('android uniffi sample publishes a consumable local Maven artifact', () => {
  const sampleLibraryGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts'),
    'utf8',
  );
  const verifierScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'verify-uniffi-android-sample.js'),
    'utf8',
  );
  const androidReadme = read('platforms', 'android', 'README.md');

  assert.match(sampleLibraryGradle, /id\("maven-publish"\)/u);
  assert.match(sampleLibraryGradle, /from\(components\["debug"\]\)/u);
  assert.match(sampleLibraryGradle, /groupId\s*=\s*"com\.sona"/u);
  assert.match(sampleLibraryGradle, /artifactId\s*=\s*"sona-uniffi-bindings"/u);
  assert.match(sampleLibraryGradle, /version\s*=\s*project\.version\.toString\(\)/u);
  assert.match(sampleLibraryGradle, /url\s*=\s*uri\(layout\.buildDirectory\.dir\("repo"\)\)/u);

  assert.match(verifierScript, /:sample-library:publishDebugPublicationToSonaAndroidSampleRepository/u);
  assert.match(verifierScript, /verifyAndroidSampleMavenPublication/u);
  assert.match(verifierScript, /sona-uniffi-bindings-\$\{samplePublicationVersion\}\.module/u);
  assert.match(verifierScript, /verifyAndroidSampleGradleModuleMetadata/u);
  assert.match(verifierScript, /net\.java\.dev\.jna:jna/u);
  assert.match(verifierScript, /org\.jetbrains\.kotlinx:kotlinx-coroutines-core/u);

  assert.match(androidReadme, /publishDebugPublicationToSonaAndroidSampleRepository/u);
});

test('android uniffi sample includes a separate Maven consumer module', () => {
  const settingsGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'settings.gradle.kts'),
    'utf8',
  );
  const consumerGradlePath = path.join(
    repoRoot,
    'platforms',
    'android',
    'sample-consumer',
    'consumer-library',
    'build.gradle.kts',
  );
  const consumerSmokePath = path.join(
    repoRoot,
    'platforms',
    'android',
    'sample-consumer',
    'consumer-library',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'uniffi',
    'consumer',
    'SonaUniffiConsumerSmoke.kt',
  );
  const verifierScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'verify-uniffi-android-sample.js'),
    'utf8',
  );
  const androidReadme = read('platforms', 'android', 'README.md');

  assert.match(settingsGradle, /include\(":consumer-library"\)/u);
  assert.match(settingsGradle, /maven\s*\{\s*url\s*=\s*uri\("sample-library\/build\/repo"\)/u);

  assert.equal(fs.existsSync(consumerGradlePath), true);
  const consumerGradle = fs.readFileSync(consumerGradlePath, 'utf8');
  assert.match(consumerGradle, /id\("com\.android\.library"\)/u);
  assert.match(consumerGradle, /namespace\s*=\s*"com\.sona\.uniffi\.consumer"/u);
  assert.doesNotMatch(consumerGradle, /sona-uniffi-bindings\.gradle\.kts/u);
  assert.doesNotMatch(consumerGradle, /generateSonaUniffiKotlin|buildSonaUniffiAndroidLibraries/u);

  assert.equal(fs.existsSync(consumerSmokePath), true);
  const consumerSmoke = fs.readFileSync(consumerSmokePath, 'utf8');
  assert.match(consumerSmoke, /import com\.sona\.uniffi\.sample\.SonaUniffiSmoke/u);
  assert.match(consumerSmoke, /import uniffi\.sona_uniffi_bind\.defaultConfigJson/u);

  assert.match(verifierScript, /:consumer-library:assembleDebug/u);
  assert.match(verifierScript, /verifyAndroidConsumerAar/u);
  assert.match(verifierScript, /com\/sona\/uniffi\/consumer\/SonaUniffiConsumerSmoke/u);

  assert.match(androidReadme, /consumer-library/u);
});

test('android uniffi JNI staging clears stale ABI outputs before packaging', () => {
  const buildAndroidLibsScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
    'utf8',
  );
  const androidBindingsGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  assert.match(androidBindingsGradle, /jniLibs\.directories\.add\(generatedJniLibsDir\.get\(\)\.asFile\.path\)/u);
  assert.match(
    buildAndroidLibsScript,
    /function prepareOutputDirectory\(outDir\)\s*\{\s*fs\.rmSync\(outDir,\s*\{\s*recursive:\s*true,\s*force:\s*true\s*\}\);\s*fs\.mkdirSync\(outDir,\s*\{\s*recursive:\s*true\s*\}\);\s*\}/u,
  );
  assert.match(buildAndroidLibsScript, /if \(!dryRun && !printLinkerEnv\)\s*\{\s*prepareOutputDirectory\(outDir\);\s*\}/u);
});

test('dashboard and diagnostics clocks are supplied by desktop adapters', () => {
  const coreDiagnostics = read('core', 'src', 'runtime', 'diagnostics.rs');
  const coreCargo = read('core', 'Cargo.toml');
  const tauriDiagnostics = fs.readFileSync(
    desktopCratePath('src', 'platform', 'diagnostics.rs'),
    'utf8',
  );
  const coreDashboardService = fs.readFileSync(
    path.join(repoRoot, 'core', 'src', 'dashboard', 'service.rs'),
    'utf8',
  );
  const tauriDashboard = read(...desktopCrateSegments, 'src', 'app', 'dashboard.rs');

  assert.match(coreDiagnostics, /pub fn build_diagnostics_core_snapshot_at/u);
  assert.doesNotMatch(coreDiagnostics, /pub fn build_diagnostics_core_snapshot\(/u);
  assert.doesNotMatch(coreDiagnostics, /Utc::now|chrono::Utc::now|now_iso_like/u);
  assert.match(tauriDiagnostics, /build_diagnostics_core_snapshot_at/u);
  assert.match(tauriDiagnostics, /crate::platform::time::utc_now_rfc3339_millis\(\)/u);
  assert.doesNotMatch(tauriDiagnostics, /chrono::Utc::now\(\)/u);

  assert.match(coreDashboardService, /pub struct DashboardSnapshotTime/u);
  assert.match(coreDashboardService, /build_snapshot_at/u);
  assert.doesNotMatch(coreDashboardService, /pub async fn build_snapshot\(/u);
  assert.doesNotMatch(coreDashboardService, /Utc::now|chrono::Utc::now|chrono::Local|\bLocal\b/u);
  assert.match(tauriDashboard, /crate::platform::time::dashboard_snapshot_time_now\(\)/u);
  assert.doesNotMatch(tauriDashboard, /DashboardSnapshotTime/u);
  assert.doesNotMatch(tauriDashboard, /chrono::Utc::now|chrono::Local|chrono::SecondsFormat|with_timezone\(&chrono::Local\)/u);
  assert.doesNotMatch(coreCargo, /chrono = \{[^}]*"clock"/u);
});

test('desktop app settings owns minimize-to-tray state', () => {
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const appSettings = read(...desktopCrateSegments, 'src', 'app', 'settings.rs');

  assert.match(appSettings, /pub\(crate\) fn minimize_to_tray\(&self\) -> bool/u);
  assert.match(appSettings, /fn set_minimize_to_tray_enabled\(&self, enabled: bool\)/u);
  assert.doesNotMatch(appSettings, /pub\(crate\) minimize_to_tray:/u);
  assert.match(desktopLib, /state\.minimize_to_tray\(\)/u);
  assert.doesNotMatch(desktopLib, /minimize_to_tray\.lock\(\)/u);
});

test('usage and workspace date windows are supplied by sqlite adapters', () => {
  const coreLlmUsage = read('core', 'src', 'llm', 'usage.rs');
  const sqliteLlmUsage = read('adapters', 'sqlite', 'src', 'llm_usage.rs');
  const coreWorkspaceQuery = fs.readFileSync(
    path.join(repoRoot, 'core', 'src', 'history', 'workspace_query.rs'),
    'utf8',
  );
  const sqliteHistoryStore = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_store.rs'),
    'utf8',
  );

  assert.match(coreLlmUsage, /pub fn to_dashboard_stats_at/u);
  assert.doesNotMatch(coreLlmUsage, /Local::now|chrono::Local::now/u);
  assert.match(sqliteLlmUsage, /to_dashboard_stats_at/u);
  assert.match(sqliteLlmUsage, /Local::now\(\)\.date_naive\(\)/u);

  assert.match(coreWorkspaceQuery, /pub struct HistoryWorkspaceDateFilterThresholds/u);
  assert.match(coreWorkspaceQuery, /query_workspace_items_at/u);
  assert.match(coreWorkspaceQuery, /query_workspace_items_with_counts_at/u);
  assert.doesNotMatch(coreWorkspaceQuery, /Local::now|chrono::Local::now|with_ymd_and_hms|timestamp_millis_opt/u);
  assert.match(sqliteHistoryStore, /query_workspace_items_with_counts_at/u);
  assert.match(sqliteHistoryStore, /Local::now\(\)/u);
});

test('core ASR request contract is exposed through TS and UniFFI binding crates', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const tsBindLib = read('adapters', 'ts_bind', 'src', 'lib.rs');
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiAsrMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'asr_mapper.rs'),
    'utf8',
  );

  assert.match(coreCargo, /specta = \{[^}]*features = \["derive", "serde_json"\]/u);

  for (const typeName of [
    'AsrTranscriptionRequest',
    'AsrEngineConfig',
    'OnlineAsrProviderRequest',
    'VolcengineDoubaoAsrConfig',
    'TranscriptPostprocessOptions',
    'SpeakerProcessingConfig',
    'ModelFileConfig',
  ]) {
    assert.match(tsBindLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(uniffiLib, /default_batch_segmentation_mode/u);
  assert.match(uniffiLib, /online_asr_provider_request/u);
  assert.match(uniffiLib, /volcengine_doubao_asr_config_from_json/u);
  assert.match(uniffiAsrMapper, /pub enum FfiAsrEngine/u);
  assert.match(uniffiAsrMapper, /pub enum FfiAsrMode/u);
  assert.match(uniffiAsrMapper, /pub enum FfiBatchSegmentationMode/u);
  assert.match(uniffiAsrMapper, /pub struct FfiOnlineAsrProviderRequest/u);
  assert.match(uniffiAsrMapper, /pub struct FfiVolcengineDoubaoAsrConfig/u);
});

test('core online ASR provider manifest is exposed through UniFFI for mobile bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiAsrBridge = read('adapters', 'uniffi_bind', 'src', 'asr_bridge.rs');
  const uniffiAsrMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'asr_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiAsrBridge, /online_asr_providers as core_online_asr_providers/u);
  assert.match(uniffiAsrBridge, /find_online_asr_provider as core_find_online_asr_provider/u);
  assert.match(uniffiLib, /pub fn online_asr_providers\(\) -> Vec<FfiOnlineAsrProvider>/u);
  assert.match(
    uniffiLib,
    /pub fn find_online_asr_provider\(\s*provider_id: String,?\s*\) -> Option<FfiOnlineAsrProvider>/u,
  );
  assert.match(uniffiAsrBridge, /core_online_asr_providers\(\)\s*\.iter\(\)\s*\.map\(mapper::online_asr_provider_to_ffi\)/u);
  assert.match(uniffiAsrBridge, /core_find_online_asr_provider\(&provider_id\)\s*\.map\(mapper::online_asr_provider_to_ffi\)/u);

  for (const typeName of [
    'FfiOnlineAsrProvider',
    'FfiOnlineAsrCapability',
    'FfiOnlineAsrBatchCapability',
    'FfiOnlineAsrLocalFileBatchMode',
  ]) {
    assert.match(uniffiAsrMapper, new RegExp(`pub struct ${typeName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }
  assert.match(uniffiAsrMapper, /pub fn online_asr_provider_to_ffi/u);
});

test('UniFFI ASR bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiAsrBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'asr_bridge.rs');

  assert.match(uniffiLib, /^mod asr_bridge;/mu);
  assert.equal(fs.existsSync(uniffiAsrBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::default_batch_segmentation_mode\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::online_asr_providers\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::online_asr_provider_request\(provider_id, profile_id, config_json\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::volcengine_doubao_asr_config_from_json\(config_json\)/u);
  assert.doesNotMatch(uniffiLib, /asr_bridge::/u);
  assert.doesNotMatch(uniffiLib, /online_asr_providers as core_online_asr_providers/u);
  assert.doesNotMatch(uniffiLib, /find_online_asr_provider as core_find_online_asr_provider/u);
  assert.doesNotMatch(uniffiLib, /serde_json::from_str\(&config_json\)/u);
  assert.match(uniffiFacade, /asr_bridge::default_batch_segmentation_mode\(\)/u);
  assert.match(uniffiFacade, /asr_bridge::online_asr_providers\(\)/u);
  assert.match(uniffiFacade, /asr_bridge::online_asr_provider_request\(provider_id, profile_id, config_json\)/u);
  assert.match(uniffiFacade, /asr_bridge::volcengine_doubao_asr_config_from_json\(config_json\)/u);

  const uniffiAsrBridge = fs.readFileSync(uniffiAsrBridgePath, 'utf8');
  assert.match(uniffiAsrBridge, /online_asr_providers as core_online_asr_providers/u);
  assert.match(uniffiAsrBridge, /find_online_asr_provider as core_find_online_asr_provider/u);
  assert.match(uniffiAsrBridge, /parse_core_json\(&config_json, "ASR provider config"\)/u);
  assert.match(uniffiAsrBridge, /pub\(crate\) fn volcengine_doubao_asr_config_from_json/u);
});

test('UniFFI runtime bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiRuntimeBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'runtime_bridge.rs');

  assert.match(uniffiLib, /^mod runtime_bridge;/mu);
  assert.equal(fs.existsSync(uniffiRuntimeBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::normalize_export_format\(value\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::runtime_path_status\(path\)/u);
  assert.doesNotMatch(uniffiLib, /runtime_bridge::/u);
  assert.doesNotMatch(uniffiLib, /use sona_core::export::ExportFormat/u);
  assert.doesNotMatch(uniffiLib, /use sona_runtime_fs::resolve_runtime_path_status/u);
  assert.match(uniffiFacade, /runtime_bridge::normalize_export_format\(value\)/u);
  assert.match(uniffiFacade, /runtime_bridge::runtime_path_status\(path\)/u);

  const uniffiRuntimeBridge = fs.readFileSync(uniffiRuntimeBridgePath, 'utf8');
  assert.match(uniffiRuntimeBridge, /use sona_core::export::ExportFormat/u);
  assert.match(uniffiRuntimeBridge, /use sona_runtime_fs::resolve_runtime_path_status/u);
  assert.match(uniffiRuntimeBridge, /mapper::runtime_path_status_to_ffi\(resolve_runtime_path_status\(&path\)\)/u);
  assert.match(uniffiRuntimeBridge, /pub\(crate\) fn normalize_export_format/u);
});

test('UniFFI facade delegates bridge adapters outside the top-level export module', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacadePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'facade.rs');

  assert.match(uniffiLib, /^mod facade;/mu);
  assert.match(uniffiLib, /^pub use facade::SonaCoreFacade;/mu);
  assert.match(uniffiLib, /SonaCoreFacade::normalize_export_format\(value\)/u);
  assert.doesNotMatch(uniffiLib, /pub struct SonaCoreFacade/u);
  assert.doesNotMatch(uniffiLib, /impl SonaCoreFacade/u);
  assert.equal(fs.existsSync(uniffiFacadePath), true);

  const uniffiFacade = fs.readFileSync(uniffiFacadePath, 'utf8');
  assert.match(uniffiFacade, /pub struct SonaCoreFacade/u);
  assert.match(uniffiFacade, /impl SonaCoreFacade/u);
  assert.match(uniffiFacade, /runtime_bridge::normalize_export_format\(value\)/u);
  assert.match(uniffiFacade, /model_bridge::model_catalog_snapshot\(models_dir, installed_model_ids\)/u);
  assert.match(uniffiFacade, /llm_bridge::plan_summary_prompt_chunks_json/u);
  assert.match(uniffiFacade, /asr_bridge::online_asr_provider_request\(provider_id, profile_id, config_json\)/u);
  assert.match(uniffiFacade, /config_bridge::resolve_effective_config_json\(global_config_json, project_json\)/u);
  assert.doesNotMatch(uniffiFacade, /#\[uniffi::export\]/u);
});

test('UniFFI mapper facade re-exports focused domain mappers', () => {
  const mapperFacade = read('adapters', 'uniffi_bind', 'src', 'mapper.rs');

  for (const mapperName of [
    'runtime_mapper',
    'asr_mapper',
    'llm_mapper',
    'model_mapper',
    'config_mapper',
  ]) {
    assert.match(mapperFacade, new RegExp(`#\\[path = "mapper/${mapperName}\\.rs"\\]\\s*mod ${mapperName};`, 'u'));
    assert.match(mapperFacade, new RegExp(`pub use ${mapperName}::\\*;`, 'u'));
    assert.equal(
      exists('adapters', 'uniffi_bind', 'src', 'mapper', `${mapperName}.rs`),
      true,
    );
  }

  assert.doesNotMatch(mapperFacade, /use sona_core::/u);
  assert.doesNotMatch(mapperFacade, /pub (?:enum|struct) Ffi/u);
});

test('UniFFI root re-exports nested public model catalog records', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );
  const mapperExports = uniffiLib.match(/pub use mapper::\{([\s\S]*?)\};/u)?.[1] ?? '';

  assert.match(uniffiModelMapper, /pub struct FfiModelCatalogGroup/u);
  assert.match(uniffiModelMapper, /pub groups: Vec<FfiModelCatalogGroup>/u);
  assert.match(mapperExports, /\bFfiModelCatalogGroup\b/u);
});

test('core LLM provider manifest is exposed through UniFFI for mobile bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiLlmBridge, /llm_providers as core_llm_providers/u);
  assert.match(uniffiLlmBridge, /find_llm_provider_by_id_or_alias as core_find_llm_provider_by_id_or_alias/u);
  assert.match(uniffiLib, /pub fn llm_providers\(\) -> Vec<FfiLlmProvider>/u);
  assert.match(
    uniffiLib,
    /pub fn find_llm_provider_by_id_or_alias\(\s*id_or_alias: String,?\s*\) -> Option<FfiLlmProvider>/u,
  );
  assert.match(uniffiLlmBridge, /core_llm_providers\(\)\s*\.iter\(\)\s*\.map\(mapper::llm_provider_to_ffi\)/u);
  assert.match(
    uniffiLlmBridge,
    /core_find_llm_provider_by_id_or_alias\(&id_or_alias\)\s*\.map\(mapper::llm_provider_to_ffi\)/u,
  );

  assert.match(uniffiLlmMapper, /pub struct FfiLlmProviderDefaults/u);
  assert.match(uniffiLlmMapper, /pub struct FfiLlmProvider/u);
  assert.match(uniffiLlmMapper, /pub fn llm_provider_to_ffi/u);
});

test('core owns ASR runtime error contract reused by desktop', () => {
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const desktopAsrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const desktopAsrErrorPath = desktopCratePath('src', 'integrations', 'asr', 'error.rs');

  assert.match(coreAsr, /pub enum SherpaError/u);
  assert.match(coreAsr, /impl Serialize for SherpaError/u);
  assert.match(coreAsr, /UNSUPPORTED_ONLINE_PROVIDER/u);
  assert.match(coreAsr, /GENERIC_ERROR/u);
  assert.equal(fs.existsSync(desktopAsrErrorPath), false);
  assert.doesNotMatch(desktopAsrMod, /^mod error;/mu);
  assert.match(desktopAsrMod, /pub use sona_core::ports::asr::SherpaError;/u);
});

test('core owns ASR metric helpers reused by desktop', () => {
  const coreMetrics = read('core', 'src', 'transcription', 'asr_metrics.rs');
  const desktopMetrics = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'metrics.rs'),
    'utf8',
  );

  for (const helper of [
    'duration_to_ms',
    'samples_to_ms',
    'calculate_rtf',
    'calculate_rss_delta_mb',
    'format_optional_mb',
    'format_optional_ms',
    'format_optional_rtf',
    'format_optional_count',
  ]) {
    assert.match(coreMetrics, new RegExp(`pub fn ${helper}`, 'u'));
    assert.match(desktopMetrics, new RegExp(`sona_core::transcription::asr_metrics::[\\s\\S]*${helper}`, 'u'));
    assert.doesNotMatch(desktopMetrics, new RegExp(`fn ${helper}`, 'u'));
  }

  assert.doesNotMatch(coreMetrics, /SystemTime::now|UNIX_EPOCH|current_time_millis/u);
  assert.match(desktopMetrics, /pub\(crate\) fn current_time_millis/u);
  assert.match(desktopMetrics, /crate::platform::time::unix_timestamp_millis\(\)/u);
  assert.doesNotMatch(desktopMetrics, /SystemTime::now|UNIX_EPOCH/u);
  assert.match(desktopMetrics, /pub\(crate\) fn capture_process_memory_mb/u);
  assert.match(desktopMetrics, /sysinfo::/u);
});

test('history item factories receive generated values from adapters', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const coreHistoryFactory = fs.readFileSync(
    path.join(repoRoot, 'core', 'src', 'history', 'item_factory.rs'),
    'utf8',
  );
  const sqliteHistoryStore = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_store.rs'),
    'utf8',
  );

  assert.match(coreHistoryFactory, /pub struct HistoryItemGeneratedValues/u);
  assert.match(coreHistoryFactory, /fallback_id/u);
  assert.match(coreHistoryFactory, /timestamp/u);
  assert.match(coreHistoryFactory, /recording_title/u);
  assert.doesNotMatch(coreHistoryFactory, /Uuid::new_v4|uuid::Uuid::new_v4/u);
  assert.doesNotMatch(coreHistoryFactory, /current_time_millis/u);
  assert.doesNotMatch(coreHistoryFactory, /chrono::Local|DateTime::<chrono::Local>|UNIX_EPOCH/u);
  assert.doesNotMatch(coreHistoryFactory, /crate::(?:runtime::)?file_utils/u);
  assert.doesNotMatch(
    sqliteHistoryStore,
    /sona_core::(?:runtime::)?file_utils::current_time_millis/u,
  );
  assert.match(sqliteHistoryStore, /fn new_history_item_generated_values/u);
  assert.match(sqliteHistoryStore, /HistoryItemGeneratedValues/u);
  assert.match(sqliteHistoryStore, /recording_title/u);
  assert.doesNotMatch(coreCargo, /chrono = \{[^}]*"clock"/u);
});

test('local ASR runtime pool is owned by the local ASR adapter', () => {
  const localAsrLib = read('adapters', 'local_asr', 'src', 'lib.rs');
  const localAsrRuntime = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'runtime.rs'),
    'utf8',
  );
  const desktopAsrState = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'state.rs'),
    'utf8',
  );
  const desktopAsrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(localAsrLib, /^pub mod runtime;/mu);
  assert.match(localAsrRuntime, /pub struct RecognizerPool/u);
  assert.match(localAsrRuntime, /pub struct ModelConfigKey/u);
  assert.doesNotMatch(localAsrRuntime, /pub recognizers:/u);
  assert.doesNotMatch(localAsrRuntime, /pub punctuations:/u);
  assert.match(localAsrRuntime, /pub async fn recognizer_cell_for_gpu_plan/u);
  assert.match(localAsrRuntime, /pub async fn register_recognizer_gpu_provider/u);
  assert.match(localAsrRuntime, /pub async fn punctuation_cell_for_path/u);
  assert.match(desktopAsrState, /use sona_local_asr::runtime::RecognizerPool;/u);
  assert.doesNotMatch(desktopAsrState, /pub struct RecognizerPool/u);
  assert.doesNotMatch(desktopAsrState, /pub struct ModelConfigKey/u);
  assert.match(desktopAsrMod, /pub use sona_local_asr::runtime::RecognizerPool;/u);
  assert.match(desktopAsrMod, /pub\(crate\) use sona_local_asr::runtime::ModelConfigKey;/u);

  for (const desktopFile of [
    ...rustFilesUnder(desktopCratePath('src', 'integrations', 'asr')),
    desktopCratePath('src', 'integrations', 'streaming.rs'),
  ]) {
    const content = fs.readFileSync(desktopFile, 'utf8');
    assert.doesNotMatch(content, /\.recognizers\.lock\(\)/u);
    assert.doesNotMatch(content, /\.recognizers\.insert/u);
    assert.doesNotMatch(content, /\.recognizers\.get/u);
    assert.doesNotMatch(content, /\.punctuations\.lock\(\)/u);
  }
});

test('desktop API server obtains ASR recognizer pools through integration facade', () => {
  const appServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const desktopAsrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(desktopAsrMod, /pub\(crate\) fn recognizer_pool_for_app/u);
  assert.match(appServer, /crate::integrations::asr::recognizer_pool_for_app\(app\.as_ref\(\)\)/u);
  assert.doesNotMatch(appServer, /state::<crate::integrations::asr::AsrState>\(\)\s*\.recognizer_pool/u);
  assert.doesNotMatch(appServer, /RecognizerPool::new/u);
});

test('desktop API server controller owns runtime handles', () => {
  const appServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const appSetup = read(...desktopCrateSegments, 'src', 'app', 'setup.rs');

  assert.match(appServer, /pub\(crate\) fn online_asr_config/u);
  assert.match(appServer, /pub\(crate\) async fn replace_online_asr_config/u);
  assert.match(appServer, /pub\(crate\) async fn take_running_server/u);
  assert.match(appServer, /pub\(crate\) async fn set_running_server/u);
  assert.doesNotMatch(appServer, /pub running_server:/u);
  assert.doesNotMatch(appServer, /pub online_asr_config:/u);
  assert.doesNotMatch(appSetup, /online_asr_config\.clone\(\)/u);
});

test('desktop ASR session maps are owned by AsrState', () => {
  const stateRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'state.rs');
  const commandsAsr = read(...desktopCrateSegments, 'src', 'commands', 'asr.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');

  assert.match(stateRs, /pub async fn insert_session/u);
  assert.match(stateRs, /pub async fn session/u);
  assert.doesNotMatch(stateRs, /pub active_sessions:/u);
  assert.doesNotMatch(stateRs, /pub instance_engines:/u);
  assert.doesNotMatch(commandsAsr, /active_sessions\.lock\(\)/u);
  assert.doesNotMatch(asrMod, /active_sessions\.lock\(\)/u);
});

test('desktop streaming handler uses Tauri streaming context accessors', () => {
  const appServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const streaming = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');

  assert.match(appServer, /pub\(crate\) struct TauriStreamingContext/u);
  assert.doesNotMatch(appServer, /pub struct TauriStreamingContext/u);
  assert.match(appServer, /pub\(crate\) fn app_handle\(&self\)/u);
  assert.match(appServer, /pub\(crate\) fn recognizer_pool\(&self\)/u);
  assert.doesNotMatch(appServer, /pub app:/u);
  assert.doesNotMatch(appServer, /pub recognizer_pool:/u);
  assert.match(streaming, /context\.app_handle\(\)/u);
  assert.match(streaming, /recognizer_cell_for_gpu_plan/u);
  assert.match(streaming, /register_recognizer_gpu_provider/u);
  assert.doesNotMatch(streaming, /context\.app\b/u);
  assert.doesNotMatch(streaming, /context\.recognizer_pool(?!\()/u);
});

test('desktop standalone streaming uses local ASR recognizer accessors', () => {
  const streaming = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');

  assert.doesNotMatch(streaming, /RecognizerInner/u);
  assert.doesNotMatch(streaming, /\b(?:recognizer|r)\.inner\b/u);
  assert.doesNotMatch(asrMod, /\bRecognizerInner\b/u);
});

test('desktop batch ASR uses local ASR recognizer accessors', () => {
  const batch = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');

  assert.doesNotMatch(batch, /RecognizerInner/u);
  assert.doesNotMatch(batch, /\brecognizer\.inner\b/u);
});

test('local streaming ASR uses local ASR recognizer accessors', () => {
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );

  assert.doesNotMatch(adapterSession, /RecognizerInner/u);
  assert.doesNotMatch(adapterSession, /\b(?:recognizer|recognizer_copy)\.inner\b/u);
});

test('local ASR recognizer internals stay behind accessors', () => {
  const localAsrRecognizer = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'),
    'utf8',
  );

  for (const helper of ['kind_label', 'is_offline', 'offline', 'online']) {
    assert.match(localAsrRecognizer, new RegExp(`pub fn ${helper}\\(&self\\)`, 'u'));
  }
  assert.doesNotMatch(localAsrRecognizer, /pub enum RecognizerInner/u);
  assert.doesNotMatch(localAsrRecognizer, /pub inner: RecognizerInner/u);
});

test('streaming ASR session contract is core-owned and platform-neutral', () => {
  const coreAsr = fs.readFileSync(
    path.join(repoRoot, 'core', 'src', 'ports', 'asr.rs'),
    'utf8',
  );
  const tauriTraits = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'traits.rs'),
    'utf8',
  );
  const tauriState = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'state.rs'),
    'utf8',
  );
  const tauriCommands = fs.readFileSync(
    desktopCratePath('src', 'commands', 'asr.rs'),
    'utf8',
  );

  assert.match(coreAsr, /pub trait AsrStreamingSession/u);
  assert.match(coreAsr, /pub trait AsrRuntimeObserver/u);
  assert.doesNotMatch(
    coreAsr,
    /AppHandle|AsrState|TauriEventEmitter|crate::platform/u,
  );
  assert.doesNotMatch(tauriTraits, /trait AsrStreamingSession/u);
  assert.match(
    tauriState,
    /sona_core::ports::asr::AsrStreamingSession/u,
  );
  assert.match(tauriCommands, /session\.start\(\)\.await/u);
  assert.match(tauriCommands, /session\.flush\(\)\.await/u);
});

test('local streaming ASR session is implemented by the local adapter', () => {
  const localAsrRoot = path.join(repoRoot, 'adapters', 'local_asr');
  const adapterSessionPath = path.join(
    localAsrRoot, 'src', 'streaming', 'session.rs',
  );
  const adapterStreamingMod = fs.readFileSync(
    path.join(localAsrRoot, 'src', 'streaming', 'mod.rs'),
    'utf8',
  );
  const localAsrSources = rustFilesUnder(path.join(localAsrRoot, 'src'))
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');
  const localAsrCargo = fs.readFileSync(path.join(localAsrRoot, 'Cargo.toml'), 'utf8');
  const desktopSessionPath = desktopCratePath('src', 'integrations', 'asr', 'sherpa_onnx.rs');
  const desktopSchedulerPath = desktopCratePath('src', 'platform', 'asr_runtime.rs');
  const provider = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'adapter.rs'),
    'utf8',
  );

  assert.equal(fs.existsSync(adapterSessionPath), true);
  assert.equal(fs.existsSync(desktopSessionPath), false);
  assert.equal(fs.existsSync(desktopSchedulerPath), false);
  const adapterSession = fs.readFileSync(adapterSessionPath, 'utf8');
  assert.match(adapterSession, /impl AsrStreamingSession for LocalSherpaSession/u);
  assert.doesNotMatch(adapterSession, /tauri::|crate::platform|AsrState/u);
  assert.match(adapterStreamingMod, /^mod inference;$/mu);
  assert.doesNotMatch(
    `${localAsrSources}\n${localAsrCargo}`,
    /tauri|AsrState|crate::platform/iu,
  );
  assert.match(provider, /sona_local_asr::streaming::create_streaming_session/u);
});

test('local ASR streaming runtime state is owned by the local ASR adapter', () => {
  const localAsrRuntime = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'runtime.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const desktopAsr = rustFilesUnder(
    desktopCratePath('src', 'integrations', 'asr'),
  )
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');
  const desktopStreaming = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'streaming.rs'),
    'utf8',
  );

  for (const symbol of ['SherpaInstance', 'OfflineState', 'RecordDiagnosticsState']) {
    assert.match(localAsrRuntime, new RegExp(`pub struct ${symbol}`, 'u'));
    assert.doesNotMatch(desktopAsr, new RegExp(`pub struct ${symbol}`, 'u'));
  }

  for (const field of ['vad', 'vad_model', 'vad_buffer']) {
    assert.doesNotMatch(localAsrRuntime, new RegExp(`pub ${field}:`, 'u'));
  }

  for (const helper of [
    'configure_vad',
    'reset_or_reload_vad',
    'has_vad_configuration',
    'vad',
    'vad_buffer',
  ]) {
    assert.match(localAsrRuntime, new RegExp(`pub fn ${helper}\\(`, 'u'));
  }

  for (const field of [
    'speech_buffer',
    'ring_buffer',
    'is_speaking',
    'last_inference_time',
    'utterance_start_sample',
  ]) {
    assert.doesNotMatch(localAsrRuntime, new RegExp(`pub ${field}:`, 'u'));
  }

  for (const field of [
    'first_sample_logged',
    'skipped_while_stopped_logged',
    'first_segment_emitted',
    'is_running',
    'recognizer',
    'punctuation',
    'stream',
    'last_partial_metric_sample',
  ]) {
    assert.doesNotMatch(localAsrRuntime, new RegExp(`pub ${field}:`, 'u'));
    assert.doesNotMatch(
      adapterSession,
      field === 'is_running'
        ? /\binstance\.is_running(?!\()/u
        : field === 'recognizer'
          ? /\binstance\.recognizer\b(?!\s*\()/u
        : field === 'punctuation'
          ? /\binstance\.punctuation\b(?!\s*\()/u
        : field === 'stream'
          ? /\binstance\.stream(?!\s*\()/u
        : field === 'last_partial_metric_sample'
          ? /\binstance\.last_partial_metric_sample\b/u
          : new RegExp(`record_diagnostics[\\s\\S]{0,80}\\.${field}\\b`, 'u'),
    );
  }

  assert.match(adapterSession, /pub struct LocalSherpaSession/u);
  assert.match(adapterSession, /pub async fn create_streaming_session/u);
  assert.doesNotMatch(adapterSession, /pub instance:/u);

  for (const helper of [
    'buffered_sample_count',
    'begin_speech',
    'buffered_speech_chunk_count',
    'buffered_speech_sample_count',
    'clear_speech_buffer',
    'push_speech_chunk',
    'finish_speech_with_chunk',
    'push_ring_chunk',
    'push_ring_chunk_with_sample_limit',
    'ring_sample_count',
    'speech_chunks',
    'should_run_inference',
    'mark_inference_time',
    'utterance_start_seconds',
    'start_instance_runtime',
    'stop_instance_runtime',
    'should_log_first_sample',
    'mark_first_sample_logged',
    'should_log_skipped_while_stopped',
    'mark_skipped_while_stopped_logged',
    'first_segment_emitted_flag',
    'is_running',
    'recognizer',
    'recognizer_clone',
    'set_recognizer',
    'punctuation',
    'punctuation_clone',
    'has_punctuation',
    'set_punctuation',
    'stream',
    'take_stream',
    'restore_stream',
    'should_record_partial_metric',
    'mark_partial_metric_sample',
    'clear_partial_metric_sample',
  ]) {
    assert.match(localAsrRuntime, new RegExp(`pub fn ${helper}`, 'u'));
    assert.doesNotMatch(adapterSession, new RegExp(`pub fn ${helper}`, 'u'));
  }

  assert.match(adapterSession, /use crate::runtime::\{[\s\S]*SherpaInstance/u);
  assert.match(desktopStreaming, /sona_local_asr::runtime::OfflineState/u);
  assert.doesNotMatch(
    adapterSession,
    /offline_state\.(?:is_speaking|ring_buffer|speech_buffer|utterance_start_sample|last_inference_time)\b/u,
  );
  assert.doesNotMatch(
    desktopStreaming,
    /offline_state\.(?:is_speaking|ring_buffer|speech_buffer|utterance_start_sample)\b/u,
  );
});

test('core owns local batch ASR request contract reused by desktop', () => {
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const desktopAsrTypes = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'types.rs'),
    'utf8',
  );
  const desktopAsrAdapter = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'adapter.rs'),
    'utf8',
  );
  const desktopBatchProcessor = desktopAsrAdapter.slice(
    desktopAsrAdapter.indexOf('pub struct LocalSherpaBatchProcessor'),
  );

  assert.match(coreAsr, /pub struct BatchTranscriptionRequest/u);
  assert.match(coreAsr, /pub struct LocalSherpaStreamingRequest/u);
  assert.match(coreAsr, /pub instance_id: Option<String>/u);
  assert.match(coreAsr, /pub instance_id: String/u);
  assert.match(coreAsr, /pub postprocessor: TranscriptPostprocessor/u);
  assert.match(coreAsr, /pub fn validate_local_sherpa_mode/u);
  assert.match(coreAsr, /pub fn from_local_sherpa_request/u);
  assert.match(desktopAsrTypes, /BatchTranscriptionRequest/u);
  assert.match(desktopAsrTypes, /LocalSherpaStreamingRequest/u);
  assert.doesNotMatch(desktopAsrTypes, /pub struct BatchTranscriptionRequest/u);
  assert.doesNotMatch(desktopAsrTypes, /pub struct LocalSherpaStreamingRequest/u);
  assert.match(desktopAsrAdapter, /validate_local_sherpa_mode\(request, AsrMode::Batch\)/u);
  assert.match(desktopAsrAdapter, /BatchTranscriptionRequest::from_local_sherpa_request/u);
  assert.match(desktopAsrAdapter, /LocalSherpaStreamingRequest::from_local_sherpa_request/u);
  assert.doesNotMatch(desktopBatchProcessor, /AsrEngineConfig::LocalSherpa/u);
  assert.doesNotMatch(desktopAsrAdapter, /AsrEngineConfig::LocalSherpa/u);
  assert.doesNotMatch(desktopAsrAdapter, /fn ensure_mode/u);
});

test('core LLM request contracts are exposed through UniFFI binding records', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'llm_config_from_json',
    'polish_segments_request_from_json',
    'translate_segments_request_from_json',
    'summarize_transcript_request_from_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  for (const typeName of [
    'FfiLlmProviderStrategy',
    'FfiLlmConfig',
    'FfiLlmSegmentInput',
    'FfiSummarySegmentInput',
    'FfiSummaryTemplateConfig',
    'FfiPolishSegmentsRequest',
    'FfiTranslateSegmentsRequest',
    'FfiSummarizeTranscriptRequest',
  ]) {
    assert.match(uniffiLlmMapper, new RegExp(`pub (?:enum|struct) ${typeName}`, 'u'));
  }

  assert.match(uniffiLlmMapper, /pub fn llm_config_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn polish_segments_request_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn translate_segments_request_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn summarize_transcript_request_to_ffi/u);
});

test('UniFFI LLM JSON bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiLlmBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');

  assert.match(uniffiLib, /^mod llm_bridge;/mu);
  assert.equal(fs.existsSync(uniffiLlmBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::validate_llm_config_json\(config_json\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::llm_segment_inputs_from_transcript_json\(segments_json\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::plan_polish_prompt_chunks_json\(/u);
  assert.doesNotMatch(uniffiLib, /llm_bridge::/u);
  assert.doesNotMatch(uniffiLib, /validate_llm_config as core_validate_llm_config/u);
  assert.doesNotMatch(uniffiLib, /segment_inputs_from_transcript as core_segment_inputs_from_transcript/u);
  assert.doesNotMatch(uniffiLib, /plan_segment_task_chunks as core_plan_segment_task_chunks/u);
  assert.match(uniffiFacade, /llm_bridge::validate_llm_config_json\(config_json\)/u);
  assert.match(uniffiFacade, /llm_bridge::llm_segment_inputs_from_transcript_json\(segments_json\)/u);
  assert.match(uniffiFacade, /llm_bridge::plan_polish_prompt_chunks_json\(/u);

  const uniffiLlmBridge = fs.readFileSync(uniffiLlmBridgePath, 'utf8');
  assert.match(uniffiLlmBridge, /use crate::json_bridge::\{[\s\S]*map_core_validation_result[\s\S]*parse_core_json[\s\S]*serialize_core_json[\s\S]*\};/u);
  assert.match(uniffiLlmBridge, /validate_llm_config as core_validate_llm_config/u);
  assert.match(uniffiLlmBridge, /segment_inputs_from_transcript as core_segment_inputs_from_transcript/u);
  assert.match(uniffiLlmBridge, /plan_segment_task_chunks as core_plan_segment_task_chunks/u);
  assert.match(uniffiLlmBridge, /pub\(crate\) fn plan_polish_prompt_chunks_json/u);
});

test('core LLM request validation is exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiJsonBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'json_bridge.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');

  for (const validationName of [
    'validate_llm_config_json',
    'validate_llm_generate_request_json',
    'validate_polish_segments_request_json',
    'validate_translate_segments_request_json',
    'validate_summarize_transcript_request_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${validationName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${validationName}`, 'u'));
  }

  for (const coreValidationName of [
    'validate_llm_config',
    'validate_llm_generate_request',
    'validate_polish_segments_request',
    'validate_translate_segments_request',
    'validate_summarize_transcript_request',
  ]) {
    assert.match(uniffiLlmBridge, new RegExp(`${coreValidationName} as core_${coreValidationName}`, 'u'));
    assert.match(uniffiLlmBridge, new RegExp(`core_${coreValidationName}\\(&`, 'u'));
  }

  assert.match(uniffiLib, /^mod json_bridge;/mu);
  assert.doesNotMatch(uniffiLib, /use json_bridge::/u);
  assert.match(uniffiLlmBridge, /use crate::json_bridge::\{[\s\S]*map_core_validation_result[\s\S]*parse_core_json[\s\S]*serialize_core_json[\s\S]*\};/u);
  assert.doesNotMatch(uniffiLib, /fn map_core_validation_result\(result: Result<\(\), String>\) -> SonaCoreBindingResult<\(\)>/u);
  assert.equal(fs.existsSync(uniffiJsonBridgePath), true);
  const uniffiJsonBridge = fs.readFileSync(uniffiJsonBridgePath, 'utf8');
  assert.match(uniffiJsonBridge, /pub\(crate\) fn map_core_validation_result\([\s\S]*result: Result<\(\), String>[\s\S]*\) -> SonaCoreBindingResult<\(\)>/u);
  assert.match(uniffiJsonBridge, /pub\(crate\) fn parse_core_json</u);
  assert.match(uniffiJsonBridge, /pub\(crate\) fn parse_optional_core_json</u);
  assert.match(uniffiJsonBridge, /pub\(crate\) fn serialize_core_json</u);
});

test('core transcript LLM job helpers are exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');

  for (const exportName of [
    'llm_segment_inputs_from_transcript_json',
    'summary_segment_inputs_from_transcript_json',
    'merge_translated_items_into_transcript_json',
    'merge_polished_items_into_transcript_json',
    'summary_source_fingerprint_from_transcript_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  for (const coreHelperName of [
    'segment_inputs_from_transcript',
    'summary_inputs_from_transcript',
    'merge_translated_items_into_segments',
    'merge_polished_items_into_segments',
    'compute_summary_source_fingerprint',
  ]) {
    assert.match(uniffiLlmBridge, new RegExp(`${coreHelperName} as core_${coreHelperName}`, 'u'));
    assert.match(uniffiLlmBridge, new RegExp(`core_${coreHelperName}\\(`, 'u'));
  }

  assert.match(uniffiLlmBridge, /parse_core_json\(&segments_json, "transcript segments"\)/u);
  assert.match(uniffiLlmBridge, /serialize_core_json\(&merged, "merged transcript segments"\)/u);
});

test('core LLM prompt and chunk parsing helpers are exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'build_polish_prompt_json',
    'build_translate_prompt_json',
    'build_summary_chunk_prompt_json',
    'build_summary_finalize_prompt_json',
    'parse_polish_chunk_json',
    'parse_translate_chunk_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  for (const coreHelperName of [
    'build_polish_prompt',
    'build_translate_prompt',
    'build_summary_chunk_prompt',
    'build_summary_finalize_prompt',
    'parse_polish_chunk',
    'parse_translate_chunk',
  ]) {
    assert.match(uniffiLlmBridge, new RegExp(`${coreHelperName} as core_${coreHelperName}`, 'u'));
    assert.match(uniffiLlmBridge, new RegExp(`core_${coreHelperName}\\(`, 'u'));
  }

  for (const typeName of [
    'FfiPolishedSegment',
    'FfiTranslatedSegment',
  ]) {
    assert.match(uniffiLlmMapper, new RegExp(`pub struct ${typeName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(uniffiLlmMapper, /pub fn polished_segment_to_ffi/u);
  assert.match(uniffiLlmMapper, /pub fn translated_segment_to_ffi/u);
  assert.match(uniffiLlmBridge, /parse_core_json\(&segments_json, "LLM segment inputs"\)/u);
  assert.match(uniffiLlmBridge, /parse_core_json\(&template_json, "summary template"\)/u);
});

test('core LLM prompt chunk planning is exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiLlmBridge = read('adapters', 'uniffi_bind', 'src', 'llm_bridge.rs');
  const uniffiLlmMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'llm_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'plan_polish_prompt_chunks_json',
    'plan_translate_prompt_chunks_json',
    'plan_summary_prompt_chunks_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  assert.match(uniffiLlmMapper, /pub struct FfiLlmPromptChunk/u);
  assert.match(uniffiLlmMapper, /pub fn llm_prompt_chunk_to_ffi/u);
  assert.match(uniffiLlmBridge, /plan_segment_task_chunks as core_plan_segment_task_chunks/u);
  assert.match(uniffiLlmBridge, /split_summary_segments as core_split_summary_segments/u);
  assert.match(uniffiLlmBridge, /core_plan_segment_task_chunks\(/u);
  assert.match(uniffiLlmBridge, /core_split_summary_segments\(/u);
  assert.match(uniffiLlmBridge, /DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET/u);
  assert.match(uniffiLlmBridge, /DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET/u);
  assert.match(uniffiLlmBridge, /MIN_SUMMARY_CHUNK_CHAR_BUDGET/u);
});

test('core config migration surface is exposed through UniFFI JSON bridge', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiConfigBridge = read('adapters', 'uniffi_bind', 'src', 'config_bridge.rs');
  const uniffiConfigMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'config_mapper.rs'),
    'utf8',
  );

  for (const exportName of [
    'default_config_json',
    'migrate_app_config_json',
    'resolve_effective_config_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}`, 'u'));
  }

  assert.match(uniffiConfigBridge, /core_migrate_app_config/u);
  assert.match(uniffiConfigBridge, /core_resolve_effective_config/u);
  assert.match(uniffiConfigMapper, /pub struct FfiConfigMigrationResult/u);
  assert.match(uniffiConfigMapper, /pub config_json: String/u);
  assert.match(uniffiConfigMapper, /pub migrated: bool/u);
});

test('UniFFI config bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiConfigBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'config_bridge.rs');

  assert.match(uniffiLib, /^mod config_bridge;/mu);
  assert.equal(fs.existsSync(uniffiConfigBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::default_config_json\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::migrate_app_config_json\(/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_effective_config_json\(global_config_json, project_json\)/u);
  assert.doesNotMatch(uniffiLib, /config_bridge::/u);
  assert.doesNotMatch(uniffiLib, /migrate_app_config as core_migrate_app_config/u);
  assert.doesNotMatch(uniffiLib, /resolve_effective_config as core_resolve_effective_config/u);
  assert.doesNotMatch(uniffiLib, /parse_optional_core_json\(saved_config_json\.as_deref\(\), "saved config"\)/u);
  assert.match(uniffiFacade, /config_bridge::default_config_json\(\)/u);
  assert.match(uniffiFacade, /config_bridge::migrate_app_config_json\(/u);
  assert.match(uniffiFacade, /config_bridge::resolve_effective_config_json\(global_config_json, project_json\)/u);

  const uniffiConfigBridge = fs.readFileSync(uniffiConfigBridgePath, 'utf8');
  assert.match(uniffiConfigBridge, /default_config/u);
  assert.match(uniffiConfigBridge, /migrate_app_config as core_migrate_app_config/u);
  assert.match(uniffiConfigBridge, /resolve_effective_config as core_resolve_effective_config/u);
  assert.match(uniffiConfigBridge, /parse_optional_core_json\(saved_config_json\.as_deref\(\), "saved config"\)/u);
  assert.match(uniffiConfigBridge, /pub\(crate\) fn resolve_effective_config_json/u);
});

test('UniFFI model bridge is isolated from the top-level binding facade', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const uniffiModelBridgePath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'model_bridge.rs');

  assert.match(uniffiLib, /^mod model_bridge;/mu);
  assert.equal(fs.existsSync(uniffiModelBridgePath), true);
  assert.match(uniffiLib, /SonaCoreFacade::preset_models\(\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::model_catalog_selected_ids\(/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_model_download\(model_id, models_dir\)/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_gpu_acceleration\(value\)/u);
  assert.doesNotMatch(uniffiLib, /model_bridge::/u);
  assert.doesNotMatch(uniffiLib, /preset_models as core_preset_models/u);
  assert.doesNotMatch(uniffiLib, /resolve_model_download as core_resolve_model_download/u);
  assert.doesNotMatch(uniffiLib, /resolve_gpu_acceleration as core_resolve_gpu_acceleration/u);
  assert.match(uniffiFacade, /model_bridge::preset_models\(\)/u);
  assert.match(uniffiFacade, /model_bridge::model_catalog_selected_ids\(/u);
  assert.match(uniffiFacade, /model_bridge::resolve_model_download\(model_id, models_dir\)/u);
  assert.match(uniffiFacade, /model_bridge::resolve_gpu_acceleration\(value\)/u);

  const uniffiModelBridge = fs.readFileSync(uniffiModelBridgePath, 'utf8');
  assert.match(uniffiModelBridge, /preset_models as core_preset_models/u);
  assert.match(uniffiModelBridge, /resolve_model_catalog_selected_ids/u);
  assert.match(uniffiModelBridge, /resolve_model_download as core_resolve_model_download/u);
  assert.match(uniffiModelBridge, /resolve_gpu_acceleration as core_resolve_gpu_acceleration/u);
  assert.match(uniffiModelBridge, /pub\(crate\) fn model_catalog_snapshot/u);
});

test('core model catalog selected id resolution is exposed through UniFFI bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelBridge = read('adapters', 'uniffi_bind', 'src', 'model_bridge.rs');
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiModelBridge, /resolve_model_catalog_selected_ids/u);
  assert.match(uniffiLib, /pub fn model_catalog_selected_ids/u);
  assert.match(uniffiLib, /SonaCoreFacade::model_catalog_selected_ids/u);
  assert.match(uniffiModelMapper, /pub struct FfiModelSelectionPaths/u);
  assert.match(uniffiModelMapper, /pub struct FfiModelCatalogSelectedIds/u);
  assert.match(uniffiModelMapper, /pub fn model_selection_paths_from_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_catalog_selected_ids_to_ffi/u);
});

test('core model catalog grouping and dependency requests are exposed through UniFFI snapshot', () => {
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  for (const typeName of [
    'FfiModelCatalogSectionType',
    'FfiModelCatalogGroup',
    'FfiModelCatalogSection',
    'FfiModelDependencyConfigKey',
    'FfiModelDependencyRequest',
    'FfiModelDependencyRequestsForModel',
  ]) {
    assert.match(uniffiModelMapper, new RegExp(`pub (?:enum|struct) ${typeName}`, 'u'));
  }

  assert.match(uniffiModelMapper, /pub sections: Vec<FfiModelCatalogSection>/u);
  assert.match(uniffiModelMapper, /pub dependency_requests_by_model_id: Vec<FfiModelDependencyRequestsForModel>/u);
  assert.match(uniffiModelMapper, /pub fn model_catalog_section_to_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_dependency_requests_by_model_id_to_ffi/u);
});

test('core model catalog path indexes are exposed through UniFFI snapshot', () => {
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  for (const typeName of [
    'FfiModelPathByIdEntry',
    'FfiModelIdByNormalizedPathEntry',
    'FfiModelCatalogPathMatchToken',
  ]) {
    assert.match(uniffiModelMapper, new RegExp(`pub struct ${typeName}`, 'u'));
  }

  assert.match(uniffiModelMapper, /pub model_path_by_id: Vec<FfiModelPathByIdEntry>/u);
  assert.match(uniffiModelMapper, /pub model_id_by_normalized_path: Vec<FfiModelIdByNormalizedPathEntry>/u);
  assert.match(uniffiModelMapper, /pub path_match_tokens: Vec<FfiModelCatalogPathMatchToken>/u);
  assert.match(uniffiModelMapper, /pub fn model_path_by_id_to_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_id_by_normalized_path_to_ffi/u);
  assert.match(uniffiModelMapper, /pub fn model_catalog_path_match_token_to_ffi/u);
});

test('core model download planning is exposed through UniFFI bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelBridge = read('adapters', 'uniffi_bind', 'src', 'model_bridge.rs');
  const uniffiModelMapper = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'mapper', 'model_mapper.rs'),
    'utf8',
  );

  assert.match(uniffiModelBridge, /core_resolve_model_download/u);
  assert.match(uniffiModelBridge, /required_companion_models/u);
  assert.match(uniffiLib, /pub fn resolve_model_download/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_model_download/u);
  assert.match(uniffiModelMapper, /pub struct FfiRequiredCompanionModels/u);
  assert.match(uniffiModelMapper, /pub struct FfiResolvedModelDownload/u);
  assert.match(uniffiModelMapper, /pub fn resolved_model_download_to_ffi/u);
});

test('core GPU acceleration config validation is exposed through UniFFI bindings', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiModelBridge = read('adapters', 'uniffi_bind', 'src', 'model_bridge.rs');

  assert.match(uniffiModelBridge, /resolve_gpu_acceleration as core_resolve_gpu_acceleration/u);
  assert.match(uniffiLib, /pub fn resolve_gpu_acceleration/u);
  assert.match(uniffiLib, /SonaCoreFacade::resolve_gpu_acceleration/u);
});

test('UniFFI Kotlin bindings are generated through the 0.32 Android Gradle integration', () => {
  const workspaceCargo = read('Cargo.toml');
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  const bindgenCargoPath = path.join(repoRoot, 'tools', 'uniffi_bindgen', 'Cargo.toml');
  const bindgenCargo = fs.readFileSync(
    bindgenCargoPath,
    'utf8',
  );
  const bindgenMain = read('tools', 'uniffi_bindgen', 'src', 'main.rs');
  const generateScript = read('scripts', 'generate-uniffi-kotlin.js');
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  assert.match(workspaceCargo, /"tools\/uniffi_bindgen"/u);
  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');
  assert.match(bindgenCargo, /name\s*=\s*"sona-uniffi-bindgen"/u);
  assertCargoDependencyVersionAndFeature(bindgenCargoPath, 'uniffi', '0.32', 'cli');
  assert.match(bindgenMain, /uniffi::uniffi_bindgen_main\(\)/u);
  assert.match(generateScript, /cargo/u);
  assert.match(generateScript, /sona-uniffi-bind/u);
  assert.match(generateScript, /sona-uniffi-bindgen/u);
  assert.match(generateScript, /generate/u);
  assert.match(generateScript, /--library/u);
  assert.match(generateScript, /--language/u);
  assert.match(generateScript, /kotlin/u);
  assert.match(generateScript, /fs\.rmSync\(outDir,\s*\{\s*recursive:\s*true,\s*force:\s*true\s*\}\)/u);
  assert.match(gradleIntegration, /generateSonaUniffiKotlin/u);
  assert.match(gradleIntegration, /scripts\/generate-uniffi-kotlin\.js/u);
  assert.match(gradleIntegration, /generated\/source\/uniffi\/main\/kotlin/u);
  assert.match(gradleIntegration, /com\.android\.build\.api\.dsl\.LibraryExtension/u);
  assert.match(gradleIntegration, /java\.directories\.add\(generatedKotlinDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /net\.java\.dev\.jna:jna:5\.12\.0@aar/u);
  assert.match(gradleIntegration, /org\.jetbrains\.kotlinx:kotlinx-coroutines-core:1\.6\.4/u);
});

test('UniFFI streaming ASR preserves the online-only architecture boundary', () => {
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  const streamingBridge = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'src', 'asr_streaming_bridge.rs'),
    'utf8',
  );

  assertStreamingAsrArchitecture(uniffiCargoPath, streamingBridge);
});

test('Android UniFFI streaming smoke compiles the generated observer and session surface', () => {
  const sampleKotlin = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin', 'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt'),
    'utf8',
  );
  const consumerKotlin = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin', 'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt'),
    'utf8',
  );

  for (const kotlinSmoke of [sampleKotlin, consumerKotlin]) {
    assertAndroidStreamingSmoke(kotlinSmoke);
  }
});

test('UniFFI Android Gradle integration builds ABI-scoped native libraries', () => {
  const buildScript = read('scripts', 'build-uniffi-android-libs.js');
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );
  const androidReadme = read('platforms', 'android', 'README.md');

  for (const [abi, target] of [
    ['arm64-v8a', 'aarch64-linux-android'],
    ['armeabi-v7a', 'armv7-linux-androideabi'],
    ['x86', 'i686-linux-android'],
    ['x86_64', 'x86_64-linux-android'],
  ]) {
    assert.match(buildScript, new RegExp(`['"]${abi}['"]:\\s*['"]${target}['"]`, 'u'));
  }

  assert.match(buildScript, /cargo/u);
  assert.match(buildScript, /build/u);
  assert.match(buildScript, /--target/u);
  assert.match(buildScript, /sona-uniffi-bind/u);
  assert.match(buildScript, /libsona_uniffi_bind\.so/u);
  assert.match(buildScript, /jniLibs/u);
  assert.match(buildScript, /SONA_ANDROID_ABIS/u);
  assert.match(buildScript, /SONA_ANDROID_MIN_SDK/u);
  assert.match(gradleIntegration, /buildSonaUniffiAndroidLibraries/u);
  assert.match(gradleIntegration, /scripts\/build-uniffi-android-libs\.js/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_ANDROID_ABIS"\)/u);
  assert.match(gradleIntegration, /providers\.gradleProperty\("SONA_REPO_ROOT"\)/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_REPO_ROOT"\)/u);
  assert.match(gradleIntegration, /inputs\.property\("sonaAndroidAbis"/u);
  assert.match(gradleIntegration, /"--abis"/u);
  assert.match(gradleIntegration, /providers\.environmentVariable\("SONA_ANDROID_MIN_SDK"\)/u);
  assert.match(gradleIntegration, /inputs\.property\("sonaAndroidMinSdk"/u);
  assert.match(gradleIntegration, /"--min-sdk"/u);
  assert.match(gradleIntegration, /generated\/jniLibs\/main/u);
  assert.match(gradleIntegration, /jniLibs\.directories\.add\(generatedJniLibsDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /dependsOn\(buildSonaUniffiAndroidLibraries\)/u);
  assert.match(androidReadme, /buildSonaUniffiAndroidLibraries/u);
  assert.match(androidReadme, /SONA_ANDROID_ABIS/u);
});

test('UniFFI Android Gradle integration tracks build inputs for incremental reruns', () => {
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  for (const inputPath of [
    'Cargo.toml',
    'Cargo.lock',
    'scripts/generate-uniffi-kotlin.js',
    'scripts/build-uniffi-android-libs.js',
    'tools/uniffi_bindgen/Cargo.toml',
    'tools/uniffi_bindgen/src',
    'adapters/runtime_fs/Cargo.toml',
    'adapters/runtime_fs/src',
  ]) {
    assert.match(gradleIntegration, new RegExp(`inputs\\.(?:file|dir)\\(File\\(repoRoot, "${inputPath.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')}"\\)\\)`, 'u'));
  }
});

test('UniFFI Android Gradle integration avoids Gradle 10 deprecation warnings', () => {
  const gradleIntegration = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sona-uniffi-bindings.gradle.kts'),
    'utf8',
  );

  assert.doesNotMatch(gradleIntegration, /by tasks\.registering/u);
  assert.match(gradleIntegration, /val buildSonaUniffiAndroidLibraries = tasks\.register<Exec>\("buildSonaUniffiAndroidLibraries"\)/u);
  assert.match(gradleIntegration, /val generateSonaUniffiKotlin = tasks\.register<Exec>\("generateSonaUniffiKotlin"\)/u);
  assert.match(gradleIntegration, /KotlinCompile/u);
  assert.doesNotMatch(gradleIntegration, /println\(/u);
  assert.doesNotMatch(gradleIntegration, /\.srcDir\(/u);
  assert.match(gradleIntegration, /java\.directories\.add\(generatedKotlinDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /jniLibs\.directories\.add\(generatedJniLibsDir\.get\(\)\.asFile\.path\)/u);
  assert.match(gradleIntegration, /tasks\.withType<KotlinCompile>\(\)/u);
  assert.match(gradleIntegration, /source\(generatedKotlinDir\)/u);
  assert.match(gradleIntegration, /it\.name\.startsWith\("extract"\) && it\.name\.endsWith\("Annotations"\)/u);
  assert.match(gradleIntegration, /dependsOn\(generateSonaUniffiKotlin\)/u);
});

test('UniFFI Android sample consumer imports generated Kotlin bindings', () => {
  const packageJson = JSON.parse(read('package.json'));
  const verifyScript = read('scripts', 'verify-uniffi-android-sample.js');
  const sampleSettings = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'settings.gradle.kts'),
    'utf8',
  );
  const sampleRootGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'build.gradle.kts'),
    'utf8',
  );
  const sampleProperties = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'gradle.properties'),
    'utf8',
  );
  const sampleGradle = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'build.gradle.kts'),
    'utf8',
  );
  const sampleKotlin = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin', 'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt'),
    'utf8',
  );

  assert.equal(packageJson.scripts['verify:android-uniffi'], 'node scripts/verify-uniffi-android-sample.js');
  assert.match(verifyScript, /generate-uniffi-kotlin\.js/u);
  assert.match(verifyScript, /build-uniffi-android-libs\.js/u);
  assert.match(verifyScript, /sample-consumer/u);
  assert.match(verifyScript, /--require-gradle/u);
  assert.match(verifyScript, /SONA_ANDROID_ABIS/u);
  assert.match(verifyScript, /gradle\.bat|gradle/u);
  assert.match(verifyScript, /:sample-library:assembleDebug/u);
  assert.doesNotMatch(verifyScript, /:sample-library:tasks/u);

  assert.match(sampleSettings, /pluginManagement/u);
  assert.match(sampleSettings, /id\("com\.android\.library"\) version "9\.2\.1" apply false/u);
  assert.match(sampleSettings, /include\(":sample-library"\)/u);
  assert.match(sampleRootGradle, /tasks\.register\("clean", Delete::class\)/u);
  assert.match(sampleProperties, /^SONA_REPO_ROOT=\.\.\/\.\.\/\.\.\/\.\.$/mu);
  assert.match(sampleGradle, /id\("com\.android\.library"\)/u);
  assert.doesNotMatch(sampleGradle, /org\.jetbrains\.kotlin\.android|kotlin-android/u);
  assert.match(sampleGradle, /namespace = "com\.sona\.uniffi\.sample"/u);
  assert.match(sampleGradle, /compileOptions\s*\{/u);
  assert.match(sampleGradle, /compilerOptions/u);
  assert.match(sampleGradle, /apply\(from = "\.\.\/\.\.\/sona-uniffi-bindings\.gradle\.kts"\)/u);

  for (const generatedImport of [
    'FfiLlmPromptChunk',
    'FfiPolishedSegment',
    'SonaCoreBindingException',
    'defaultConfigJson',
    'parsePolishChunkJson',
    'planPolishPromptChunksJson',
  ]) {
    assert.match(sampleKotlin, new RegExp(`import uniffi\\.sona_uniffi_bind\\.${generatedImport}`, 'u'));
  }

  assert.match(sampleKotlin, /object SonaUniffiSmoke/u);
  assert.match(sampleKotlin, /planPolishPromptChunksJson\(/u);
  assert.match(sampleKotlin, /parsePolishChunkJson\(/u);
});

test('UniFFI binding errors avoid Kotlin Throwable message conflicts', () => {
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');

  assert.match(uniffiLib, /InvalidInput\s*\{\s*reason: String\s*\}/u);
  assert.doesNotMatch(uniffiLib, /InvalidInput\s*\{\s*message: String\s*\}/u);
});

test('UniFFI Android Gradle smoke verifies assembled AAR contents', () => {
  const verifyScript = read('scripts', 'verify-uniffi-android-sample.js');
  const androidReadme = read('platforms', 'android', 'README.md');

  assert.match(verifyScript, /function readZipEntries/u);
  assert.match(verifyScript, /function verifyAndroidSampleAar/u);
  assert.match(verifyScript, /sample-library-debug\.aar/u);
  assert.match(verifyScript, /jni\/arm64-v8a\/libsona_uniffi_bind\.so/u);
  assert.match(verifyScript, /classes\.jar/u);
  assert.match(verifyScript, /uniffi\/sona_uniffi_bind\//u);
  assert.match(verifyScript, /uniffi\/sona_uniffi_bind\/FfiAsrStreamingSession/u);
  assert.match(verifyScript, /uniffi\/sona_uniffi_bind\/FfiAsrStreamingObserver/u);
  assert.match(verifyScript, /com\/sona\/uniffi\/sample\/SonaUniffiSmoke/u);
  assert.match(verifyScript, /com\/sona\/uniffi\/consumer\/SonaUniffiConsumerSmoke/u);
  assert.match(androidReadme, /assembles the sample debug AAR/u);
  assert.match(androidReadme, /jni\/arm64-v8a\/libsona_uniffi_bind\.so/u);
  assert.match(androidReadme, /object\s*:\s*FfiAsrStreamingObserver/u);
  assert.match(androidReadme, /createOnlineAsrStreamingSession/u);
  assert.match(androidReadme, /verify:android-uniffi:gradle/u);
});

test('UniFFI Android sample ignores generated Gradle outputs', () => {
  const androidIgnorePath = path.join(repoRoot, 'platforms', 'android', '.gitignore');

  assert.equal(fs.existsSync(androidIgnorePath), true);
  const androidIgnore = fs.readFileSync(androidIgnorePath, 'utf8');
  const ignoreRules = androidIgnore.split(/\r?\n/u);

  for (const ignoreRule of [
    '/generated/',
    '/sample-consumer/.gradle/',
    '/sample-consumer/build/',
    '/sample-consumer/**/build/',
  ]) {
    assert.ok(ignoreRules.includes(ignoreRule), `${androidIgnorePath} must include ${ignoreRule}`);
  }
});

test('UniFFI Android sample can use a repo-managed Gradle distribution', () => {
  const packageJson = JSON.parse(read('package.json'));
  const verifyScript = read('scripts', 'verify-uniffi-android-sample.js');
  const androidReadme = read('platforms', 'android', 'README.md');
  const gradleRunnerPath = path.join(repoRoot, 'scripts', 'run-managed-gradle.js');

  assert.equal(
    packageJson.scripts['verify:android-uniffi:gradle'],
    'node scripts/verify-uniffi-android-sample.js --download-gradle --require-gradle',
  );
  assert.match(verifyScript, /--download-gradle/u);
  assert.match(verifyScript, /run-managed-gradle\.js/u);
  assert.ok(fs.existsSync(gradleRunnerPath), 'scripts/run-managed-gradle.js should exist');

  const gradleRunner = fs.readFileSync(gradleRunnerPath, 'utf8');
  assert.match(gradleRunner, /DEFAULT_GRADLE_VERSION\s*=\s*'9\.6\.1'/u);
  assert.match(gradleRunner, /MAX_DOWNLOAD_ATTEMPTS\s*=\s*3/u);
  assert.match(gradleRunner, /9c0f7faeeb306cb14e4279a3e084ca6b596894089a0638e68a07c945a32c9e14/u);
  assert.match(gradleRunner, /https:\/\/services\.gradle\.org\/distributions\/gradle-\$\{gradleVersion\}-bin\.zip/u);
  assert.match(gradleRunner, /https:\/\/downloads\.gradle\.org\/distributions\/gradle-\$\{gradleVersion\}-bin\.zip/u);
  assert.match(gradleRunner, /--distribution-zip/u);
  assert.match(gradleRunner, /SONA_GRADLE_DISTRIBUTION_ZIP/u);
  assert.match(gradleRunner, /fs\.copyFileSync\(distributionZip/u);
  assert.match(gradleRunner, /function curlDownloadFile/u);
  assert.match(gradleRunner, /--location/u);
  assert.match(gradleRunner, /--retry/u);
  assert.match(gradleRunner, /attempt < MAX_DOWNLOAD_ATTEMPTS/u);
  assert.match(gradleRunner, /target[u]?['"], ['"]managed-gradle/u);
  assert.match(gradleRunner, /Expand-Archive/u);
  assert.match(gradleRunner, /spawnSync\(gradleExecutable/u);
  assert.match(gradleRunner, /shell:\s*process\.platform === 'win32'/u);
  assert.match(gradleRunner, /result\.error/u);
  assert.match(androidReadme, /verify:android-uniffi:gradle/u);
  assert.match(androidReadme, /SONA_GRADLE_DISTRIBUTION_ZIP/u);
  assert.match(androidReadme, /target\/managed-gradle/u);
});

test('UniFFI Android native build script supports a no-toolchain dry run', () => {
  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
      '--dry-run',
      '--abis',
      'arm64-v8a',
      '--out-dir',
      path.join(os.tmpdir(), 'sona-uniffi-android-dry-run'),
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /arm64-v8a/u);
  assert.match(result.stdout, /aarch64-linux-android/u);
  assert.match(result.stdout, /libsona_uniffi_bind\.so/u);
  assert.doesNotMatch(result.stdout, /cargo build/u);
});

const androidNdkAbiCases = [
  {
    abi: 'arm64-v8a',
    target: 'aarch64-linux-android',
    linkerPrefix: 'aarch64-linux-android',
  },
  {
    abi: 'armeabi-v7a',
    target: 'armv7-linux-androideabi',
    linkerPrefix: 'armv7a-linux-androideabi',
  },
  {
    abi: 'x86',
    target: 'i686-linux-android',
    linkerPrefix: 'i686-linux-android',
  },
  {
    abi: 'x86_64',
    target: 'x86_64-linux-android',
    linkerPrefix: 'x86_64-linux-android',
  },
];

function androidNdkHostLayout(hostPlatform) {
  if (hostPlatform === 'windows' || hostPlatform === 'win32') {
    return { hostTag: 'windows-x86_64', linkerExtension: '.cmd', archiverExtension: '.exe' };
  }
  if (hostPlatform === 'darwin') {
    return { hostTag: 'darwin-x86_64', linkerExtension: '', archiverExtension: '' };
  }
  return { hostTag: 'linux-x86_64', linkerExtension: '', archiverExtension: '' };
}

function androidNdkToolPaths(ndkHome, abiCase, hostPlatform) {
  const layout = androidNdkHostLayout(hostPlatform);
  const binDir = path.join(ndkHome, 'toolchains', 'llvm', 'prebuilt', layout.hostTag, 'bin');
  return {
    linkerPath: path.join(binDir, `${abiCase.linkerPrefix}23-clang${layout.linkerExtension}`),
    archiverPath: path.join(binDir, `llvm-ar${layout.archiverExtension}`),
  };
}

function runAndroidNdkPrint({ abi, androidHome = '', ndkHome = '', hostPlatform }) {
  const commandArgs = [
    path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
    '--print-linker-env',
    '--abis',
    abi,
  ];
  if (hostPlatform) {
    commandArgs.push('--host-platform', hostPlatform);
  }

  return spawnSync(node, commandArgs, {
    cwd: repoRoot,
    encoding: 'utf8',
    env: {
      ...process.env,
      ANDROID_HOME: androidHome,
      ANDROID_SDK_ROOT: '',
      ANDROID_NDK_HOME: ndkHome,
      ANDROID_NDK_ROOT: '',
    },
  });
}

test('UniFFI Android native build script skips incomplete auto-discovered NDK installs', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-ndk-'));
  const sdkRoot = path.join(tempRoot, 'sdk');
  const validNdk = path.join(sdkRoot, 'ndk', '29.0.14206865');
  const incompleteNdk = path.join(sdkRoot, 'ndk', '30.0.15729638');

  for (const abiCase of androidNdkAbiCases) {
    const validPaths = androidNdkToolPaths(validNdk, abiCase, process.platform);
    const incompletePaths = androidNdkToolPaths(incompleteNdk, abiCase, process.platform);
    fs.mkdirSync(path.dirname(validPaths.linkerPath), { recursive: true });
    fs.mkdirSync(path.dirname(incompletePaths.linkerPath), { recursive: true });
    fs.writeFileSync(validPaths.linkerPath, '');
    fs.writeFileSync(incompletePaths.linkerPath, '');
  }
  const validArchiverPath = androidNdkToolPaths(validNdk, androidNdkAbiCases[0], process.platform).archiverPath;
  fs.writeFileSync(validArchiverPath, '');

  for (const abiCase of androidNdkAbiCases) {
    const result = runAndroidNdkPrint({ abi: abiCase.abi, androidHome: sdkRoot });
    const validPaths = androidNdkToolPaths(validNdk, abiCase, process.platform);
    const targetEnvSuffix = abiCase.target.replace(/-/gu, '_');
    const expectedLines = [
      `CARGO_TARGET_${targetEnvSuffix.toUpperCase()}_LINKER=${validPaths.linkerPath}`,
      `CC_${targetEnvSuffix}=${validPaths.linkerPath}`,
      `AR_${targetEnvSuffix}=${validPaths.archiverPath}`,
    ];

    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.deepEqual(result.stdout.trim().split(/\r?\n/u), expectedLines);
    assert.doesNotMatch(result.stdout, /30\.0\.15729638/u);
  }
});

test('UniFFI Android native build script reports a missing linker in an explicit NDK', () => {
  const ndkHome = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-explicit-ndk-'));
  const abiCase = androidNdkAbiCases[0];
  const toolPaths = androidNdkToolPaths(ndkHome, abiCase, process.platform);
  fs.mkdirSync(path.dirname(toolPaths.archiverPath), { recursive: true });
  fs.writeFileSync(toolPaths.archiverPath, '');

  const result = runAndroidNdkPrint({ abi: abiCase.abi, ndkHome });
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.ok(output.includes(`Missing Android NDK linker at ${toolPaths.linkerPath}`), output);
});

test('UniFFI Android native build script reports a missing archiver in an explicit NDK', () => {
  const ndkHome = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-android-explicit-ndk-'));
  const abiCase = androidNdkAbiCases[0];
  const toolPaths = androidNdkToolPaths(ndkHome, abiCase, process.platform);
  fs.mkdirSync(path.dirname(toolPaths.linkerPath), { recursive: true });
  fs.writeFileSync(toolPaths.linkerPath, '');

  const result = runAndroidNdkPrint({ abi: abiCase.abi, ndkHome });
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.ok(output.includes(`Missing Android NDK archiver at ${toolPaths.archiverPath}`), output);
});

test('UniFFI Android native build script supports injected host toolchain layouts in print mode', () => {
  const abiCase = androidNdkAbiCases[0];

  for (const hostPlatform of ['windows', 'linux', 'darwin']) {
    const ndkHome = fs.mkdtempSync(path.join(os.tmpdir(), `sona-android-${hostPlatform}-ndk-`));
    const toolPaths = androidNdkToolPaths(ndkHome, abiCase, hostPlatform);
    fs.mkdirSync(path.dirname(toolPaths.linkerPath), { recursive: true });
    fs.writeFileSync(toolPaths.linkerPath, '');
    fs.writeFileSync(toolPaths.archiverPath, '');

    const result = runAndroidNdkPrint({ abi: abiCase.abi, ndkHome, hostPlatform });
    const targetEnvSuffix = abiCase.target.replace(/-/gu, '_');

    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.deepEqual(result.stdout.trim().split(/\r?\n/u), [
      `CARGO_TARGET_${targetEnvSuffix.toUpperCase()}_LINKER=${toolPaths.linkerPath}`,
      `CC_${targetEnvSuffix}=${toolPaths.linkerPath}`,
      `AR_${targetEnvSuffix}=${toolPaths.archiverPath}`,
    ]);
  }
});

test('UniFFI Android native build script rejects host overrides outside print mode', () => {
  const result = spawnSync(
    node,
    [
      path.join(repoRoot, 'scripts', 'build-uniffi-android-libs.js'),
      '--dry-run',
      '--host-platform',
      'linux',
      '--abis',
      'arm64-v8a',
    ],
    { cwd: repoRoot, encoding: 'utf8' },
  );
  const output = `${result.stderr}\n${result.stdout}`;

  assert.notEqual(result.status, 0, output);
  assert.match(output, /--host-platform is only supported with --print-linker-env/u);
});

test('core owns LLM request field validation while online adapter owns API host policy', () => {
  const coreRequests = read('core', 'src', 'llm', 'requests.rs');
  const coreCargo = read('core', 'Cargo.toml');
  const desktopTasks = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'llm', 'tasks.rs'),
    'utf8',
  );
  const desktopCommands = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'llm', 'commands.rs'),
    'utf8',
  );
  const onlineLlm = read('adapters', 'online_llm', 'src', 'lib.rs');

  assert.doesNotMatch(coreCargo, /^url\s*=/mu);
  assert.doesNotMatch(coreRequests, /use url::Url/u);
  assert.doesNotMatch(coreRequests, /pub fn validate_llm_api_host/u);
  assert.doesNotMatch(coreRequests, /fn is_loopback_host/u);
  assert.match(coreRequests, /pub fn validate_llm_config/u);
  assert.match(coreRequests, /pub fn validate_task_request/u);
  assert.match(coreRequests, /pub fn validate_llm_generate_request/u);
  assert.match(coreRequests, /pub fn validate_polish_segments_request/u);
  assert.match(coreRequests, /pub fn validate_translate_segments_request/u);
  assert.match(coreRequests, /pub fn validate_summarize_transcript_request/u);
  assert.doesNotMatch(desktopTasks, /fn validate_llm_config/u);
  assert.doesNotMatch(desktopTasks, /fn validate_task_request/u);
  assert.match(desktopCommands, /validate_llm_generate_request\(&request\)/u);
  assert.match(desktopCommands, /validate_polish_segments_request\(&request\)/u);
  assert.match(desktopCommands, /validate_translate_segments_request\(&request\)/u);
  assert.match(desktopCommands, /validate_summarize_transcript_request\(&request\)/u);
  assert.doesNotMatch(desktopCommands, /Input cannot be empty/u);
  assert.doesNotMatch(desktopCommands, /Target language cannot be empty/u);
  assert.doesNotMatch(desktopCommands, /does not support transcript polishing/u);
  assert.match(desktopTasks, /sona_core::llm::requests(?:::\{[\s\S]*validate_llm_config|::validate_llm_config)/u);
  assert.match(onlineLlm, /pub fn validate_llm_api_host/u);
  assert.match(onlineLlm, /fn is_loopback_host/u);
  assert.doesNotMatch(onlineLlm, /sona_core::llm::requests::validate_llm_api_host/u);
});

test('online ASR provider manifest is owned by core and used directly by desktop', () => {
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const coreManifestPath = path.join(repoRoot, 'core', 'src', 'ports', 'online-asr-providers.json');
  const legacySharedManifestPath = path.join(repoRoot, 'src', 'shared', 'online-asr-providers.json');
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const apiServer = read('adapters', 'api_server', 'src', 'lib.rs');
  const streamingRs = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');
  const onlineAdapterRs = ['groq.rs', 'mistral.rs', 'volcengine.rs']
    .map((file) => read(...desktopCrateSegments, 'src', 'integrations', 'asr', file))
    .join('\n');
  const tsBindLib = read('adapters', 'ts_bind', 'src', 'lib.rs');
  const onlineProvidersTs = read('src', 'services', 'onlineAsrProviders.ts');
  const asrConfigServiceTest = fs.readFileSync(
    path.join(repoRoot, 'src', 'services', '__tests__', 'asrConfigService.test.ts'),
    'utf8',
  );

  assert.ok(fs.existsSync(coreManifestPath));
  assert.equal(fs.existsSync(legacySharedManifestPath), false);
  assert.match(coreAsr, /ONLINE_ASR_PROVIDERS_JSON/u);
  assert.match(coreAsr, /include_str!\("online-asr-providers\.json"\)/u);
  assert.doesNotMatch(coreAsr, /src\/shared\/online-asr-providers\.json/u);
  assert.match(coreAsr, /pub const VOLCENGINE_DOUBAO_PROVIDER_ID/u);
  assert.match(coreAsr, /pub fn online_asr_providers\(\)/u);
  assert.match(coreAsr, /pub fn find_online_asr_provider/u);
  assert.match(coreAsr, /pub struct OnlineAsrProvider/u);
  assert.match(coreAsr, /pub struct OnlineAsrBatchCapability/u);
  assert.match(coreAsr, /pub fn provider_id\(&self\) -> &str/u);

  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'asr_providers.rs'), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod asr_providers;/mu);
  assert.match(apiServer, /sona_core::ports::asr::\{[^}]*find_online_asr_provider[^}]*online_asr_providers/u);
  assert.match(streamingRs, /sona_core::ports::asr::find_online_asr_provider/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::GROQ_WHISPER_PROVIDER_ID/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::MISTRAL_VOXTRAL_PROVIDER_ID/u);
  assert.match(onlineAdapterRs, /sona_core::ports::asr::VOLCENGINE_DOUBAO_PROVIDER_ID/u);

  assert.match(tsBindLib, /OnlineAsrProvider/u);
  assert.match(tsBindLib, /OnlineAsrCapability/u);
  assert.match(tsBindLib, /OnlineAsrBatchCapability/u);
  assert.match(tsBindLib, /OnlineAsrLocalFileBatchMode/u);
  assert.match(onlineProvidersTs, /\.\.\/\.\.\/core\/src\/ports\/online-asr-providers\.json/u);
  assert.match(asrConfigServiceTest, /\.\.\/\.\.\/\.\.\/core\/src\/ports\/online-asr-providers\.json/u);
});

test('preset model catalog data is owned by core and reused by frontend', () => {
  const corePresetModels = read('core', 'src', 'models', 'preset_models.rs');
  const corePresetModelsPath = path.join(repoRoot, 'core', 'src', 'models', 'preset-models.json');
  const legacySharedPresetModelsPath = path.join(repoRoot, 'src', 'shared', 'preset-models.json');
  const modelServiceTs = read('src', 'services', 'modelService.ts');

  assert.ok(fs.existsSync(corePresetModelsPath));
  assert.equal(fs.existsSync(legacySharedPresetModelsPath), false);
  assert.match(corePresetModels, /include_str!\("preset-models\.json"\)/u);
  assert.doesNotMatch(corePresetModels, /src\/shared\/preset-models\.json/u);
  assert.match(modelServiceTs, /\.\.\/\.\.\/core\/src\/models\/preset-models\.json/u);
});

test('core model path resolution is adapter-driven without desktop filesystem probes', () => {
  const coreModelPaths = read('core', 'src', 'models', 'paths.rs');
  const cliDesktopPaths = read('platforms', 'cli', 'src', 'desktop_paths.rs');
  const cliModels = read('platforms', 'cli', 'src', 'models.rs');
  const coreTranscribeRuntime = read('core', 'src', 'transcription', 'runtime.rs');
  const coreServeRuntime = read('core', 'src', 'runtime', 'serve.rs');

  assert.match(coreModelPaths, /pub enum ModelsDirStatus/u);
  assert.match(coreModelPaths, /status_of/u);
  assert.doesNotMatch(coreModelPaths, /default_desktop_models_dir/u);
  assert.doesNotMatch(coreModelPaths, /std::fs::metadata/u);
  assert.match(cliDesktopPaths, /pub fn models_dir_status/u);
  assert.match(cliModels, /crate::desktop_paths::default_models_dir/u);
  assert.match(cliModels, /crate::desktop_paths::models_dir_status/u);
  assert.match(coreTranscribeRuntime, /default_models_dir: Option<PathBuf>/u);
  assert.match(coreServeRuntime, /default_models_dir: Option<PathBuf>/u);
});

test('desktop platform adapters own Tauri path event diagnostics and preset model bridges', () => {
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformPaths = read(...desktopCrateSegments, 'src', 'platform', 'paths.rs');
  const platformEvent = read(...desktopCrateSegments, 'src', 'platform', 'event.rs');
  const platformPresetModels = fs.readFileSync(
    desktopCratePath('src', 'platform', 'preset_models.rs'),
    'utf8',
  );
  const platformDiagnostics = fs.readFileSync(
    desktopCratePath('src', 'platform', 'diagnostics.rs'),
    'utf8',
  );
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');

  assert.match(desktopPlatform, /^pub mod paths;/mu);
  assert.match(desktopPlatform, /^pub mod event;/mu);
  assert.match(desktopPlatform, /^pub mod preset_models;/mu);
  assert.match(desktopPlatform, /^pub mod diagnostics;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'paths.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'event.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'preset_models.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'diagnostics.rs'), false);
  assert.match(platformPaths, /pub struct TauriPathProvider/u);
  assert.match(platformPaths, /impl<R: Runtime> PathProvider for TauriPathProvider<R>/u);
  assert.match(platformEvent, /pub struct TauriEventEmitter<R: Runtime>\(pub AppHandle<R>\)/u);
  assert.match(platformEvent, /impl<R: Runtime> EventEmitter for TauriEventEmitter<R>/u);
  assert.match(platformPresetModels, /pub use sona_core::models::preset_models::\*/u);
  assert.match(platformPresetModels, /tauri::async_runtime::spawn_blocking/u);
  assert.match(platformPresetModels, /pub async fn get_model_catalog_snapshot_for_app/u);
  assert.match(platformPresetModels, /pub async fn resolve_model_catalog_selected_ids_for_app/u);
  assert.match(platformDiagnostics, /pub use sona_core::runtime::diagnostics::\{/u);
  assert.match(platformDiagnostics, /crate::platform::paths::\{PathKind, PathProvider\}/u);
  assert.match(platformDiagnostics, /pub async fn get_diagnostics_core_snapshot_for_app/u);
  assert.match(systemCommand, /crate::platform::preset_models::get_model_catalog_snapshot_for_app\(&app\)\.await/u);
  assert.match(systemCommand, /crate::platform::preset_models::resolve_model_catalog_selected_ids_for_app\(&app, paths\)\.await/u);
  assert.match(systemCommand, /crate::platform::diagnostics::get_diagnostics_core_snapshot_for_app\(&app, state, input\)\.await/u);
  assert.doesNotMatch(systemCommand, /TauriPathProvider/u);
});

test('desktop runtime status adapter lives in platform layer', () => {
  const appMod = read(...desktopCrateSegments, 'src', 'app', 'mod.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformDiagnostics = fs.readFileSync(
    desktopCratePath('src', 'platform', 'diagnostics.rs'),
    'utf8',
  );
  const platformRuntimeStatusPath = desktopCratePath('src', 'platform', 'runtime_status.rs');

  assert.equal(fs.existsSync(platformRuntimeStatusPath), true);
  const platformRuntimeStatus = fs.readFileSync(platformRuntimeStatusPath, 'utf8');

  assert.match(platformMod, /^pub mod runtime_status;/mu);
  assert.doesNotMatch(appMod, /^pub mod runtime_status;/mu);
  assert.match(platformRuntimeStatus, /pub async fn open_log_folder/u);
  assert.match(platformRuntimeStatus, /pub fn resolve_runtime_environment_status/u);
  assert.match(platformRuntimeStatus, /pub async fn get_runtime_environment_status/u);
  assert.match(platformRuntimeStatus, /pub async fn get_path_statuses/u);
  assert.match(platformRuntimeStatus, /sona_runtime_fs::ensure_directory_exists\(&log_dir\)/u);
  assert.match(systemCommand, /crate::platform::runtime_status::open_log_folder\(app\)\.await/u);
  assert.match(systemCommand, /crate::platform::runtime_status::get_runtime_environment_status\(app\)\.await/u);
  assert.match(systemCommand, /crate::platform::runtime_status::get_path_statuses\(paths\)\.await/u);
  assert.match(platformDiagnostics, /crate::platform::runtime_status::resolve_runtime_environment_status\(provider\)/u);
  assert.match(platformDiagnostics, /crate::platform::runtime_status::resolve_runtime_path_status\(path\)/u);
  assert.doesNotMatch(systemCommand, /crate::app::runtime_status/u);
  assert.doesNotMatch(platformDiagnostics, /crate::app::runtime_status/u);
});

test('desktop tauri crate imports core crates directly without a local core shim', () => {
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const desktopRust = rustFilesUnder(desktopCratePath('src'))
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');

  assert.doesNotMatch(desktopLib, /^pub mod core;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core'), false);
  assert.doesNotMatch(desktopRust, /crate::core::/u);
  assert.match(desktopRust, /sona_core::/u);
  assert.match(desktopRust, /sona_sqlite::/u);
});

test('desktop filesystem adapters live in platform rather than repositories', () => {
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformStorage = read(...desktopCrateSegments, 'src', 'platform', 'file_storage.rs');
  const platformRecovery = fs.readFileSync(
    desktopCratePath('src', 'platform', 'recovery_repository.rs'),
    'utf8',
  );
  const workspaceCargo = read('Cargo.toml');
  const coreExport = read('core', 'src', 'export', 'mod.rs');
  const exportAdapter = read('adapters', 'export', 'src', 'lib.rs');
  const exportCommand = read(...desktopCrateSegments, 'src', 'commands', 'export.rs');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');

  assert.doesNotMatch(desktopPlatform, /^pub mod export_files;/mu);
  assert.match(desktopPlatform, /^pub mod file_storage;/mu);
  assert.match(desktopPlatform, /^pub mod recovery_repository;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'platform', 'export_files.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'export.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'storage.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'recovery.rs'), false);
  assert.match(platformStorage, /pub use sona_runtime_fs::\{/u);
  assert.match(platformRecovery, /pub struct FsRecoveryRepository/u);
  assert.match(workspaceCargo, /"adapters\/export"/u);
  assert.match(tauriCargo, /^sona-export\s*=/mu);
  assert.doesNotMatch(cliCargo, /^sona-export\s*=/mu);
  assert.match(coreExport, /pub fn export_segments_with_mode/u);
  assert.doesNotMatch(coreExport, /std::fs::write/u);
  assert.doesNotMatch(coreExport, /pub fn export_transcript_file/u);
  assert.match(exportAdapter, /pub fn export_transcript_file/u);
  assert.match(exportAdapter, /sona_core::export::export_segments_with_mode/u);
  assert.match(exportCommand, /use sona_export::\{[\s\S]*export_transcript_file as adapter_export_transcript_file/u);
  assert.match(exportCommand, /adapter_export_transcript_file\(ExportTranscriptFileRequest/u);
  assert.doesNotMatch(exportCommand, /core_export_transcript_file/u);
  assert.doesNotMatch(exportCommand, /crate::platform::export_files/u);
  assert.match(platformRecovery, /async fn run_recovery_repository_task/u);
  assert.match(platformRecovery, /PathKind::AppLocalData/u);
  assert.match(platformRecovery, /tauri::async_runtime::spawn_blocking/u);
  assert.match(platformRecovery, /pub async fn load_snapshot/u);
  assert.match(platformRecovery, /pub async fn save_snapshot/u);
  assert.match(platformRecovery, /pub async fn persist_queue_snapshot/u);
  assert.match(platformRecovery, /pub async fn load_snapshot_for_app/u);
  assert.match(platformRecovery, /pub async fn save_snapshot_for_app/u);
  assert.match(platformRecovery, /pub async fn persist_queue_snapshot_for_app/u);
  assert.match(systemCommand, /crate::platform::recovery_repository::load_snapshot_for_app\(&app\)\.await/u);
  assert.match(systemCommand, /crate::platform::recovery_repository::save_snapshot_for_app\(&app, items\)\.await/u);
  assert.match(
    systemCommand,
    /crate::platform::recovery_repository::persist_queue_snapshot_for_app\(\s*&app,\s*queue_items,\s*resolved_ids\s*,?\s*\)\s*\.await/u,
  );
  assert.doesNotMatch(systemCommand, /FsRecoveryRepository/u);
  assert.doesNotMatch(systemCommand, /run_recovery_repository_task/u);
  assert.doesNotMatch(systemCommand, /PathKind::AppLocalData/u);
  assert.doesNotMatch(systemCommand, /tauri::async_runtime::spawn_blocking/u);
});

test('desktop automation and project repository task adapters live in platform', () => {
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformAutomation = fs.readFileSync(
    desktopCratePath('src', 'platform', 'automation_repository.rs'),
    'utf8',
  );
  const platformProject = fs.readFileSync(
    desktopCratePath('src', 'platform', 'project_repository.rs'),
    'utf8',
  );
  const automationCommand = read(...desktopCrateSegments, 'src', 'commands', 'automation.rs');
  const projectCommand = read(...desktopCrateSegments, 'src', 'commands', 'project.rs');

  assert.match(desktopPlatform, /^pub mod automation_repository;/mu);
  assert.match(desktopPlatform, /^pub mod project_repository;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'automation.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'automation'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'project.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'project'), false);
  assert.match(platformAutomation, /pub fn validate_rule_activation_inner/u);
  assert.match(platformAutomation, /sona_sqlite::automation::SqliteAutomationRepository/u);
  assert.match(platformProject, /sona_sqlite::project::SqliteProjectRepository/u);
  assert.match(automationCommand, /crate::platform::automation_repository::load_repository_state/u);
  assert.match(projectCommand, /crate::platform::project_repository::\{/u);
  assert.match(automationCommand, /sona_core::automation::\{/u);
  assert.match(projectCommand, /sona_core::project::\{/u);
});

test('desktop history repository facade lives in platform without repositories module', () => {
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformHistory = fs.readFileSync(
    desktopCratePath('src', 'platform', 'history_repository.rs'),
    'utf8',
  );
  const platformDashboardPath = desktopCratePath('src', 'platform', 'dashboard.rs');
  const platformDashboard = fs.existsSync(platformDashboardPath) ? fs.readFileSync(platformDashboardPath, 'utf8') : '';
  const dashboardApp = read(...desktopCrateSegments, 'src', 'app', 'dashboard.rs');
  const setupApp = read(...desktopCrateSegments, 'src', 'app', 'setup.rs');
  const historyCommand = read(...desktopCrateSegments, 'src', 'commands', 'history.rs');
  const historyLlmHelpers = fs.readFileSync(
    desktopCratePath('src', 'platform', 'history_repository', 'llm_helpers.rs'),
    'utf8',
  );
  const desktopRust = rustFilesUnder(desktopCratePath('src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /crate::repositories|repositories::history/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));
  const historyTypeShimReferences = rustFilesUnder(desktopCratePath('src', 'platform', 'history_repository'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /super::types|history::types/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.doesNotMatch(desktopLib, /^pub mod repositories;/mu);
  assert.equal(fs.existsSync(platformDashboardPath), true);
  assert.match(desktopPlatform, /^pub mod dashboard;/mu);
  assert.match(desktopPlatform, /^pub mod history_repository;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories'), false);
  assert.ok(exists(...desktopCrateSegments, 'src', 'platform', 'history_repository', 'llm_helpers.rs'));
  assert.ok(exists(...desktopCrateSegments, 'src', 'platform', 'history_repository', 'state.rs'));
  assert.equal(exists(...desktopCrateSegments, 'src', 'platform', 'history_repository', 'types.rs'), false);
  assert.doesNotMatch(platformHistory, /^mod types;/mu);
  assert.deepEqual(historyTypeShimReferences, []);
  assert.deepEqual(desktopRust, []);
  assert.match(platformHistory, /pub use sona_core::history::\{/u);
  assert.match(platformHistory, /pub async fn run_history_db_task/u);
  assert.match(platformHistory, /pub async fn run_history_file_task/u);
  assert.match(platformHistory, /pub async fn export_backup_archive/u);
  assert.match(platformHistory, /pub async fn prepare_backup_import/u);
  assert.match(platformHistory, /pub async fn apply_prepared_history_import/u);
  assert.match(platformHistory, /pub async fn dispose_prepared_backup_import/u);
  assert.match(platformHistory, /pub async fn open_history_folder/u);
  assert.match(
    platformHistory,
    /pub use sona_core::history::transcript_diff::\{[\s\S]*build_transcript_diff[\s\S]*restore_transcript_diff_rows[\s\S]*\};/u,
  );
  assert.match(historyLlmHelpers, /use super::\{[\s\S]*SqliteHistoryStore[\s\S]*\};/u);
  assert.doesNotMatch(historyLlmHelpers, /crate::platform::history_repository::sqlite_store::SqliteHistoryStore/u);
  assert.match(platformHistory, /SqliteHistoryStore::new\(app_local_data_dir\.clone\(\), db\)/u);
  assert.match(platformHistory, /app\.opener\(\)[\s\S]*\.open_path\(/u);
  assert.match(historyCommand, /crate::platform::history_repository::run_history_file_task\(\s*&app,\s*state\.inner\(\),/u);
  assert.match(historyCommand, /crate::platform::history_repository::run_history_db_task\(\s*&app,/u);
  assert.match(historyCommand, /crate::platform::history_repository::export_backup_archive\(\s*&app,\s*history_state\.inner\(\),\s*request\s*,?\s*\)\s*\.await/u);
  assert.match(historyCommand, /crate::platform::history_repository::prepare_backup_import\(\s*state\.inner\(\),\s*archive_path\s*,?\s*\)\s*\.await/u);
  assert.match(historyCommand, /crate::platform::history_repository::apply_prepared_history_import\(\s*&app,\s*history_state\.inner\(\),\s*prepared_state\.inner\(\),\s*import_id\s*,?\s*\)\s*\.await/u);
  assert.match(historyCommand, /crate::platform::history_repository::dispose_prepared_backup_import\(\s*state\.inner\(\),\s*import_id\s*,?\s*\)\s*\.await/u);
  assert.match(historyCommand, /crate::platform::history_repository::open_history_folder\(&app, state\.inner\(\)\)\.await/u);
  assert.match(historyCommand, /crate::platform::history_repository::build_transcript_diff\(/u);
  assert.match(historyCommand, /crate::platform::history_repository::restore_transcript_diff_rows\(/u);
  assert.doesNotMatch(historyCommand, /run_history_file_task_inner/u);
  assert.doesNotMatch(historyCommand, /crate::platform::history_repository::transcript_diff::/u);
  assert.doesNotMatch(historyCommand, /sona_sqlite::Database/u);
  assert.doesNotMatch(historyCommand, /SqliteHistoryStore/u);
  assert.doesNotMatch(historyCommand, /HistoryStoreError/u);
  assert.doesNotMatch(historyCommand, /tauri::async_runtime::spawn_blocking/u);
  assert.doesNotMatch(historyCommand, /crate::platform::database::sqlite_database/u);
  assert.doesNotMatch(historyCommand, /TauriPathProvider|PathKind|PathProvider/u);
  assert.doesNotMatch(historyCommand, /export_backup_archive_inner|prepare_backup_import_inner|apply_prepared_history_import_inner/u);
  assert.doesNotMatch(historyCommand, /remove_path_if_exists/u);
  assert.doesNotMatch(historyCommand, /tauri_plugin_opener::OpenerExt|\.open_path\(/u);
  assert.match(platformDashboard, /DashboardService<SqliteHistoryStore, SqliteProjectRepository, SqliteAnalyticsRepository>/u);
  assert.match(platformDashboard, /pub fn create_dashboard_service/u);
  assert.match(platformDashboard, /SqliteHistoryStore::new/u);
  assert.match(dashboardApp, /pub use crate::platform::dashboard::AppDashboardService/u);
  assert.doesNotMatch(dashboardApp, /sona_sqlite::|SqliteHistoryStore|SqliteProjectRepository|SqliteAnalyticsRepository/u);
  assert.match(setupApp, /crate::platform::dashboard::create_dashboard_service/u);
  assert.doesNotMatch(setupApp, /crate::platform::history_repository::SqliteHistoryStore::new/u);
  assert.doesNotMatch(setupApp, /sona_sqlite::project::SqliteProjectRepository::new/u);
  assert.doesNotMatch(setupApp, /sona_sqlite::analytics::SqliteAnalyticsRepository::new/u);
});

test('app config migration and LLM provider manifest are owned by core', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const coreConfig = read('core', 'src', 'config', 'mod.rs');
  const coreDefaults = read('core', 'src', 'config', 'defaults.rs');
  const coreMigration = read('core', 'src', 'config', 'migration.rs');
  const desktopSystemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const desktopServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const desktopServerRuntime = desktopServer.split(/#\[cfg\(test\)\]/u)[0];
  const platformApiServerConfig = fs.readFileSync(
    desktopCratePath('src', 'platform', 'api_server_config.rs'),
    'utf8',
  );
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const llmProvidersTs = read('src', 'services', 'llm', 'providers.ts');

  assert.match(coreLib, /^pub mod config;/mu);
  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod providers;/mu);
  assert.ok(exists('core', 'src', 'llm', 'llm-providers.json'));
  assert.equal(exists('src', 'shared', 'llm-providers.json'), false);
  assert.match(coreConfig, /^pub mod defaults;/mu);
  assert.match(coreConfig, /pub fn migrate_app_config/u);
  assert.match(coreDefaults, /crate::ports::asr::online_asr_providers/u);
  assert.match(coreMigration, /crate::llm::providers::find_llm_provider_by_id_or_alias/u);
  assert.match(desktopSystemCommand, /sona_core::config::migrate_app_config/u);
  assert.match(platformApiServerConfig, /sona_sqlite::config_store::\{/u);
  assert.doesNotMatch(desktopServerRuntime, /sona_sqlite::config_store/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'config'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'config', 'defaults.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'config', 'migration.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'config', 'types.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'config', 'error.rs'), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod llm_providers;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'llm_providers.rs'), false);
  assert.match(llmProvidersTs, /\.\.\/\.\.\/\.\.\/core\/src\/llm\/llm-providers\.json/u);
});

test('SQLite database handle and schema are owned by sqlite adapter', () => {
  const workspaceCargo = read('Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const desktopSetup = read(...desktopCrateSegments, 'src', 'app', 'setup.rs');
  const platformDatabase = read(...desktopCrateSegments, 'src', 'platform', 'database.rs');
  const sqliteCargoPath = path.join(repoRoot, 'adapters', 'sqlite', 'Cargo.toml');
  const sqliteLibPath = path.join(repoRoot, 'adapters', 'sqlite', 'src', 'lib.rs');
  const sqliteMigrationPath = path.join(repoRoot, 'adapters', 'sqlite', 'src', 'legacy_migration.rs');
  const sqliteLib = fs.readFileSync(sqliteLibPath, 'utf8');

  assert.match(workspaceCargo, /"adapters\/sqlite"/u);
  assert.ok(fs.existsSync(sqliteCargoPath));
  assert.ok(fs.existsSync(sqliteLibPath));
  assert.ok(fs.existsSync(sqliteMigrationPath));
  assert.match(sqliteLib, /^pub mod legacy_migration;/mu);
  assert.match(tauriCargo, /sona-sqlite\s*=\s*\{\s*path\s*=\s*"..\/..\/adapters\/sqlite"/u);
  assert.match(platformDatabase, /pub fn open_and_migrate_sqlite_for_app/u);
  assert.match(platformDatabase, /sona_sqlite::Database::open/u);
  assert.match(platformDatabase, /sona_sqlite::legacy_migration::migrate_legacy_to_sqlite/u);
  assert.match(platformDatabase, /sona_sqlite::legacy_migration::move_legacy_domains_to_backup/u);
  assert.match(desktopSetup, /crate::platform::database::open_and_migrate_sqlite_for_app\(&app_handle_for_listener\)\?/u);
  assert.doesNotMatch(desktopSetup, /sona_sqlite::Database::open/u);
  assert.doesNotMatch(desktopSetup, /sona_sqlite::legacy_migration::migrate_legacy_to_sqlite/u);
  assert.doesNotMatch(desktopSetup, /sona_sqlite::legacy_migration::move_legacy_domains_to_backup/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'database'), false);
  assert.equal(
    exists(...desktopCrateSegments, 'src', 'core', 'database', 'legacy_migration.rs'),
    false,
  );
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'database', 'error.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'database', 'ports.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'database', 'schema.rs'), false);
  assert.doesNotMatch(desktopSetup, /crate::core::database/u);
  assert.doesNotMatch(desktopSetup, /rusqlite::Connection/u);
  assert.doesNotMatch(desktopSetup, /struct ConnectionPool/u);
  assert.doesNotMatch(desktopSetup, /pub struct Database/u);
});

test('SQLite config and task ledger stores are owned by sqlite adapter', () => {
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const desktopSystemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformDatabase = read(...desktopCrateSegments, 'src', 'platform', 'database.rs');
  const platformAppConfigPath = desktopCratePath('src', 'platform', 'app_config.rs');
  const taskLedgerRepositoryPath = desktopCratePath('src', 'platform', 'task_ledger_repository.rs');

  assert.ok(exists('adapters', 'sqlite', 'src', 'config_store.rs'));
  assert.ok(exists('adapters', 'sqlite', 'src', 'task_ledger.rs'));
  assert.equal(fs.existsSync(platformAppConfigPath), true);
  assert.equal(fs.existsSync(taskLedgerRepositoryPath), true);
  const platformAppConfig = fs.readFileSync(platformAppConfigPath, 'utf8');
  const platformTaskLedgerRepository = fs.readFileSync(taskLedgerRepositoryPath, 'utf8');
  assert.match(sqliteLib, /^pub mod config_store;/mu);
  assert.match(sqliteLib, /^pub mod task_ledger;/mu);
  assert.match(sqliteLib, /^pub use config_store::SqliteConfigStore;/mu);
  assert.match(sqliteLib, /^pub use task_ledger::SqliteLedgerRepository;/mu);
  assert.match(platformDatabase, /sona_sqlite::config_store::SqliteConfigStore/u);
  assert.match(platformAppConfig, /crate::platform::database::sqlite_config_store/u);
  assert.match(platformTaskLedgerRepository, /sona_sqlite::task_ledger::SqliteLedgerRepository/u);
  assert.doesNotMatch(desktopSystemCommand, /crate::platform::database::sqlite_config_store/u);
  assert.doesNotMatch(desktopSystemCommand, /sona_sqlite::task_ledger::SqliteLedgerRepository/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'config', 'sqlite_store.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'task_ledger', 'sqlite_repository.rs'), false);
});

test('desktop app config store commands delegate to platform adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformAppConfigPath = desktopCratePath('src', 'platform', 'app_config.rs');

  assert.equal(fs.existsSync(platformAppConfigPath), true);
  const platformAppConfig = fs.readFileSync(platformAppConfigPath, 'utf8');

  assert.match(platformMod, /^pub mod app_config;/mu);
  assert.match(platformAppConfig, /pub fn load_config/u);
  assert.match(platformAppConfig, /pub fn save_config/u);
  assert.match(platformAppConfig, /pub fn get_setting/u);
  assert.match(platformAppConfig, /pub fn set_setting/u);
  assert.match(platformAppConfig, /crate::platform::database::sqlite_config_store/u);
  assert.match(systemCommand, /crate::platform::app_config::load_config\(&app\)/u);
  assert.match(systemCommand, /crate::platform::app_config::save_config\(&app, config\)/u);
  assert.match(systemCommand, /crate::platform::app_config::get_setting\(&app, key\)/u);
  assert.match(systemCommand, /crate::platform::app_config::set_setting\(&app, key, value\)/u);
  assert.doesNotMatch(systemCommand, /sqlite_config_store/u);
  assert.doesNotMatch(systemCommand, /SqliteConfigStore/u);
});

test('desktop task ledger repository adapter lives in platform layer', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformTaskLedgerPath = desktopCratePath('src', 'platform', 'task_ledger_repository.rs');

  assert.equal(fs.existsSync(platformTaskLedgerPath), true);
  const platformTaskLedger = fs.readFileSync(platformTaskLedgerPath, 'utf8');

  assert.match(platformMod, /^pub mod task_ledger_repository;/mu);
  assert.match(platformTaskLedger, /sona_sqlite::task_ledger::SqliteLedgerRepository/u);
  assert.match(platformTaskLedger, /TASK_LEDGER_UPDATED_EVENT/u);
  assert.match(platformTaskLedger, /async fn run_task_ledger_repository_task/u);
  assert.match(platformTaskLedger, /fn emit_task_ledger_snapshot/u);
  assert.match(platformTaskLedger, /app\.emit\(TASK_LEDGER_UPDATED_EVENT, snapshot\)/u);
  assert.match(platformTaskLedger, /pub async fn load_snapshot/u);
  assert.match(platformTaskLedger, /pub async fn upsert_task/u);
  assert.match(platformTaskLedger, /pub async fn patch_task/u);
  assert.match(platformTaskLedger, /pub async fn remove_task/u);
  assert.match(platformTaskLedger, /pub async fn clear_resolved/u);
  assert.match(systemCommand, /crate::platform::task_ledger_repository::load_snapshot\(&app\)\.await/u);
  assert.match(systemCommand, /crate::platform::task_ledger_repository::upsert_task\(&app, record\)\.await/u);
  assert.match(systemCommand, /crate::platform::task_ledger_repository::patch_task\(&app, id, patch\)\.await/u);
  assert.match(systemCommand, /crate::platform::task_ledger_repository::remove_task\(&app, id\)\.await/u);
  assert.match(systemCommand, /crate::platform::task_ledger_repository::clear_resolved\(&app\)\.await/u);
  assert.doesNotMatch(systemCommand, /TASK_LEDGER_UPDATED_EVENT/u);
  assert.doesNotMatch(systemCommand, /SqliteLedgerRepository/u);
  assert.doesNotMatch(systemCommand, /run_task_ledger_repository_task/u);
  assert.doesNotMatch(systemCommand, /emit_task_ledger_snapshot/u);
});

test('SQLite automation repository is owned by sqlite adapter', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreAutomationPath = path.join(repoRoot, 'core', 'src', 'automation', 'mod.rs');
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const platformAutomationRepository = fs.readFileSync(
    desktopCratePath('src', 'platform', 'automation_repository.rs'),
    'utf8',
  );
  const desktopAutomationCommand = read(...desktopCrateSegments, 'src', 'commands', 'automation.rs');

  assert.ok(fs.existsSync(coreAutomationPath));
  assert.match(coreLib, /^pub mod automation;/mu);
  assert.ok(exists('adapters', 'sqlite', 'src', 'automation.rs'));
  assert.match(sqliteLib, /^pub mod automation;/mu);
  assert.match(sqliteLib, /^pub use automation::\{AutomationRepositoryState, SqliteAutomationRepository\};/mu);
  assert.match(platformAutomationRepository, /pub use sona_sqlite::automation::AutomationRepositoryState/u);
  assert.match(platformAutomationRepository, /sona_sqlite::automation::SqliteAutomationRepository/u);
  assert.match(platformAutomationRepository, /pub async fn load_repository_state/u);
  assert.match(platformAutomationRepository, /pub async fn persist_rules/u);
  assert.match(platformAutomationRepository, /pub async fn persist_processed_entries/u);
  assert.match(platformAutomationRepository, /pub async fn persist_repository_state/u);
  assert.match(platformAutomationRepository, /pub async fn validate_rule_activation/u);
  assert.match(platformAutomationRepository, /validate_rule_activation/u);
  assert.match(desktopAutomationCommand, /sona_core::automation::\{/u);
  assert.match(desktopAutomationCommand, /crate::platform::automation_repository::load_repository_state\(&app\)\.await/u);
  assert.match(desktopAutomationCommand, /crate::platform::automation_repository::persist_rules\(&app, rules\)\.await/u);
  assert.match(desktopAutomationCommand, /crate::platform::automation_repository::persist_processed_entries\(&app, processed_entries\)\.await/u);
  assert.match(
    desktopAutomationCommand,
    /crate::platform::automation_repository::persist_repository_state\(\s*&app,\s*rules,\s*processed_entries\s*,?\s*\)\s*\.await/u,
  );
  assert.match(desktopAutomationCommand, /crate::platform::automation_repository::validate_rule_activation\(/u);
  assert.doesNotMatch(desktopAutomationCommand, /sona_sqlite::automation::AutomationRepositoryState/u);
  assert.doesNotMatch(desktopAutomationCommand, /run_automation_task/u);
  assert.doesNotMatch(desktopAutomationCommand, /validate_rule_activation_inner/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'automation.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'automation'), false);
  assert.doesNotMatch(platformAutomationRepository, /is_feature_llm_config_complete/u);
  assert.doesNotMatch(platformAutomationRepository, /fn is_feature_llm_config_complete/u);
  assert.doesNotMatch(platformAutomationRepository, /fn is_batch_asr_configured/u);
  assert.doesNotMatch(platformAutomationRepository, /online_asr_providers/u);
  assert.equal(
    exists(...desktopCrateSegments, 'src', 'repositories', 'automation', 'sqlite_repository.rs'),
    false,
  );
});

test('automation runtime path rules are owned by core and adapted by desktop', () => {
  const coreAutomation = read('core', 'src', 'automation', 'mod.rs');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const runtimeFsLib = read('adapters', 'runtime_fs', 'src', 'lib.rs');
  const desktopPlatformRuntime = fs.readFileSync(
    desktopCratePath('src', 'platform', 'automation_runtime.rs'),
    'utf8',
  );
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const desktopCommands = read(...desktopCrateSegments, 'src', 'commands', 'automation.rs');

  assert.match(coreAutomation, /pub struct AutomationRuntimeRuleConfig/u);
  assert.match(coreAutomation, /pub struct AutomationRuntimeCandidatePayload/u);
  assert.match(coreAutomation, /pub enum AutomationRuntimePathCollectionOutcome/u);
  assert.match(coreAutomation, /pub struct AutomationRuntimePathMetadata/u);
  assert.match(coreAutomation, /pub fn should_consider_runtime_candidate_path/u);
  assert.match(coreAutomation, /pub fn collect_runtime_rule_path_result/u);
  assert.match(desktopPlatformRuntime, /sona_core::automation::\{/u);
  assert.match(desktopPlatformRuntime, /collect_runtime_rule_path_result/u);
  assert.match(desktopPlatformRuntime, /pub async fn collect_rule_path_results/u);
  assert.match(desktopPlatformRuntime, /should_consider_runtime_candidate_path/u);
  assert.match(runtimeFsLib, /pub fn automation_runtime_path_metadata/u);
  assert.match(runtimeFsLib, /pub fn collect_automation_runtime_candidate_paths/u);
  assert.match(desktopPlatformRuntime, /sona_runtime_fs::automation_runtime_path_metadata/u);
  assert.match(desktopPlatformRuntime, /sona_runtime_fs::collect_automation_runtime_candidate_paths/u);
  assert.match(desktopLib, /^pub mod platform;/mu);
  assert.match(desktopLib, /crate::platform::automation_runtime::AutomationRuntimeState/u);
  assert.match(desktopCommands, /crate::platform::automation_runtime::\{/u);
  assert.match(desktopCommands, /collect_rule_path_results\(rule, file_paths\)\.await/u);
  assert.doesNotMatch(desktopCommands, /collect_rule_path_result\(/u);
  assert.doesNotMatch(desktopCommands, /tauri::async_runtime::spawn_blocking/u);
  assert.doesNotMatch(tauriCargo, /^walkdir\s*=/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'automation.rs'), false);
  assert.doesNotMatch(desktopPlatformRuntime, /const SUPPORTED_MEDIA_EXTENSIONS/u);
  assert.doesNotMatch(desktopPlatformRuntime, /pub struct AutomationRuntimeRuleConfig/u);
  assert.doesNotMatch(desktopPlatformRuntime, /pub struct AutomationRuntimeCandidatePayload/u);
  assert.doesNotMatch(desktopPlatformRuntime, /pub enum AutomationRuntimePathCollectionOutcome/u);
  assert.doesNotMatch(desktopPlatformRuntime, /fn is_supported_media_path/u);
  assert.doesNotMatch(desktopPlatformRuntime, /fn is_path_within_watch_scope/u);
  assert.doesNotMatch(desktopPlatformRuntime, /walkdir::|WalkDir::|std::fs::metadata/u);
});

test('SQLite project repository is owned by sqlite adapter', () => {
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformDatabasePath = desktopCratePath('src', 'platform', 'database.rs');
  const platformDatabase = fs.existsSync(platformDatabasePath)
    ? fs.readFileSync(platformDatabasePath, 'utf8')
    : '';
  const platformProjectRepository = fs.readFileSync(
    desktopCratePath('src', 'platform', 'project_repository.rs'),
    'utf8',
  );
  const desktopProjectCommand = read(...desktopCrateSegments, 'src', 'commands', 'project.rs');

  assert.ok(exists('adapters', 'sqlite', 'src', 'project.rs'));
  assert.match(sqliteLib, /^pub mod project;/mu);
  assert.match(sqliteLib, /^pub use project::SqliteProjectRepository;/mu);
  assert.ok(fs.existsSync(platformDatabasePath));
  assert.match(platformMod, /^pub mod database;/mu);
  assert.match(platformDatabase, /pub fn sqlite_database/u);
  assert.match(platformDatabase, /pub fn sqlite_config_store/u);
  assert.match(platformDatabase, /app\.state::<Arc<sona_sqlite::Database>>\(\)/u);
  assert.match(platformProjectRepository, /sona_sqlite::project::SqliteProjectRepository/u);
  assert.match(platformProjectRepository, /crate::platform::database::sqlite_database/u);
  assert.match(platformProjectRepository, /pub async fn get_active_project_id/u);
  assert.match(platformProjectRepository, /pub async fn set_active_project_id/u);
  assert.match(desktopProjectCommand, /sona_core::project::\{/u);
  assert.match(desktopProjectCommand, /get_active_project_id/u);
  assert.match(desktopProjectCommand, /set_active_project_id/u);
  assert.doesNotMatch(desktopProjectCommand, /tauri_plugin_store::StoreExt/u);
  assert.doesNotMatch(desktopProjectCommand, /SqliteConfigStore/u);
  assert.doesNotMatch(desktopProjectCommand, /SETTINGS_FILE_NAME/u);
  assert.doesNotMatch(desktopProjectCommand, /ACTIVE_PROJECT_SETTINGS_KEY/u);
  assert.doesNotMatch(desktopProjectCommand, /app\.state::<Arc<sona_sqlite::Database>>\(\)/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'project.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'project'), false);
  assert.equal(
    exists(...desktopCrateSegments, 'src', 'repositories', 'project', 'sqlite_repository.rs'),
    false,
  );
});

test('desktop sqlite app-state access is centralized in platform database adapter', () => {
  const platformDatabase = fs.readFileSync(
    desktopCratePath('src', 'platform', 'database.rs'),
    'utf8',
  );
  const allowedPath = path.join('platforms', 'desktop', 'src', 'platform', 'database.rs').replaceAll(path.sep, '/');
  const directStateAccess = rustFilesUnder(desktopCratePath('src'))
    .sort()
    .flatMap((filePath) => {
      const relativePath = path.relative(repoRoot, filePath).replaceAll(path.sep, '/');
      if (relativePath === allowedPath) {
        return [];
      }
      const content = fs.readFileSync(filePath, 'utf8');
      return [
        ...content.matchAll(/\.state::<(?:std::sync::)?Arc<sona_sqlite::Database>>\s*\(\s*\)/gu),
        ...content.matchAll(/\.state::<(?:std::sync::)?Arc<Database>>\s*\(\s*\)/gu),
      ].map((match) => `${relativePath}: direct sqlite app-state access (${match[0]})`);
    });

  assert.match(platformDatabase, /app\.state::<Arc<sona_sqlite::Database>>\(\)/u);
  assert.deepEqual(directStateAccess, []);
});

test('LLM usage domain and SQLite usage store are owned by core and sqlite adapter', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformLlmUsagePath = desktopCratePath('src', 'platform', 'llm_usage.rs');
  const desktopLlmCommands = read(...desktopCrateSegments, 'src', 'commands', 'llm.rs');
  const tauriLlm = read(...desktopCrateSegments, 'src', 'integrations', 'llm.rs');
  const tauriLlmCommandImpl = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'commands.rs');
  const tauriIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const tauriLlmTypes = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'types.rs');

  assert.ok(exists('core', 'src', 'llm', 'usage.rs'));
  assert.ok(exists('adapters', 'sqlite', 'src', 'llm_usage.rs'));
  assert.ok(exists('adapters', 'sqlite', 'src', 'analytics.rs'));
  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod usage;/mu);
  assert.match(sqliteLib, /^pub mod llm_usage;/mu);
  assert.match(sqliteLib, /^pub mod analytics;/mu);
  assert.equal(fs.existsSync(platformLlmUsagePath), true);
  const platformLlmUsage = fs.readFileSync(platformLlmUsagePath, 'utf8');
  assert.match(platformMod, /^pub mod llm_usage;/mu);
  assert.match(platformLlmUsage, /pub fn record_usage/u);
  assert.match(platformLlmUsage, /pub fn read_raw/u);
  assert.match(platformLlmUsage, /pub fn replace_raw/u);
  assert.match(platformLlmUsage, /sona_sqlite::llm_usage::record_usage/u);
  assert.match(platformLlmUsage, /sona_sqlite::llm_usage::read_raw/u);
  assert.match(platformLlmUsage, /sona_sqlite::llm_usage::replace_raw/u);
  assert.match(tauriLlmCommandImpl, /crate::platform::llm_usage::record_usage\(\s*&self\.app,/u);
  assert.doesNotMatch(tauriLlmCommandImpl, /llm_usage_sqlite::record_usage/u);
  assert.match(desktopLlmCommands, /crate::platform::llm_usage::read_raw\(&app\)/u);
  assert.match(desktopLlmCommands, /crate::platform::llm_usage::replace_raw\(&app, content\)/u);
  assert.doesNotMatch(desktopLlmCommands, /llm_usage_sqlite::read_raw/u);
  assert.doesNotMatch(desktopLlmCommands, /llm_usage_sqlite::replace_raw/u);
  assert.match(tauriLlm, /pub\(crate\) use sona_core::llm::usage::UsageRecord;/u);
  assert.match(tauriLlmCommandImpl, /&UsageRecord \{/u);
  assert.doesNotMatch(tauriLlmCommandImpl, /crate::integrations::llm::usage::UsageRecord/u);
  assert.doesNotMatch(tauriLlm, /pub\(crate\) use sona_core::llm::usage;/u);
  assert.doesNotMatch(tauriIntegrations, /pub use sona_sqlite::llm_usage as llm_usage_sqlite;/u);
  assert.match(tauriLlmTypes, /pub use sona_core::llm::usage::\{LlmGenerateSource, LlmUsageCategory, TokenUsage\};/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'analytics.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'llm_usage.rs'), false);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'llm_usage_sqlite.rs'), false);
});

test('LLM task models and prompt planning are owned by core and reused by desktop', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const coreLlmTasks = read('core', 'src', 'llm', 'tasks.rs');
  const desktopLlm = read(...desktopCrateSegments, 'src', 'integrations', 'llm.rs');
  const desktopLlmTypes = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'types.rs');
  const desktopLlmTasks = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'tasks.rs');
  const onlineLlmLib = read('adapters', 'online_llm', 'src', 'lib.rs');
  const tsBindLib = read('adapters', 'ts_bind', 'src', 'lib.rs');

  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod tasks;/mu);
  assert.match(coreLlmTasks, /pub enum LlmTaskType/u);
  assert.match(coreLlmTasks, /pub struct LlmSegmentInput/u);
  assert.match(coreLlmTasks, /pub fn plan_segment_task_chunks/u);
  assert.match(coreLlmTasks, /pub struct SegmentTaskContext/u);
  assert.match(coreLlmTasks, /pub async fn run_segment_task/u);
  assert.match(coreLlmTasks, /pub async fn run_streaming_segment_task/u);
  assert.match(coreLlmTasks, /pub fn build_polish_prompt/u);
  assert.match(coreLlmTasks, /pub fn build_summary_chunk_prompt/u);
  assert.match(onlineLlmLib, /pub async fn run_google_translate_free_requests_in_order/u);
  assert.match(desktopLlm, /pub\(crate\) use sona_core::llm::tasks::\{/u);
  assert.match(desktopLlmTypes, /pub use sona_core::llm::tasks::\{[\s\S]*LlmTaskType[\s\S]*SummarySegmentInput/u);
  assert.match(desktopLlmTasks, /sona_core::llm::tasks::\{/u);
  assert.match(desktopLlmTasks, /run_segment_task/u);
  assert.match(desktopLlmTasks, /run_streaming_segment_task/u);
  assert.match(desktopLlmTasks, /sona_online_llm::\{/u);
  assert.match(tsBindLib, /sona_core::llm::tasks::\{/u);
  assert.match(tsBindLib, /LlmTaskType/u);
  assert.match(tsBindLib, /TranscriptSummaryResult/u);
  assert.doesNotMatch(desktopLlmTypes, /pub enum LlmTaskType/u);
  assert.doesNotMatch(desktopLlmTypes, /pub struct LlmSegmentInput/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) struct SegmentTaskContext/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) async fn run_segment_task/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) async fn run_streaming_segment_task/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) async fn run_google_translate_free_requests_in_order/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) fn plan_segment_task_chunks/u);
  assert.doesNotMatch(desktopLlmTasks, /pub\(crate\) fn build_polish_prompt/u);
});

test('transcript LLM job helpers are owned by core and reused by desktop', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const coreLlmJobs = read('core', 'src', 'llm', 'jobs.rs');
  const desktopLlmJobs = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'jobs.rs');

  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod jobs;/mu);
  assert.match(coreLlmJobs, /pub fn normalized_job_history_id/u);
  assert.match(coreLlmJobs, /pub fn segment_inputs_from_transcript/u);
  assert.match(coreLlmJobs, /pub fn merge_translated_items_into_segments/u);
  assert.match(coreLlmJobs, /pub fn compute_summary_source_fingerprint/u);
  assert.match(desktopLlmJobs, /sona_core::llm::jobs::\{/u);
  assert.doesNotMatch(desktopLlmJobs, /fn normalized_job_history_id/u);
  assert.doesNotMatch(desktopLlmJobs, /fn segment_inputs_from_transcript/u);
  assert.doesNotMatch(desktopLlmJobs, /pub\(crate\) fn merge_translated_items_into_segments/u);
  assert.doesNotMatch(desktopLlmJobs, /pub\(crate\) fn compute_summary_source_fingerprint/u);
});

test('desktop LLM commands use the integration facade instead of implementation submodules', () => {
  const desktopLlm = read(...desktopCrateSegments, 'src', 'integrations', 'llm.rs');
  const desktopLlmCommands = read(...desktopCrateSegments, 'src', 'commands', 'llm.rs');

  assert.match(desktopLlm, /^mod commands;/mu);
  assert.match(desktopLlm, /^mod jobs;/mu);
  assert.match(
    desktopLlm,
    /pub\(crate\) use commands::\{[\s\S]*generate_llm_text_command[\s\S]*list_llm_models_command[\s\S]*polish_transcript_segments_command[\s\S]*summarize_transcript_command[\s\S]*translate_transcript_segments_command[\s\S]*\};/u,
  );
  assert.match(desktopLlm, /pub\(crate\) use jobs::run_transcript_llm_job_command;/u);
  assert.doesNotMatch(desktopLlm, /^pub\(crate\) mod commands;/mu);
  assert.doesNotMatch(desktopLlm, /^pub\(crate\) mod jobs;/mu);
  assert.match(desktopLlmCommands, /crate::integrations::llm::generate_llm_text_command\(/u);
  assert.match(desktopLlmCommands, /crate::integrations::llm::run_transcript_llm_job_command\(/u);
  assert.match(desktopLlmCommands, /crate::integrations::llm::list_llm_models_command\(/u);
  assert.doesNotMatch(desktopLlmCommands, /integrations::llm::(?:commands|jobs)::/u);
});

test('LLM provider protocol mapping is owned by core and reused by desktop', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const coreProviderProtocol = read('core', 'src', 'llm', 'provider_protocol.rs');
  const desktopLlmTypes = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'types.rs');
  const desktopLlmProviders = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'providers.rs');
  const onlineLlmLib = read('adapters', 'online_llm', 'src', 'lib.rs');

  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod provider_protocol;/mu);
  assert.match(coreProviderProtocol, /pub struct LlmModelSummary/u);
  assert.match(coreProviderProtocol, /pub struct StandardLlmRequest/u);
  assert.match(coreProviderProtocol, /pub fn format_openai_models_urls/u);
  assert.match(coreProviderProtocol, /pub fn extract_text_from_json_response/u);
  assert.match(desktopLlmTypes, /pub use sona_core::llm::provider_protocol::\{/u);
  assert.match(onlineLlmLib, /sona_core::llm::provider_protocol::\{/u);
  assert.match(desktopLlmProviders, /sona_online_llm::\{/u);
  assert.doesNotMatch(desktopLlmProviders, /fn strategy_uses_openai_chat_payload/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn clean_gemini_base_url/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn format_openai_models_urls/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn extract_text_from_json_response/u);
  assert.doesNotMatch(desktopLlmProviders, /pub\(crate\) fn build_standard_input/u);
});

test('LLM streaming protocol helpers are owned by core and reused by desktop', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const coreStreaming = read('core', 'src', 'llm', 'streaming_protocol.rs');
  const desktopLlm = read(...desktopCrateSegments, 'src', 'integrations', 'llm.rs');
  const desktopStreaming = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'streaming.rs');
  const onlineLlmLib = read('adapters', 'online_llm', 'src', 'lib.rs');

  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod streaming_protocol;/mu);
  assert.match(coreStreaming, /pub struct StreamTextAccumulator/u);
  assert.match(coreStreaming, /pub struct StreamingLineBuffer/u);
  assert.match(coreStreaming, /pub struct SseEventBuffer/u);
  assert.match(coreStreaming, /pub fn build_openai_chat_payload/u);
  assert.match(desktopLlm, /sona_core::llm::streaming_protocol::\{/u);
  assert.match(onlineLlmLib, /pub async fn try_stream_text_with_provider/u);
  assert.match(desktopStreaming, /try_stream_text_with_provider/u);
  assert.doesNotMatch(desktopStreaming, /pub\(crate\) struct StreamTextAccumulator/u);
  assert.doesNotMatch(desktopStreaming, /pub\(crate\) struct StreamingLineBuffer/u);
  assert.doesNotMatch(desktopStreaming, /struct SseEventBuffer/u);
  assert.doesNotMatch(desktopStreaming, /pub\(crate\) fn build_openai_chat_payload/u);
  assert.doesNotMatch(desktopStreaming, /fn build_openai_stream_url/u);
  assert.doesNotMatch(desktopStreaming, /reqwest::header::CONTENT_TYPE/u);
  assert.doesNotMatch(desktopStreaming, /rig_core::providers/u);
  assert.doesNotMatch(desktopStreaming, /StreamedAssistantContent/u);
});

test('LLM request and config contracts are owned by core and reused by desktop', () => {
  const coreLib = read('core', 'src', 'lib.rs');
  const coreLlm = read('core', 'src', 'llm', 'mod.rs');
  const coreRequestsPath = path.join(repoRoot, 'core', 'src', 'llm', 'requests.rs');
  const coreRequests = fs.readFileSync(coreRequestsPath, 'utf8');
  const desktopLlmTypes = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'types.rs');
  const tsBindLib = read('adapters', 'ts_bind', 'src', 'lib.rs');
  const requestTypes = [
    'LlmConfig',
    'LlmGenerateRequest',
    'LlmUsageEventPayload',
    'LlmModelsRequest',
    'PolishSegmentsRequest',
    'TranslateSegmentsRequest',
    'SummarizeTranscriptRequest',
    'TranscriptLlmJobRequest',
    'TranscriptSummaryRecordPayload',
    'HistorySummaryPayload',
  ];

  assert.match(coreLib, /^pub mod llm;/mu);
  assert.match(coreLlm, /^pub mod requests;/mu);
  for (const typeName of requestTypes) {
    assert.match(coreRequests, new RegExp(`pub struct ${typeName}\\b`, 'u'));
    assert.match(desktopLlmTypes, new RegExp(`\\b${typeName}\\b`, 'u'));
    assert.match(tsBindLib, new RegExp(`\\b${typeName}\\b`, 'u'));
  }

  assert.match(desktopLlmTypes, /pub use sona_core::llm::requests::\{/u);
  assert.match(tsBindLib, /sona_core::llm::requests::\{/u);
  assert.doesNotMatch(desktopLlmTypes, /struct RawLlmConfig/u);
  for (const typeName of requestTypes) {
    assert.doesNotMatch(desktopLlmTypes, new RegExp(`pub struct ${typeName}\\b`, 'u'));
  }
  assert.match(desktopLlmTypes, /pub struct TranscriptLlmJobResult/u);
});

test('LLM generation and model listing use core ports with desktop adapters', () => {
  const corePorts = read('core', 'src', 'ports', 'mod.rs');
  const coreLlmPortPath = path.join(repoRoot, 'core', 'src', 'ports', 'llm.rs');
  const coreLlmPort = fs.readFileSync(coreLlmPortPath, 'utf8');
  const desktopCommands = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'commands.rs');
  const desktopProviders = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'providers.rs');
  const onlineLlmLib = read('adapters', 'online_llm', 'src', 'lib.rs');

  assert.match(corePorts, /^pub mod llm;/mu);
  assert.match(coreLlmPort, /trait LlmTextGenerator/u);
  assert.match(coreLlmPort, /trait LlmModelLister/u);
  assert.match(coreLlmPort, /LlmGenerateRequest/u);
  assert.match(coreLlmPort, /LlmModelsRequest/u);
  assert.match(coreLlmPort, /StandardLlmResponse/u);
  assert.match(coreLlmPort, /LlmModelSummary/u);
  assert.match(onlineLlmLib, /impl LlmTextGenerator for OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /impl LlmModelLister for OnlineLlmAdapter/u);
  assert.match(desktopProviders, /OnlineLlmAdapter as DesktopLlmAdapter/u);
  assert.match(desktopCommands, /DesktopLlmAdapter/u);
  assert.match(desktopCommands, /\.generate_text\(/u);
  assert.match(desktopCommands, /\.list_models\(/u);
  assert.doesNotMatch(desktopCommands, /generate_with_rig/u);
});

test('desktop LLM timestamps are supplied by platform time adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformTimePath = desktopCratePath('src', 'platform', 'time.rs');
  const desktopLlmCommands = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'llm', 'commands.rs'),
    'utf8',
  );
  const desktopLlmJobs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'llm', 'jobs.rs'),
    'utf8',
  );
  const desktopLlmIntegration = `${desktopLlmCommands}\n${desktopLlmJobs}`;

  assert.equal(fs.existsSync(platformTimePath), true);
  const platformTime = fs.readFileSync(platformTimePath, 'utf8');

  assert.match(platformMod, /^pub mod time;/mu);
  assert.match(platformTime, /pub fn utc_now_rfc3339\(/u);
  assert.match(platformTime, /pub fn utc_now_rfc3339_millis\(/u);
  assert.match(desktopLlmCommands, /crate::platform::time::utc_now_rfc3339\(\)/u);
  assert.match(desktopLlmJobs, /crate::platform::time::utc_now_rfc3339_millis\(\)/u);
  assert.doesNotMatch(desktopLlmIntegration, /chrono::Utc::now\(\)/u);
});

test('desktop app log timestamps are supplied by platform time adapter', () => {
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');
  const platformTime = read(...desktopCrateSegments, 'src', 'platform', 'time.rs');

  assert.match(platformTime, /pub fn unix_timestamp_secs\(/u);
  assert.match(desktopLib, /crate::platform::time::unix_timestamp_secs\(\)/u);
  assert.doesNotMatch(desktopLib, /SystemTime::now|UNIX_EPOCH/u);
});

test('online LLM provider implementation lives in adapter crate', () => {
  const workspaceCargo = read('Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const onlineLlmCargoPath = path.join(repoRoot, 'adapters', 'online_llm', 'Cargo.toml');
  const onlineLlmLibPath = path.join(repoRoot, 'adapters', 'online_llm', 'src', 'lib.rs');
  const onlineLlmLib = fs.readFileSync(onlineLlmLibPath, 'utf8');
  const desktopCommands = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'commands.rs');
  const desktopProviders = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'providers.rs');
  const desktopNetwork = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'network.rs');
  const desktopTasks = read(...desktopCrateSegments, 'src', 'integrations', 'llm', 'tasks.rs');

  assert.match(workspaceCargo, /"adapters\/online_llm"/u);
  assert.ok(fs.existsSync(onlineLlmCargoPath));
  assert.match(tauriCargo, /sona-online-llm\s*=\s*\{\s*path\s*=\s*"..\/..\/adapters\/online_llm"/u);
  assert.match(onlineLlmLib, /pub struct OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /impl LlmTextGenerator for OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /impl LlmModelLister for OnlineLlmAdapter/u);
  assert.match(onlineLlmLib, /pub struct LlmApiUrl/u);
  assert.match(desktopProviders, /sona_online_llm::\{/u);
  assert.match(desktopNetwork, /sona_online_llm(?:::\{[\s\S]*LlmApiUrl|::LlmApiUrl)/u);
  assert.match(desktopTasks, /sona_online_llm::\{/u);
  assert.match(onlineLlmLib, /pub async fn execute_google_translate_request/u);
  assert.match(desktopCommands, /execute_google_translate_request\(/u);
  assert.match(onlineLlmLib, /pub async fn execute_google_translate_free_request/u);
  assert.match(onlineLlmLib, /pub async fn fetch_google_translate_free_translation/u);
  assert.doesNotMatch(desktopCommands, /\.post\(url\.reqwest_url\(\)\)/u);
  assert.doesNotMatch(desktopProviders, /pub(?:\(crate\))? trait LlmAdapter/u);
  assert.doesNotMatch(desktopProviders, /struct AdapterFactory/u);
  assert.doesNotMatch(desktopProviders, /pub\(crate\) async fn get_openai_models/u);
  assert.doesNotMatch(desktopProviders, /pub\(crate\) async fn get_gemini_models/u);
  assert.doesNotMatch(desktopTasks, /enum GoogleTranslateFreeAttemptError/u);
  assert.doesNotMatch(desktopTasks, /fn parse_google_translate_free_retry_after/u);
  assert.doesNotMatch(desktopTasks, /fn extract_google_translate_free_translation/u);
  assert.doesNotMatch(desktopTasks, /fn google_translate_free_retry_delay/u);
});

test('storage usage SQLite and filesystem scanner is owned by sqlite adapter', () => {
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const desktopStorageCommand = read(...desktopCrateSegments, 'src', 'commands', 'storage.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformStorageUsagePath = desktopCratePath('src', 'platform', 'storage_usage.rs');

  assert.ok(exists('adapters', 'sqlite', 'src', 'storage_usage.rs'));
  assert.equal(fs.existsSync(platformStorageUsagePath), true);
  const platformStorageUsage = fs.readFileSync(platformStorageUsagePath, 'utf8');
  assert.match(sqliteLib, /^pub mod storage_usage;/mu);
  assert.match(platformMod, /^pub mod storage_usage;/mu);
  assert.match(platformStorageUsage, /pub use sona_sqlite::storage_usage::\{[\s\S]*StorageUsageSnapshot/u);
  assert.match(platformStorageUsage, /pub use sona_sqlite::storage_usage::\{[\s\S]*WebviewBrowsingDataClearResult/u);
  assert.match(platformStorageUsage, /collect_storage_usage_snapshot/u);
  assert.match(platformStorageUsage, /observable_webview_cache_bytes/u);
  assert.match(platformStorageUsage, /build_webview_clear_result/u);
  assert.match(platformStorageUsage, /tauri::async_runtime::spawn_blocking/u);
  assert.match(platformStorageUsage, /get_webview_window\("main"\)/u);
  assert.match(platformStorageUsage, /clear_all_browsing_data/u);
  assert.match(desktopStorageCommand, /crate::platform::storage_usage::get_usage_snapshot\(&app\)\.await/u);
  assert.match(desktopStorageCommand, /crate::platform::storage_usage::clear_webview_browsing_data\(&app\)\.await/u);
  assert.doesNotMatch(desktopStorageCommand, /sona_sqlite::storage_usage/u);
  assert.doesNotMatch(desktopStorageCommand, /PathKind::AppLocalData/u);
  assert.doesNotMatch(desktopStorageCommand, /tauri::async_runtime::spawn_blocking/u);
  assert.doesNotMatch(desktopStorageCommand, /observable_webview_cache_bytes/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'storage_usage.rs'), false);
});

test('history filesystem helpers are owned by sqlite adapter', () => {
  const coreHistory = read('core', 'src', 'history', 'mod.rs');
  const coreCargo = read('core', 'Cargo.toml');
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const sqliteHistoryFs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'sqlite', 'src', 'history_fs_utils.rs'),
    'utf8',
  );
  const platformHistory = fs.readFileSync(
    desktopCratePath('src', 'platform', 'history_repository.rs'),
    'utf8',
  );

  assert.equal(exists('core', 'src', 'history', 'fs_utils.rs'), false);
  assert.doesNotMatch(coreHistory, /^pub mod fs_utils;/mu);
  assert.doesNotMatch(coreCargo, /^bzip2\s*=/mu);
  assert.doesNotMatch(coreCargo, /^tar\s*=/mu);
  assert.match(sqliteLib, /^pub mod history_fs_utils;/mu);
  assert.match(platformHistory, /pub\(crate\) use sona_sqlite::history_fs_utils as fs_utils;/u);
  assert.match(sqliteHistoryFs, /pub fn create_tar_bz2_archive/u);
  assert.match(sqliteHistoryFs, /pub fn extract_tar_bz2_archive/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'history', 'fs_utils.rs'), false);
  assert.doesNotMatch(platformHistory, /^pub\(crate\) mod fs_utils;/mu);
});

test('SQLite history store is owned by sqlite adapter', () => {
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const platformHistory = fs.readFileSync(
    desktopCratePath('src', 'platform', 'history_repository.rs'),
    'utf8',
  );

  assert.ok(exists('adapters', 'sqlite', 'src', 'history_store.rs'));
  assert.match(sqliteLib, /^pub mod history_store;/mu);
  assert.match(sqliteLib, /^pub use history_store::SqliteHistoryStore;/mu);
  assert.match(platformHistory, /pub use sona_sqlite::history_store as sqlite_store;/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'history', 'sqlite_store.rs'), false);
  assert.doesNotMatch(platformHistory, /^pub mod sqlite_store;/mu);
});

test('history backup archive persistence is owned by sqlite adapter', () => {
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const platformHistory = fs.readFileSync(
    desktopCratePath('src', 'platform', 'history_repository.rs'),
    'utf8',
  );

  assert.ok(exists('adapters', 'sqlite', 'src', 'history_backup.rs'));
  assert.ok(exists('adapters', 'sqlite', 'src', 'history_archive.rs'));
  assert.match(sqliteLib, /^pub mod history_backup;/mu);
  assert.match(sqliteLib, /^pub mod history_archive;/mu);
  assert.match(platformHistory, /pub use sona_sqlite::history_backup as backup;/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'repositories', 'history', 'backup.rs'), false);
  assert.equal(
    exists(...desktopCrateSegments, 'src', 'repositories', 'history', 'repository.rs'),
    false,
  );
  assert.doesNotMatch(platformHistory, /^pub mod backup;/mu);
  assert.doesNotMatch(platformHistory, /^pub\(crate\) mod repository;/mu);
});

test('standalone CLI invokes local batch ASR through the core transcriber port', () => {
  const cliAsrAdapterRs = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'cli', 'src', 'asr_adapter.rs'),
    'utf8',
  );
  const cliTranscribeRs = fs.readFileSync(
    path.join(repoRoot, 'platforms', 'cli', 'src', 'transcribe.rs'),
    'utf8',
  );

  assert.match(cliAsrAdapterRs, /use sona_core::ports::asr::BatchTranscriber;/u);
  assert.match(cliAsrAdapterRs, /sona_local_asr::batch::LocalBatchAsrAdapter/u);
  assert.match(cliTranscribeRs, /use sona_core::ports::asr::BatchTranscriber;/u);
  assert.match(cliTranscribeRs, /crate::asr_adapter::local_batch_transcriber\(\)/u);
  assert.match(cliTranscribeRs, /\.transcribe\(plan\)/u);
  assert.doesNotMatch(cliTranscribeRs, /run_offline_transcription/u);
});

test('recognizer transcript utilities are owned by core and reused by adapters', () => {
  const coreTranscript = read('core', 'src', 'transcription', 'transcript.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');
  const tauriTranscript = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const localBatch = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'batch.rs'),
    'utf8',
  );

  assert.match(coreTranscript, /pub fn normalize_recognizer_text\(/u);
  assert.match(coreTranscript, /pub fn synthesize_durations\(/u);
  assert.match(asrMod, /pub use sona_core::transcription::postprocess::TranscriptPostprocessor/u);
  assert.doesNotMatch(asrMod, /^mod postprocess;/mu);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'asr', 'postprocess.rs'), false);
  assert.match(
    tauriTranscript,
    /pub\(crate\) use sona_core::transcription::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.match(
    localBatch,
    /use sona_core::transcription::transcript::\{[\s\S]*normalize_recognizer_text[\s\S]*synthesize_durations[\s\S]*\};/u,
  );
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn normalize_recognizer_text/u);
  assert.doesNotMatch(tauriTranscript, /pub\(crate\)\s+fn synthesize_durations/u);
  assert.doesNotMatch(localBatch, /^fn normalize_recognizer_text/mu);
  assert.doesNotMatch(localBatch, /^fn synthesize_durations/mu);
});

test('timeline transcript normalization is owned by core and reused by desktop', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const coreTranscript = read('core', 'src', 'transcription', 'transcript.rs');
  const asrMod = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mod.rs');
  const tauriTranscript = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const groqRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'groq.rs');
  const mistralRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mistral.rs');

  assert.match(coreTranscript, /pub fn apply_timeline_normalization_with_id_generator/u);
  assert.match(coreTranscript, /pub fn build_transcript_update_with_id_generator/u);
  assert.doesNotMatch(coreTranscript, /Uuid::new_v4|uuid::Uuid::new_v4/u);
  assert.doesNotMatch(coreCargo, /^uuid\s*=/mu);
  assert.match(asrMod, /pub\(crate\) use transcript::\{[\s\S]*apply_timeline_normalization[\s\S]*\};/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn apply_timeline_normalization\(/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn build_transcript_update\(/u);
  assert.match(tauriTranscript, /apply_timeline_normalization_with_id_generator/u);
  assert.match(tauriTranscript, /build_transcript_update_with_id_generator/u);
  assert.match(tauriTranscript, /uuid::Uuid::new_v4\(\)\.to_string\(\)/u);
  assert.doesNotMatch(tauriTranscript, /struct TokenMap/u);
  assert.doesNotMatch(tauriTranscript, /struct SplitterState/u);
  assert.doesNotMatch(tauriTranscript, /fn split_segment_by_parts/u);
  assert.match(tauriTranscript, /pub\(crate\)\s+fn emit_transcript_update/u);
  assert.match(groqRs, /use super::apply_timeline_normalization;/u);
  assert.match(mistralRs, /use super::apply_timeline_normalization;/u);
  assert.doesNotMatch(groqRs, /crate::integrations::asr::transcript::apply_timeline_normalization/u);
  assert.doesNotMatch(mistralRs, /crate::integrations::asr::transcript::apply_timeline_normalization/u);
});

test('desktop Groq and Mistral batch providers delegate HTTP work to online ASR adapter', () => {
  const workspaceCargo = read('Cargo.toml');
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );
  const onlineAsrCargoPath = path.join(repoRoot, 'adapters', 'online_asr', 'Cargo.toml');
  const onlineAsrLibPath = path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs');
  const coreAsr = read('core', 'src', 'ports', 'asr.rs');
  const groqRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'groq.rs');
  const mistralRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'mistral.rs');

  assert.match(workspaceCargo, /"adapters\/online_asr"/u);
  assert.ok(fs.existsSync(onlineAsrCargoPath));
  assert.ok(fs.existsSync(onlineAsrLibPath));
  assert.match(tauriCargo, /sona-online-asr\s*=\s*\{\s*path\s*=\s*"..\/..\/adapters\/online_asr"/u);
  assert.match(prWorkflow, /sona-online-asr/u);
  assert.match(coreAsr, /trait OnlineBatchTranscriber/u);
  assert.match(coreAsr, /struct OnlineBatchTranscriptionRequest/u);
  assert.match(coreAsr, /struct OnlineBatchTranscriptionOutput/u);

  for (const providerRs of [groqRs, mistralRs]) {
    assert.match(providerRs, /sona_online_asr::/u);
    assert.doesNotMatch(providerRs, /reqwest::multipart/u);
    assert.doesNotMatch(providerRs, /reqwest::Client/u);
    assert.doesNotMatch(providerRs, /\.post\(/u);
  }
});

test('desktop Volcengine batch provider delegates HTTP work to online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /VolcengineDoubaoBatchTranscriber/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config/u);
  assert.match(onlineAsrLib, /build_volcengine_flash_batch_request_body/u);
  assert.match(onlineAsrLib, /segments_from_volcengine_response/u);
  assert.match(volcengineRs, /sona_online_asr::VolcengineDoubaoBatchTranscriber/u);
  assert.doesNotMatch(volcengineRs, /reqwest::Client/u);
  assert.doesNotMatch(volcengineRs, /\.post\(/u);
  assert.doesNotMatch(volcengineRs, /base64::engine::general_purpose::STANDARD\.encode/u);
});

test('desktop Volcengine streaming protocol helpers are owned by online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const onlineStreaming = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'volcengine', 'streaming.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /build_volcengine_full_client_request_frame/u);
  assert.match(onlineAsrLib, /build_volcengine_audio_frame/u);
  assert.match(onlineAsrLib, /parse_volcengine_server_response_frame/u);
  assert.match(onlineAsrLib, /volcengine_streaming_segments_from_response/u);
  assert.match(onlineAsrLib, /f32_samples_to_i16_pcm_bytes/u);
  assert.match(onlineStreaming, /crate::build_volcengine_full_client_request_frame/u);
  assert.match(onlineStreaming, /crate::parse_volcengine_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /build_volcengine_full_client_request_frame/u);
  assert.doesNotMatch(volcengineRs, /parse_volcengine_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /pub fn build_audio_frame/u);
  assert.doesNotMatch(volcengineRs, /fn parse_server_response_frame/u);
  assert.doesNotMatch(volcengineRs, /fn f32_samples_to_i16_pcm_bytes/u);
});

test('Volcengine streaming session is implemented by the online ASR adapter', () => {
  const onlineStreamingPath = path.join(
    repoRoot, 'adapters', 'online_asr', 'src', 'volcengine', 'streaming.rs',
  );
  const onlineCargo = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'Cargo.toml'),
    'utf8',
  );
  const desktopCargo = fs.readFileSync(
    desktopCratePath('Cargo.toml'),
    'utf8',
  );
  const desktopVolcengine = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );
  const desktopAsr = rustFilesUnder(
    desktopCratePath('src', 'integrations', 'asr'),
  )
    .map((filePath) => fs.readFileSync(filePath, 'utf8'))
    .join('\n');

  assert.equal(fs.existsSync(onlineStreamingPath), true);
  const onlineStreaming = fs.readFileSync(onlineStreamingPath, 'utf8');
  assert.match(onlineStreaming, /impl AsrStreamingSession for VolcengineStreamingSession/u);
  assert.match(onlineStreaming, /tokio_tungstenite::connect_async/u);
  assert.match(onlineStreaming, /tokio::spawn/u);
  assert.doesNotMatch(onlineStreaming, /tauri::|AsrState|crate::platform/u);
  assert.match(
    desktopVolcengine,
    /sona_online_asr::create_volcengine_streaming_session/u,
  );
  assert.doesNotMatch(
    desktopVolcengine,
    /VolcengineWriter|connect_async|tauri::async_runtime::spawn|parse_volcengine_server_response_frame/u,
  );
  assert.doesNotMatch(desktopAsr, /observe_streaming_transcript_update/u);
  assert.match(onlineCargo, /^tokio-tungstenite\s*=/mu);
  assert.doesNotMatch(desktopCargo, /^tokio-tungstenite\s*=/mu);
});

test('desktop Volcengine config and response helpers are owned by online ASR adapter', () => {
  const onlineAsrLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'online_asr', 'src', 'lib.rs'),
    'utf8',
  );
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.match(onlineAsrLib, /VolcengineConfigError/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config_checked/u);
  assert.match(onlineAsrLib, /resolve_volcengine_config_from_value_checked/u);
  assert.match(onlineAsrLib, /build_volcengine_flash_batch_request_body/u);
  assert.match(onlineAsrLib, /segments_from_volcengine_response/u);
  assert.match(onlineAsrLib, /map_volcengine_status_error/u);
  assert.match(volcengineRs, /sona_online_asr::resolve_volcengine_config_checked/u);
  assert.doesNotMatch(volcengineRs, /pub enum VolcengineMode/u);
  assert.doesNotMatch(volcengineRs, /pub struct VolcengineDoubaoConfigFields/u);
  assert.doesNotMatch(volcengineRs, /fn config_fields/u);
  assert.doesNotMatch(volcengineRs, /pub fn validate_config/u);
  assert.doesNotMatch(volcengineRs, /fn detect_audio_format/u);
  assert.doesNotMatch(volcengineRs, /fn build_flash_batch_request_body/u);
  assert.doesNotMatch(volcengineRs, /pub fn segments_from_response_value/u);
  assert.doesNotMatch(volcengineRs, /fn segment_from_utterance/u);
  assert.doesNotMatch(volcengineRs, /fn ms_value/u);
  assert.doesNotMatch(volcengineRs, /pub fn map_status_error/u);
});

test('desktop hardware GPU adapter lives in platform layer', () => {
  const appMod = read(...desktopCrateSegments, 'src', 'app', 'mod.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');
  const streamingRs = read(...desktopCrateSegments, 'src', 'integrations', 'streaming.rs');
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const hardwarePath = desktopCratePath('src', 'platform', 'hardware.rs');

  assert.equal(fs.existsSync(hardwarePath), true);
  const hardwareRs = fs.readFileSync(hardwarePath, 'utf8');

  assert.match(platformMod, /^pub mod hardware;/mu);
  assert.doesNotMatch(appMod, /^pub mod hardware;/mu);
  assert.match(hardwareRs, /pub\(crate\) use sona_local_asr::gpu::\{/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::check_gpu_availability\(\)\.await/u);
  assert.match(hardwareRs, /sona_local_asr::gpu::resolve_gpu_acceleration_plan/u);
  assert.match(systemCommand, /crate::platform::hardware::check_gpu_availability\(\)\.await/u);
  assert.match(batchRs, /crate::platform::hardware::resolve_gpu_acceleration_plan/u);
  assert.match(streamingRs, /crate::platform::hardware::resolve_gpu_acceleration_plan/u);
  assert.match(adapterSession, /use crate::gpu::resolve_gpu_acceleration_plan/u);
  assert.doesNotMatch(hardwareRs, /tokio::process::Command/u);
  assert.doesNotMatch(hardwareRs, /struct GpuAccelerationPlan/u);
  assert.doesNotMatch(hardwareRs, /struct GpuFallbackNotice/u);
  assert.doesNotMatch(systemCommand, /crate::app::hardware/u);
  assert.doesNotMatch(batchRs, /crate::app::hardware/u);
  assert.doesNotMatch(streamingRs, /crate::app::hardware/u);
  assert.doesNotMatch(adapterSession, /crate::app::hardware/u);
});

test('desktop local audio helpers come from local ASR adapter without Tauri core pipeline', () => {
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');
  const desktopAudio = read(...desktopCrateSegments, 'src', 'integrations', 'audio.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformAudioStoragePath = desktopCratePath('src', 'platform', 'audio_storage.rs');
  const platformAudioStorage = fs.existsSync(platformAudioStoragePath) ? fs.readFileSync(platformAudioStoragePath, 'utf8') : '';
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const localAsrSpeakerProcessing = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs'),
    'utf8',
  );
  const runtimeStatusRs = fs.readFileSync(
    desktopCratePath('src', 'platform', 'runtime_status.rs'),
    'utf8',
  );
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.equal(exists(...desktopCrateSegments, 'src', 'core', 'pipeline.rs'), false);
  assert.match(batchRs, /sona_local_asr::audio::extract_and_resample_audio/u);
  assert.match(batchRs, /sona_local_asr::audio::save_wav_file/u);
  assert.match(localAsrSpeakerProcessing, /crate::audio::extract_and_resample_audio/u);
  assert.match(localAsrSpeakerProcessing, /crate::audio::save_wav_file/u);
  assert.match(runtimeStatusRs, /sona_local_asr::audio::resolve_ffmpeg_sidecar_path/u);
  assert.equal(fs.existsSync(platformAudioStoragePath), true);
  assert.match(platformMod, /^pub mod audio_storage;/mu);
  assert.match(platformAudioStorage, /pub fn create_history_recording_path_for_app/u);
  assert.match(platformAudioStorage, /sona_runtime_fs::ensure_directory_exists\(&history_dir\)/u);
  assert.match(desktopAudio, /crate::platform::audio_storage::create_history_recording_path_for_app\(&app\)/u);
  assert.match(desktopAudio, /sona_local_asr::audio::LiveWavRecorder/u);
  assert.doesNotMatch(desktopAudio, /sona_runtime_fs::ensure_directory_exists/u);
  assert.doesNotMatch(desktopAudio, /TauriPathProvider|PathKind|PathProvider/u);
  assert.doesNotMatch(desktopAudio, /std::fs::create_dir_all/u);
  assert.doesNotMatch(desktopAudio, /\bhound::/u);
  assert.doesNotMatch(tauriCargo, /^hound\s*=/mu);
  assert.doesNotMatch(prWorkflow, /core::pipeline::tests/u);

  const desktopPipelineReferences = rustFilesUnder(desktopCratePath('src'))
    .map((filePath) => ({
      filePath,
      content: fs.readFileSync(filePath, 'utf8'),
    }))
    .filter(({ content }) => /crate::core::pipeline|core::pipeline/u.test(content))
    .map(({ filePath }) => path.relative(repoRoot, filePath));

  assert.deepEqual(desktopPipelineReferences, []);
});

test('local ASR blocking tasks are owned by the local adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformAsrRuntimePath = desktopCratePath('src', 'platform', 'asr_runtime.rs');
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );

  assert.equal(fs.existsSync(platformAsrRuntimePath), false);
  assert.doesNotMatch(platformMod, /^pub mod asr_runtime;/mu);
  assert.match(adapterSession, /tokio::task::spawn_blocking\(task\)/u);
  assert.match(adapterSession, /drop\(tokio::task::spawn_blocking\(task\)\)/u);
  assert.doesNotMatch(adapterSession, /tauri::async_runtime::spawn_blocking/u);
});

test('desktop system audio mute command is owned by platform adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const commandAudio = read(...desktopCrateSegments, 'src', 'commands', 'audio.rs');
  const desktopAudio = read(...desktopCrateSegments, 'src', 'integrations', 'audio.rs');
  const platformSystemAudioPath = desktopCratePath('src', 'platform', 'system_audio.rs');

  assert.equal(fs.existsSync(platformSystemAudioPath), true);
  const platformSystemAudio = fs.readFileSync(platformSystemAudioPath, 'utf8');

  assert.match(platformMod, /^pub mod system_audio;/mu);
  assert.match(commandAudio, /crate::platform::system_audio::set_system_audio_mute\(mute\)\.await/u);
  assert.match(platformSystemAudio, /pub async fn set_system_audio_mute/u);
  assert.match(platformSystemAudio, /set_mute_windows/u);
  assert.match(platformSystemAudio, /set_mute_macos/u);
  assert.match(platformSystemAudio, /set_mute_linux/u);
  assert.doesNotMatch(desktopAudio, /pub async fn set_system_audio_mute/u);
  assert.doesNotMatch(desktopAudio, /set_mute_windows|set_mute_macos|set_mute_linux/u);
  assert.doesNotMatch(desktopAudio, /std::process::Command|Command::new|IAudioEndpointVolume/u);
});

test('desktop system input adapter lives in platform layer', () => {
  const appMod = read(...desktopCrateSegments, 'src', 'app', 'mod.rs');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformSystemPath = desktopCratePath('src', 'platform', 'system.rs');

  assert.equal(fs.existsSync(platformSystemPath), true);
  const platformSystem = fs.readFileSync(platformSystemPath, 'utf8');

  assert.match(platformMod, /^pub mod system;/mu);
  assert.doesNotMatch(appMod, /^pub mod system;/mu);
  assert.match(platformSystem, /pub enum ShortcutModifier/u);
  assert.match(platformSystem, /pub fn inject_text/u);
  assert.match(platformSystem, /pub fn get_mouse_position/u);
  assert.match(platformSystem, /pub fn get_text_cursor_position/u);
  assert.match(platformSystem, /pub fn force_exit/u);
  assert.match(platformSystem, /Enigo::new/u);
  assert.match(platformSystem, /SendInput/u);
  assert.match(systemCommand, /crate::platform::system::greet\(name\)/u);
  assert.match(systemCommand, /crate::platform::system::force_exit\(app\)/u);
  assert.match(systemCommand, /Option<Vec<crate::platform::system::ShortcutModifier>>/u);
  assert.match(systemCommand, /crate::platform::system::inject_text\(text, shortcut_modifiers\)/u);
  assert.match(systemCommand, /crate::platform::system::get_mouse_position\(\)/u);
  assert.match(systemCommand, /crate::platform::system::get_text_cursor_position\(\)/u);
  assert.doesNotMatch(systemCommand, /crate::app::system/u);
});

test('desktop startup error dialog is owned by platform adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const desktopMain = read(...desktopCrateSegments, 'src', 'main.rs');
  const startupDialogPath = desktopCratePath('src', 'platform', 'startup_dialog.rs');

  assert.equal(fs.existsSync(startupDialogPath), true);
  const startupDialog = fs.readFileSync(startupDialogPath, 'utf8');

  assert.match(platformMod, /^pub mod startup_dialog;/mu);
  assert.match(desktopMain, /tauri_appsona_lib::platform::startup_dialog::show_error_dialog/u);
  assert.match(startupDialog, /pub fn show_error_dialog\(message: &str\)/u);
  assert.match(startupDialog, /fn escape_applescript_text\(message: &str\)/u);
  assert.match(startupDialog, /MessageBoxW/u);
  assert.match(startupDialog, /Command::new\("osascript"\)/u);
  assert.match(startupDialog, /Command::new\("zenity"\)/u);
  assert.match(startupDialog, /Command::new\("kdialog"\)/u);
  assert.doesNotMatch(desktopMain, /fn show_error_dialog|escape_applescript_text/u);
  assert.doesNotMatch(desktopMain, /std::process::Command|Command::new|MessageBoxW/u);
});

test('desktop startup console setup is owned by platform adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const desktopMain = read(...desktopCrateSegments, 'src', 'main.rs');
  const startupConsolePath = desktopCratePath('src', 'platform', 'startup_console.rs');

  assert.equal(fs.existsSync(startupConsolePath), true);
  const startupConsole = fs.readFileSync(startupConsolePath, 'utf8');

  assert.match(platformMod, /^pub mod startup_console;/mu);
  assert.match(desktopMain, /tauri_appsona_lib::platform::startup_console::fix_console\(false\)/u);
  assert.match(startupConsole, /pub fn fix_console\(show_new_console: bool\)/u);
  assert.match(startupConsole, /fn AllocConsole\(\) -> i32/u);
  assert.match(startupConsole, /fn AttachConsole\(dwProcessId: u32\) -> i32/u);
  assert.match(startupConsole, /OpenOptions::new\(\)\.write\(true\)\.open\("CONOUT\$"\)/u);
  assert.match(startupConsole, /OpenOptions::new\(\)\.read\(true\)\.open\("CONIN\$"\)/u);
  assert.doesNotMatch(desktopMain, /fn fix_console|AllocConsole|AttachConsole|SetStdHandle|OpenOptions/u);
});

test('desktop startup test-exit environment switch is owned by platform adapter', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const desktopMain = read(...desktopCrateSegments, 'src', 'main.rs');
  const startupEnvPath = desktopCratePath('src', 'platform', 'startup_env.rs');

  assert.equal(fs.existsSync(startupEnvPath), true);
  const startupEnv = fs.readFileSync(startupEnvPath, 'utf8');

  assert.match(platformMod, /^pub mod startup_env;/mu);
  assert.match(
    desktopMain,
    /tauri_appsona_lib::platform::startup_env::should_exit_before_app\(\)/u,
  );
  assert.match(startupEnv, /pub fn should_exit_before_app\(\) -> bool/u);
  assert.match(startupEnv, /std::env::var_os\("SONA_TEST_EXIT_BEFORE_APP"\)\.is_some\(\)/u);
  assert.doesNotMatch(desktopMain, /std::env::var_os|SONA_TEST_EXIT_BEFORE_APP/u);
});

test('desktop batch ASR delegates audio segmentation to local ASR adapter', () => {
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');

  assert.match(batchRs, /sona_local_asr::audio::segment_batch_audio/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::SileroVadModelConfig/u);
  assert.doesNotMatch(batchRs, /sherpa_onnx::VadModelConfig/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::vad_segment_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::fixed_chunk_audio/u);
  assert.doesNotMatch(batchRs, /crate::core::pipeline::whole_audio_segment/u);
});

test('speaker processing runtime is owned by local ASR adapter and wrapped by desktop platform', () => {
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const localAsrLib = read('adapters', 'local_asr', 'src', 'lib.rs');
  const localAsrProcessing = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'speaker_processing.rs'),
    'utf8',
  );
  const desktopPlatform = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformSpeaker = fs.readFileSync(
    desktopCratePath('src', 'platform', 'speaker_processing.rs'),
    'utf8',
  );
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const batchRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'batch.rs');

  assert.match(localAsrLib, /^pub mod speaker;/mu);
  assert.match(localAsrLib, /^pub mod speaker_processing;/mu);
  assert.equal(exists('adapters', 'local_asr', 'src', 'speaker.rs'), true);
  assert.equal(exists('adapters', 'local_asr', 'src', 'speaker_processing.rs'), true);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'speaker.rs'), false);
  assert.match(desktopPlatform, /^pub mod speaker_processing;/mu);
  assert.doesNotMatch(desktopIntegrations, /^pub mod speaker;/mu);
  assert.doesNotMatch(tauriCargo, /^sherpa-onnx\s*=/mu);
  assert.match(localAsrProcessing, /crate::speaker::run_speaker_diarization/u);
  assert.match(localAsrProcessing, /crate::speaker::SpeakerEmbeddingIndex/u);
  assert.match(localAsrProcessing, /crate::audio::extract_and_resample_audio/u);
  assert.doesNotMatch(localAsrProcessing, /use sherpa_onnx::/u);
  assert.doesNotMatch(localAsrProcessing, /OfflineSpeakerDiarization/u);
  assert.doesNotMatch(localAsrProcessing, /SpeakerEmbeddingExtractor/u);
  assert.doesNotMatch(localAsrProcessing, /SpeakerEmbeddingManager/u);
  assert.match(platformSpeaker, /sona_local_asr::speaker_processing::annotate_speaker_segments_from_file/u);
  assert.match(platformSpeaker, /sona_local_asr::speaker_processing::import_speaker_profile_sample/u);
  assert.match(platformSpeaker, /pub async fn import_speaker_profile_sample_for_app/u);
  assert.match(systemCommand, /crate::platform::speaker_processing::annotate_speaker_segments_from_file/u);
  assert.match(systemCommand, /crate::platform::speaker_processing::import_speaker_profile_sample_for_app\(\s*&app,\s*profile_id,\s*source_path,\s*source_name\s*,?\s*\)\s*\.await/u);
  assert.match(batchRs, /sona_local_asr::speaker_processing::annotate_segments_with_speakers/u);
  assert.doesNotMatch(batchRs, /crate::integrations::speaker/u);
});

test('media file IO detection is delegated to media detector adapter', () => {
  const coreCargo = read('core', 'Cargo.toml');
  const coreLib = read('core', 'src', 'lib.rs');
  const coreRuntime = read('core', 'src', 'runtime', 'mod.rs');
  const coreMediaDetector = read('core', 'src', 'runtime', 'media_detector.rs');
  const mediaDetectorCargo = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'media_detector', 'Cargo.toml'),
    'utf8',
  );
  const mediaDetectorLib = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'media_detector', 'src', 'lib.rs'),
    'utf8',
  );
  const tauriCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const apiCargo = read('adapters', 'api_server', 'Cargo.toml');
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const platformMediaDetectorPath = desktopCratePath('src', 'platform', 'media_detector.rs');
  const desktopIntegrations = read(...desktopCrateSegments, 'src', 'integrations', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const tauriServer = read(...desktopCrateSegments, 'src', 'app', 'server.rs');
  const apiServer = read('adapters', 'api_server', 'src', 'lib.rs');

  assert.match(coreCargo, /^infer\s*=/mu);
  assert.match(coreLib, /^pub mod runtime;/mu);
  assert.match(coreRuntime, /^pub mod media_detector;/mu);
  assert.match(coreMediaDetector, /pub fn is_valid_media_bytes/u);
  assert.doesNotMatch(coreMediaDetector, /tokio::fs::File/u);
  assert.doesNotMatch(coreMediaDetector, /pub async fn is_valid_media_file/u);
  assert.doesNotMatch(coreMediaDetector, /pub async fn check_media_formats/u);
  assert.match(mediaDetectorCargo, /^sona-core\s*=/mu);
  assert.match(mediaDetectorCargo, /^tokio\s*=/mu);
  assert.match(mediaDetectorLib, /pub async fn is_valid_media_file/u);
  assert.match(mediaDetectorLib, /pub async fn check_media_formats/u);
  assert.match(mediaDetectorLib, /sona_core::runtime::media_detector::is_valid_media_bytes/u);
  assert.equal(fs.existsSync(platformMediaDetectorPath), true);
  const platformMediaDetector = fs.readFileSync(platformMediaDetectorPath, 'utf8');
  assert.match(platformMod, /^pub mod media_detector;/mu);
  assert.match(platformMediaDetector, /pub async fn check_media_formats/u);
  assert.match(platformMediaDetector, /sona_media_detector::check_media_formats/u);
  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'media_detector.rs'), false);
  assert.doesNotMatch(desktopIntegrations, /^pub mod media_detector;/mu);
  assert.doesNotMatch(tauriCargo, /^infer\s*=/mu);
  assert.match(tauriCargo, /^sona-media-detector\s*=/mu);
  assert.doesNotMatch(cliCargo, /^sona-media-detector\s*=/mu);
  assert.match(apiCargo, /^sona-media-detector\s*=/mu);
  assert.match(systemCommand, /crate::platform::media_detector::check_media_formats\(paths\)\.await/u);
  assert.doesNotMatch(systemCommand, /sona_media_detector::check_media_formats/u);
  assert.match(apiServer, /sona_media_detector::is_valid_media_file/u);
  assert.doesNotMatch(systemCommand, /sona_core::runtime::media_detector::check_media_formats/u);
  assert.doesNotMatch(apiServer, /sona_core::runtime::media_detector::is_valid_media_file/u);
  assert.doesNotMatch(systemCommand, /crate::integrations::media_detector/u);
  assert.doesNotMatch(apiServer, /crate::integrations::media_detector/u);
  assert.doesNotMatch(tauriServer, /sona_media_detector::is_valid_media_file/u);
});

test('local streaming ASR VAD creation is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const localAsrRuntime = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'runtime.rs'),
    'utf8',
  );

  assert.equal(exists(...desktopCrateSegments, 'src', 'integrations', 'asr', 'model_config.rs'), false);
  assert.doesNotMatch(asrMod, /^mod model_config;/mu);
  assert.match(localAsrRuntime, /use crate::audio::\{[^}]*\bload_vad\b/u);
  assert.match(localAsrRuntime, /use crate::audio::\{[^}]*\bSafeVad\b/u);
  assert.match(localAsrRuntime, /pub fn configure_vad\(&mut self/u);
  assert.match(adapterSession, /session_instance\.configure_vad\(/u);
  assert.doesNotMatch(asrMod, /\bload_vad\b/u);
  assert.doesNotMatch(adapterSession, /use crate::audio::\{[\s\S]*SafeVad/u);
  assert.doesNotMatch(asrMod, /create_vad_detector/u);
  assert.doesNotMatch(asrMod, /pub struct SafeVad/u);
  assert.doesNotMatch(asrMod, /SileroVadModelConfig/u);
  assert.doesNotMatch(asrMod, /VadModelConfig/u);
  assert.doesNotMatch(asrMod, /VoiceActivityDetector/u);
});

test('desktop punctuation loading is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(asrMod, /pub use sona_local_asr::punctuation::\{Punctuation, load_punctuation\}/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuation/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuationConfig/u);
  assert.doesNotMatch(asrMod, /OfflinePunctuationModelConfig/u);
});

test('desktop recognizer construction is delegated to local ASR adapter', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );

  assert.match(asrMod, /pub use sona_local_asr::recognizer::/u);
  assert.doesNotMatch(asrMod, /^use sherpa_onnx::/mu);
  assert.doesNotMatch(asrMod, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(asrMod, /OnlineRecognizerConfig/u);
  assert.doesNotMatch(asrMod, /pub enum ModelType/u);
  assert.doesNotMatch(asrMod, /impl Recognizer/u);
});

test('desktop ASR modules use a local runtime facade instead of sherpa implementation paths', () => {
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const adapterRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'adapter.rs'),
    'utf8',
  );
  const stateRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'state.rs');
  const transcriptRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'transcript.rs'),
    'utf8',
  );
  const traitsRs = read(...desktopCrateSegments, 'src', 'integrations', 'asr', 'traits.rs');
  const volcengineRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'volcengine.rs'),
    'utf8',
  );

  assert.doesNotMatch(asrMod, /^mod sherpa_onnx;$/mu);
  assert.match(asrMod, /^mod state;$/mu);
  assert.match(asrMod, /^mod traits;$/mu);
  assert.doesNotMatch(asrMod, /^pub mod state;$/mu);
  assert.doesNotMatch(asrMod, /^pub mod traits;$/mu);
  assert.match(adapterRs, /sona_local_asr::streaming::create_streaming_session/u);
  assert.doesNotMatch(adapterRs, /super::state::AsrState/u);
  assert.doesNotMatch(adapterRs, /super::traits::/u);
  assert.doesNotMatch(stateRs, /resolve_punctuation/u);
  assert.doesNotMatch(stateRs, /super::traits::/u);
  assert.match(transcriptRs, /pub\(crate\) fn diagnostics_instance_label/u);
  assert.match(transcriptRs, /pub\(crate\) fn log_segment_emit_diagnostics/u);
  assert.match(traitsRs, /use super::\{[\s\S]*AsrTranscriptionRequest[\s\S]*\};/u);
  assert.doesNotMatch(stateRs, /crate::integrations::asr::state::/u);
  assert.doesNotMatch(transcriptRs, /crate::integrations::asr::traits::/u);
  assert.doesNotMatch(traitsRs, /crate::integrations::asr::types::/u);
  assert.doesNotMatch(volcengineRs, /crate::integrations::asr::types::/u);
  assert.doesNotMatch(volcengineRs, /super::super::types::/u);
  assert.doesNotMatch(adapterRs, /sherpa_onnx::/u);
  assert.doesNotMatch(stateRs, /sherpa_onnx::/u);
  assert.doesNotMatch(transcriptRs, /sherpa_onnx::/u);
});

test('local batch transcription reuses local ASR recognizer model construction', () => {
  const batchRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'batch.rs'),
    'utf8',
  );
  const recognizerRs = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'recognizer.rs'),
    'utf8',
  );

  assert.match(batchRs, /pub struct LocalBatchAsrAdapter/u);
  assert.match(batchRs, /impl BatchTranscriber for LocalBatchAsrAdapter/u);
  assert.match(batchRs, /use crate::recognizer::\{[^}]*build_offline_model_config/u);
  assert.match(batchRs, /decode_offline_samples/u);
  assert.match(batchRs, /create_offline_recognizer/u);
  assert.match(recognizerRs, /pub fn create_offline_recognizer\([\s\S]*\) -> Result<SafeOfflineRecognizer, String>/u);
  assert.doesNotMatch(batchRs, /pub async fn run_offline_transcription/u);
  assert.doesNotMatch(batchRs, /enum ModelType/u);
  assert.doesNotMatch(batchRs, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(recognizerRs, /\) -> Result<OfflineRecognizer, String>/u);
  assert.doesNotMatch(batchRs, /OfflineRecognizerConfig/u);
  assert.doesNotMatch(batchRs, /fn build_model_config/u);
  assert.doesNotMatch(batchRs, /fn create_recognizer/u);
});

test('local streaming ASR offline decode is delegated to recognizer helpers', () => {
  const adapterInference = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'inference.rs'),
    'utf8',
  );

  assert.match(adapterInference, /decode_offline_samples/u);
  assert.doesNotMatch(adapterInference, /use sherpa_onnx::OfflineRecognizer/u);
  assert.doesNotMatch(adapterInference, /let stream = r\.create_stream\(\)/u);
});

test('desktop batch offline decode is delegated to local ASR adapter', () => {
  const batchRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );

  assert.match(batchRs, /decode_offline_samples/u);
  assert.doesNotMatch(batchRs, /FFI: Calling accept_waveform \(Offline segment\)/u);
  assert.doesNotMatch(batchRs, /let stream = r\.0\.create_stream\(\)/u);
});

test('desktop streaming offline decode is delegated to local ASR adapter', () => {
  const localAsrAudio = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'audio.rs'),
    'utf8',
  );
  const streamingRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'streaming.rs'),
    'utf8',
  );

  assert.match(localAsrAudio, /pub fn pcm_s16le_bytes_to_f32/u);
  assert.match(streamingRs, /pcm_s16le_bytes_to_f32\(&pcm\)/u);
  assert.match(streamingRs, /decode_offline_samples/u);
  assert.doesNotMatch(streamingRs, /chunks_exact\(2\).*i16::from_le_bytes/u);
  assert.doesNotMatch(streamingRs, /let stream = r\.0\.create_stream\(\)/u);
  assert.doesNotMatch(streamingRs, /stream\.accept_waveform\(16000, &full_audio\)/u);
  assert.doesNotMatch(streamingRs, /r\.0\.decode\(&stream\)/u);
});

test('local streaming ASR online operations use recognizer helpers', () => {
  const batchRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'batch.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const localOnlineRs = `${batchRs}\n${adapterSession}`;

  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*create_online_stream/u);
  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*accept_online_samples/u);
  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*decode_online_ready/u);
  assert.match(adapterSession, /use crate::recognizer::\{[\s\S]*online_stream_result/u);
  assert.doesNotMatch(localOnlineRs, /SafeStream\(r\.0\.create_stream\(\)\)/u);
  assert.doesNotMatch(localOnlineRs, /\.0\.accept_waveform\(16000/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.is_ready\(&[^)]*\.0\)/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.decode\(&[^)]*\.0\)/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.get_result\(&[^)]*\.0\)/u);
  assert.doesNotMatch(localOnlineRs, /r\.0\.reset\(&[^)]*\.0\)/u);
});

test('local streaming ASR VAD runtime operations use private wrappers', () => {
  const audioRs = read('adapters', 'local_asr', 'src', 'audio.rs');
  const recognizerRs = read('adapters', 'local_asr', 'src', 'recognizer.rs');
  const runtimeRs = read('adapters', 'local_asr', 'src', 'runtime.rs');
  const asrMod = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'asr', 'mod.rs'),
    'utf8',
  );
  const adapterSession = fs.readFileSync(
    path.join(repoRoot, 'adapters', 'local_asr', 'src', 'streaming', 'session.rs'),
    'utf8',
  );
  const streamingRs = fs.readFileSync(
    desktopCratePath('src', 'integrations', 'streaming.rs'),
    'utf8',
  );
  const vadConsumers = `${adapterSession}\n${streamingRs}`;

  assert.match(asrMod, /accept_vad_samples/u);
  assert.match(runtimeRs, /pub fn reset_or_reload_vad\(&mut self\)/u);
  assert.match(adapterSession, /instance\.reset_or_reload_vad\(\)/u);
  assert.doesNotMatch(
    adapterSession,
    /^use crate::audio::\{[^}]*(?:load_vad|reset_vad)\b/mu,
  );
  assert.match(asrMod, /vad_detected/u);
  assert.doesNotMatch(vadConsumers, /SafeVad\([^)]+\)/u);
  assert.doesNotMatch(vadConsumers, /\.0\.accept_waveform/u);
  assert.doesNotMatch(vadConsumers, /\.0\.detected/u);
  assert.doesNotMatch(audioRs, /pub struct SafeVad\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeOnlineRecognizer\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeOfflineRecognizer\(pub /u);
  assert.doesNotMatch(recognizerRs, /pub struct SafeStream\(pub /u);
});

test('local ASR VAD sherpa primitives are crate-private', () => {
  const audioRs = read('adapters', 'local_asr', 'src', 'audio.rs');

  assert.doesNotMatch(audioRs, /^pub type VadConfig\s*=/mu);
  assert.doesNotMatch(audioRs, /^pub type VadDetector\s*=/mu);
  assert.doesNotMatch(audioRs, /^pub fn create_vad_config/mu);
  assert.doesNotMatch(audioRs, /^pub fn create_vad_detector/mu);
  assert.doesNotMatch(audioRs, /^pub fn vad_segment_audio_with_capacity/mu);
  assert.match(audioRs, /^pub\(crate\) type VadConfig\s*=/mu);
  assert.match(audioRs, /^pub\(crate\) fn create_vad_config/mu);
});
