import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {
  desktopCratePath,
  desktopCrateSegments,
  desktopFrontendDependencies,
  exists,
  read,
  readWorkflowBuildTauriSteps,
  readWorkflowStep,
  readWorkflowStepIndex,
  repoRoot,
  rustFilesUnder,
  stripRustLineComment,
} from './test-support/repository.js';

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
  assert.equal(fs.existsSync(path.join(repoRoot, 'eslint.config.js')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'tsconfig.json')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'tsconfig.node.json')), false);
  assert.equal(fs.existsSync(path.join(repoRoot, 'playwright.config.ts')), false);
  assert.equal(fs.existsSync(frontend('index.html')), true);
  assert.equal(desktopConfig.build.frontendDist, 'frontend/dist');
  assert.equal(desktopConfig.bundle.resources, undefined);
  assert.equal(desktopConfig.bundle.externalBin, undefined);
  for (const command of [desktopConfig.build.beforeDevCommand, desktopConfig.build.beforeBuildCommand]) {
    assert.equal(typeof command, 'object');
    assert.equal(typeof command.script, 'string');
    assert.equal(command.cwd, 'frontend');
  }
  assert.match(desktopLib, /"frontend\/src\/bindings\.ts"/u);
  assert.ok(frontendPackage.dependencies['@tauri-apps/api']);
  for (const dependencyName of desktopFrontendDependencies) {
    assert.equal(
      rootPackage.dependencies?.[dependencyName],
      undefined,
      `${dependencyName} must not be a root production dependency`,
    );
    assert.equal(
      rootPackage.devDependencies?.[dependencyName],
      undefined,
      `${dependencyName} must not be a root development dependency`,
    );
  }
  assert.equal(rootPackage.dependencies?.ws, undefined, 'root ws dependency has no remaining consumer');
  assert.equal(rootPackage.devDependencies?.ws, undefined, 'root ws dev dependency has no remaining consumer');
  assert.equal(rootPackage.scripts.tauri, 'node platforms/desktop/scripts/tauri.js');
  assert.equal(exists('platforms', 'desktop', 'scripts', 'tauri.js'), true);
  assert.equal(exists('scripts', 'tauri.js'), false);
});

test('desktop tauri crate no longer bundles sona-cli sidecar artifacts', () => {
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

test('packaging cleanup removes legacy source-tree resources and the ffmpeg-static package', () => {
  const frontendPackage = JSON.parse(read('platforms', 'desktop', 'frontend', 'package.json'));
  const lockfile = read('pnpm-lock.yaml');
  const workspaceConfig = read('pnpm-workspace.yaml');
  const rootIgnore = read('.gitignore');
  const desktopIgnore = read(...desktopCrateSegments, '.gitignore');
  const prWorkflow = read('.github', 'workflows', 'pr-guardrails.yml');

  assert.equal(frontendPackage.dependencies?.['ffmpeg-static'], undefined);
  assert.equal(frontendPackage.devDependencies?.['ffmpeg-static'], undefined);
  assert.doesNotMatch(lockfile, /^\s+ffmpeg-static:/mu);
  assert.doesNotMatch(lockfile, /^\s+ffmpeg-static@/mu);
  assert.doesNotMatch(workspaceConfig, /ffmpeg-static/u);
  assert.equal(exists(...desktopCrateSegments, 'scripts', 'setup-ffmpeg.js'), false);
  assert.equal(exists(...desktopCrateSegments, 'scripts', 'setup-sona-cli-resource.js'), false);
  assert.equal(exists(...desktopCrateSegments, 'resources', 'cli', '.gitkeep'), false);
  assert.doesNotMatch(rootIgnore, /platforms\/desktop\/(?:resources\/(?:shared_libs|cli)|binaries)\//u);
  assert.doesNotMatch(desktopIgnore, /^\/(?:binaries|models)$/mu);
  assert.match(desktopIgnore, /^\/target\/?$/mu);
  assert.doesNotMatch(prWorkflow, /setup-ffmpeg|setup-sona-cli-resource/u);
});

test('native runtime linking uses bundle-native loader contracts without runtime path mutation', () => {
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const cliBuild = read('platforms', 'cli', 'build.rs');
  const cliMain = read('platforms', 'cli', 'src', 'main.rs');
  const runtimeFsCargo = read('adapters', 'runtime_fs', 'Cargo.toml');
  const runtimeFsLib = read('adapters', 'runtime_fs', 'src', 'lib.rs');
  const desktopBuild = read(...desktopCrateSegments, 'build.rs');
  const desktopCargo = read(...desktopCrateSegments, 'Cargo.toml');
  const desktopLib = read(...desktopCrateSegments, 'src', 'lib.rs');

  const rpaths = (buildScript) => [
    ...buildScript.matchAll(/cargo:rustc-link-arg=-Wl,-rpath,([^"]+)/gu),
  ].map(([, rpath]) => rpath);

  assert.match(cliCargo, /^build\s*=\s*"build\.rs"/mu);
  assert.match(cliBuild, /SHERPA_ONNX_LIB_DIR/u);
  assert.match(cliBuild, /delayimp\.lib/u);
  assert.match(cliBuild, /\/DELAYLOAD:sherpa-onnx-c-api\.dll/u);
  assert.match(desktopBuild, /SHERPA_ONNX_LIB_DIR/u);
  assert.match(desktopBuild, /sherpa_lib_dir_has_directml/u);
  assert.match(desktopBuild, /delayimp\.lib/u);
  assert.match(desktopBuild, /\/DELAYLOAD:sherpa-onnx-c-api\.dll/u);
  assert.deepEqual(rpaths(desktopBuild), ['$ORIGIN/../lib/sona', '@loader_path/../Frameworks']);
  assert.deepEqual(rpaths(cliBuild), ['$ORIGIN/../lib/sona', '@loader_path/../Frameworks']);
  assert.doesNotMatch(
    desktopBuild,
    /resources\/shared_libs|copy_shared_libs|copy_from_target_dir|find_target_dir|create_dir_all|std::fs::write/u,
  );
  assert.doesNotMatch(runtimeFsLib, /shared_library_directory|SetDllDirectoryW|PCWSTR|OsStrExt/u);
  assert.doesNotMatch(cliMain, /init_cli_shared_library_directory|SetDllDirectoryW|PCWSTR|OsStrExt/u);
  assert.doesNotMatch(
    desktopLib,
    /init_dll_directory|SetDllDirectoryW|PCWSTR|OsStrExt|windows-test-manifest/u,
  );
  assert.doesNotMatch(runtimeFsCargo, /\bwindows\b|Win32_System_LibraryLoader/u);
  assert.doesNotMatch(desktopCargo, /Win32_System_LibraryLoader/u);
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

test('release workflows invoke the bundle preparer without source-tree staging', () => {
  assert.equal(exists('scripts', 'package-sona-cli.js'), false);

  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const workflow = read('.github', 'workflows', workflowName);

    assert.match(workflow, /args:\s*"--target aarch64-apple-darwin"/u);
    assert.match(workflow, /args:\s*"--target x86_64-apple-darwin"/u);
    assert.match(workflow, /node platforms\/desktop\/scripts\/tauri\.js build \$\{\{ matrix\.args \}\}/u);
    assert.doesNotMatch(workflow, /cargo build -p sona-cli|setup-sona-cli-resource|SONA_SKIP_CLI_RESOURCE_PREP/u);
    assert.doesNotMatch(workflow, /\blipo\b/u);
    assert.doesNotMatch(workflow, /node scripts\/package-sona-cli\.js/u);
    assert.doesNotMatch(workflow, /target\/\*\*\/release\/sona-cli-\*\.tar\.gz/u);
  }
});

test('release workflow build steps defer sidecar preparation to the Tauri wrapper', () => {
  for (const workflowName of ['release.yml', 'nightly.yml']) {
    const buildAppStep = readWorkflowStep(workflowName, 'Build the app');
    const verifyBundleStep = readWorkflowStep(workflowName, 'Verify Tauri bundle and shared libraries');
    const buildSteps = readWorkflowBuildTauriSteps(workflowName);

    assert.equal(buildSteps.some((step) => /standalone CLI|Stage .*resource/u.test(step.name ?? '')), false);
    assert.equal(buildAppStep.env.LD_LIBRARY_PATH, '${{ env.SHERPA_ONNX_LIB_DIR }}');
    assert.equal(buildAppStep.env.SONA_SKIP_CLI_RESOURCE_PREP, undefined);
    assert.equal(buildAppStep.run, 'node platforms/desktop/scripts/tauri.js build ${{ matrix.args }}');
    assert.equal(verifyBundleStep.run, 'node platforms/desktop/scripts/verify-tauri-bundle.js ${{ matrix.args }}');
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
  for (const guide of [cliGuide, cliGuideZh]) {
    assert.match(guide, /### `transcribe-live`/u);
    assert.match(guide, /sona-cli transcribe-live/u);
    assert.match(guide, /--input stdin/u);
    assert.match(guide, /16 kHz/u);
    assert.match(guide, /--output-format ndjson/u);
    assert.match(guide, /\[transcribe_live\]/u);
    assert.match(guide, /Ctrl\+C/u);
  }

  assert.doesNotMatch(docs, /main desktop executable/u);
  assert.doesNotMatch(docs, /Sona\.exe transcribe/u);
  assert.doesNotMatch(docs, /Contents\/MacOS\/Sona transcribe/u);
  assert.doesNotMatch(docs, /cargo run --manifest-path src-tauri\/Cargo\.toml/u);
  assert.doesNotMatch(docs, /not part of the current standalone surface yet/u);
  assert.doesNotMatch(docs, /\bsona serve\b/u);
  assert.match(docs, /\bsona-cli serve\b/u);
});
