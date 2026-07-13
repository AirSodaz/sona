import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), '..');
const clientRoot = path.join(repoRoot, 'platforms', 'android', 'client');

function clientPath(...segments) {
  return path.join(clientRoot, ...segments);
}

function readRepoFile(...segments) {
  const filePath = path.join(repoRoot, ...segments);
  assert.equal(fs.existsSync(filePath), true, `missing repository file: ${filePath}`);
  return fs.readFileSync(filePath, 'utf8');
}

function readClientFile(...segments) {
  const filePath = clientPath(...segments);
  assert.equal(fs.existsSync(filePath), true, `missing Android client file: ${filePath}`);
  return fs.readFileSync(filePath, 'utf8');
}

function readPngDimensions(...segments) {
  const filePath = clientPath(...segments);
  assert.equal(fs.existsSync(filePath), true, `missing PNG file: ${filePath}`);
  const image = fs.readFileSync(filePath);
  assert.deepEqual(
    [...image.subarray(1, 4)],
    [0x50, 0x4e, 0x47],
    `invalid PNG signature: ${filePath}`,
  );
  return {
    width: image.readUInt32BE(16),
    height: image.readUInt32BE(20),
  };
}

function kotlinFilesUnder(...segments) {
  const root = clientPath(...segments);
  if (!fs.existsSync(root)) {
    return [];
  }
  const files = [];
  const pending = [root];
  while (pending.length > 0) {
    const current = pending.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        pending.push(entryPath);
      } else if (entry.isFile() && entry.name.endsWith('.kt')) {
        files.push(entryPath);
      }
    }
  }
  return files.sort();
}

test('Android client Gradle modules preserve the hexagonal dependency direction', () => {
  const settings = readClientFile('settings.gradle.kts');
  const properties = readClientFile('gradle.properties');
  const applicationGradle = readClientFile('application', 'build.gradle.kts');
  const androidGradle = readClientFile('adapters', 'android', 'build.gradle.kts');
  const uniffiGradle = readClientFile('adapters', 'uniffi', 'build.gradle.kts');
  const appGradle = readClientFile('app', 'build.gradle.kts');

  assert.match(settings, /id\("com\.android\.application"\) version "9\.2\.1" apply false/u);
  assert.match(settings, /id\("com\.android\.library"\) version "9\.2\.1" apply false/u);
  assert.match(settings, /id\("org\.jetbrains\.kotlin\.plugin\.compose"\) version "2\.2\.10" apply false/u);
  assert.match(settings, /include\(":application"\)/u);
  assert.match(settings, /include\(":adapters:android"\)/u);
  assert.match(settings, /include\(":adapters:uniffi"\)/u);
  assert.match(settings, /include\(":app"\)/u);
  assert.match(properties, /^SONA_REPO_ROOT=\.\.\/\.\.\/\.\.\/\.\.\/\.\.$/mu);

  for (const source of [applicationGradle, androidGradle, uniffiGradle, appGradle]) {
    assert.match(source, /compileSdk\s*=\s*37/u);
    assert.match(source, /minSdk\s*=\s*23/u);
    assert.match(source, /JavaVersion\.VERSION_17/u);
  }

  assert.match(applicationGradle, /id\("com\.android\.library"\)/u);
  assert.doesNotMatch(
    applicationGradle,
    /project\(":adapters:(?:android|uniffi)"\)|project\(":app"\)|uniffi/u,
  );

  assert.match(androidGradle, /implementation\(project\(":application"\)\)/u);
  assert.doesNotMatch(
    androidGradle,
    /project\(":adapters:uniffi"\)|project\(":app"\)|uniffi/u,
  );

  assert.match(uniffiGradle, /implementation\(project\(":application"\)\)/u);
  assert.match(uniffiGradle, /apply\(from\s*=\s*"\.\.\/\.\.\/\.\.\/sona-uniffi-bindings\.gradle\.kts"\)/u);
  assert.doesNotMatch(uniffiGradle, /project\(":adapters:android"\)|project\(":app"\)/u);

  assert.match(appGradle, /id\("com\.android\.application"\)/u);
  assert.match(appGradle, /id\("org\.jetbrains\.kotlin\.plugin\.compose"\)/u);
  assert.match(appGradle, /targetSdk\s*=\s*37/u);
  assert.match(appGradle, /implementation\(project\(":application"\)\)/u);
  assert.match(appGradle, /implementation\(project\(":adapters:android"\)\)/u);
  assert.match(appGradle, /implementation\(project\(":adapters:uniffi"\)\)/u);
  assert.match(appGradle, /platform\("androidx\.compose:compose-bom:2026\.06\.01"\)/u);
  assert.match(appGradle, /androidx\.compose\.material3:material3-adaptive-navigation-suite/u);

  for (const manifest of [
    clientPath('application', 'src', 'main', 'AndroidManifest.xml'),
    clientPath('adapters', 'android', 'src', 'main', 'AndroidManifest.xml'),
    clientPath('adapters', 'uniffi', 'src', 'main', 'AndroidManifest.xml'),
    clientPath('app', 'src', 'main', 'AndroidManifest.xml'),
  ]) {
    assert.equal(fs.existsSync(manifest), true, `missing Android manifest: ${manifest}`);
  }
});

test('Android UniFFI adapter is the only outbound binding owner', () => {
  const adapter = readClientFile(
    'adapters',
    'uniffi',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'android',
    'adapters',
    'uniffi',
    'UniffiSonaBootstrapAdapter.kt',
  );

  assert.match(adapter, /class UniffiSonaBootstrapAdapter\s*:\s*SonaBootstrapPort/u);
  assert.match(adapter, /^import uniffi\.sona_uniffi_bind\.defaultConfigJson$/mu);
  assert.match(adapter, /defaultConfigJson\s*=\s*defaultConfigJson\(\)/u);
  assert.match(adapter, /onlineStreamingAvailable\s*=\s*true/u);
  assert.match(adapter, /localRuntimePackaged\s*=\s*true/u);
  assert.match(adapter, /localStreamingSessionAvailable\s*=\s*false/u);

  for (const sourcePath of kotlinFilesUnder('application', 'src', 'main')) {
    const source = fs.readFileSync(sourcePath, 'utf8');
    assert.doesNotMatch(
      source,
      /^import\s+(?:android|androidx|uniffi)\./mu,
      `application source crosses a platform boundary: ${sourcePath}`,
    );
  }

  for (const sourcePath of kotlinFilesUnder('app', 'src', 'main')) {
    const source = fs.readFileSync(sourcePath, 'utf8');
    assert.doesNotMatch(
      source,
      /^import\s+uniffi\./mu,
      `app source bypasses the UniFFI adapter: ${sourcePath}`,
    );
  }

  for (const sourcePath of kotlinFilesUnder('adapters', 'android', 'src', 'main')) {
    const source = fs.readFileSync(sourcePath, 'utf8');
    assert.doesNotMatch(
      source,
      /^import\s+uniffi\./mu,
      `Android platform adapter bypasses the UniFFI adapter: ${sourcePath}`,
    );
  }
});

test('Android Compose shell is native, adaptive, and wired at one composition root', () => {
  const mainActivity = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'MainActivity.kt',
  );
  const container = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'SonaAppContainer.kt',
  );
  const app = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'SonaApp.kt',
  );
  const destinations = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'SonaDestination.kt',
  );
  const viewModel = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'SonaViewModel.kt',
  );
  const theme = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'ui', 'theme', 'SonaTheme.kt',
  );
  const strings = readClientFile('app', 'src', 'main', 'res', 'values', 'strings.xml');
  const themes = readClientFile('app', 'src', 'main', 'res', 'values', 'themes.xml');

  assert.match(mainActivity, /class MainActivity\s*:\s*ComponentActivity/u);
  assert.match(mainActivity, /enableEdgeToEdge\(\)/u);
  assert.match(mainActivity, /SonaAppContainer\(/u);
  assert.match(container, /UniffiSonaBootstrapAdapter\(\)/u);
  assert.match(container, /LoadSonaBootstrap\(/u);
  assert.match(viewModel, /sealed interface SonaBootstrapUiState/u);
  assert.match(viewModel, /viewModelScope\.launch/u);

  assert.match(app, /NavigationSuiteScaffold\(/u);
  assert.match(app, /rememberNavController\(\)/u);
  assert.match(app, /NavHost\(/u);
  assert.match(app, /SingleChoiceSegmentedButtonRow\(/u);
  assert.match(app, /dynamicColorEnabled/u);
  assert.match(app, /Switch\(/u);
  assert.match(app, /enabled\s*=\s*false/u);
  assert.match(destinations, /RECORD/u);
  assert.match(destinations, /LIBRARY/u);
  assert.match(destinations, /SETTINGS/u);

  for (const color of ['0xFFFBFBFA', '0xFFF3F3F2', '0xFF37352F', '0xFF5C4D43', '0xFFE03E3E']) {
    assert.match(theme, new RegExp(color));
  }
  assert.match(theme, /dynamicLightColorScheme/u);
  assert.match(theme, /dynamicDarkColorScheme/u);

  assert.match(strings, /<string name="app_name">Sona<\/string>/u);
  assert.match(strings, /<string name="record_action_description">/u);
  assert.match(themes, /Theme\.Material3\.DayNight\.NoActionBar/u);

  const appSources = kotlinFilesUnder('app', 'src', 'main');
  const adapterConstructions = appSources.reduce((count, sourcePath) => {
    const source = fs.readFileSync(sourcePath, 'utf8');
    return count + (source.match(/UniffiSonaBootstrapAdapter\(/gu)?.length ?? 0);
  }, 0);
  assert.equal(adapterConstructions, 1, 'UniFFI adapter must be constructed only by SonaAppContainer');
});

test('Android client has a repeatable local and CI verification entry point', () => {
  const verifier = readRepoFile('scripts', 'verify-android-client.js');
  const apkVerifier = readRepoFile('scripts', 'android-client-apk.js');
  const packageJson = JSON.parse(readRepoFile('package.json'));
  const workflow = readRepoFile('.github', 'workflows', 'pr-guardrails.yml');
  const readme = readRepoFile('platforms', 'android', 'README.md');
  const gitignore = readRepoFile('platforms', 'android', '.gitignore');

  assert.equal(packageJson.scripts['verify:android-client'], 'node scripts/verify-android-client.js');
  assert.match(verifier, /path\.join\(repoRoot, 'scripts', 'run-managed-gradle\.js'\)/u);
  assert.match(verifier, /path\.join\(repoRoot, 'platforms', 'android', 'client'\)/u);
  assert.match(verifier, /':application:testDebugUnitTest'/u);
  assert.match(verifier, /':app:assembleDebug'/u);
  assert.match(verifier, /':app:lintDebug'/u);
  assert.match(
    verifier,
    /SONA_ANDROID_ABIS:\s*process\.env\.SONA_ANDROID_ABIS\s*\?\?\s*'arm64-v8a,x86_64'/u,
  );
  assert.match(apkVerifier, /Missing Android client debug APK/u);

  assert.match(workflow, /sdkmanager "platforms;android-37\.0"/u);
  assert.match(workflow, /rustup target add aarch64-linux-android x86_64-linux-android/u);
  assert.match(workflow, /- name: Run Android client verification[\s\S]*pnpm run verify:android-client/u);
  assert.match(
    workflow,
    /- name: Run Android client verification\s+env:\s+SONA_ANDROID_ABIS: arm64-v8a,x86_64/u,
  );
  assert.match(readme, /## Android client/u);
  assert.match(readme, /pnpm run verify:android-client/u);
  assert.match(readme, /app-arm64-v8a-debug\.apk/u);
  assert.match(readme, /app-x86_64-debug\.apk/u);
  assert.match(gitignore, /^\/client\/\.gradle\/$/mu);
  assert.match(gitignore, /^\/client\/build\/$/mu);
  assert.match(gitignore, /^\/client\/\*\*\/build\/$/mu);
});

test('Android client verification delivers independent arm64-v8a and x86_64 APKs', () => {
  const appGradle = readClientFile('app', 'build.gradle.kts');
  const verifier = readRepoFile('scripts', 'verify-android-client.js');
  const readme = readRepoFile('platforms', 'android', 'README.md');

  assert.match(appGradle, /orElse\("arm64-v8a,x86_64"\)/u);
  assert.match(appGradle, /splits\s*\{[\s\S]*abi\s*\{[\s\S]*isEnable\s*=\s*true/u);
  assert.match(appGradle, /reset\(\)/u);
  assert.match(appGradle, /include\(\*sonaAndroidAbis\.toTypedArray\(\)\)/u);
  assert.match(appGradle, /isUniversalApk\s*=\s*false/u);

  assert.match(
    verifier,
    /SONA_ANDROID_ABIS:\s*process\.env\.SONA_ANDROID_ABIS\s*\?\?\s*'arm64-v8a,x86_64'/u,
  );
  assert.match(verifier, /':app:lintDebug'/u);
  assert.match(
    verifier,
    /import \{ verifyAndroidClientApk \} from '\.\/android-client-apk\.js';/u,
  );
  assert.match(verifier, /verifyAndroidClientApk\(apkPath, abi\)/u);
  for (const abi of ['arm64-v8a', 'x86_64']) {
    assert.match(verifier, new RegExp(`app-${abi}-debug\\.apk`, 'u'));
    assert.match(readme, new RegExp(`app-${abi}-debug\\.apk`, 'u'));
  }
});

test('Android client theme remains API 23 compatible and declares branded launcher icons', () => {
  const manifest = readClientFile('app', 'src', 'main', 'AndroidManifest.xml');
  const themes = readClientFile('app', 'src', 'main', 'res', 'values', 'themes.xml');
  const themesV27 = readClientFile('app', 'src', 'main', 'res', 'values-v27', 'themes.xml');

  assert.doesNotMatch(themes, /android:windowLightNavigationBar/u);
  assert.match(themesV27, /android:windowLightNavigationBar/u);
  assert.match(manifest, /android:icon="@mipmap\/ic_launcher"/u);
  assert.match(manifest, /android:roundIcon="@mipmap\/ic_launcher_round"/u);

  for (const density of ['mdpi', 'hdpi', 'xhdpi', 'xxhdpi', 'xxxhdpi']) {
    for (const icon of ['ic_launcher.png', 'ic_launcher_foreground.png', 'ic_launcher_round.png']) {
      assert.equal(
        fs.existsSync(clientPath('app', 'src', 'main', 'res', `mipmap-${density}`, icon)),
        true,
        `missing ${density} launcher asset: ${icon}`,
      );
    }
  }
  assert.equal(
    fs.existsSync(clientPath(
      'app', 'src', 'main', 'res', 'mipmap-anydpi-v26', 'ic_launcher.xml',
    )),
    true,
    'missing adaptive launcher icon',
  );
});

test('Android bootstrap linkage failures are mapped into the error UI state', () => {
  const viewModel = readClientFile(
    'app', 'src', 'main', 'kotlin', 'com', 'sona', 'android', 'app', 'SonaViewModel.kt',
  );
  assert.match(viewModel, /catch \(error: LinkageError\)/u);
  assert.match(viewModel, /SonaBootstrapUiState\.Error\(error\.message\.orEmpty\(\)\)/u);
});

test('Android client treats lint warnings as failures and supplies themed adaptive icons', () => {
  const appGradle = readClientFile('app', 'build.gradle.kts');
  assert.match(appGradle, /warningsAsErrors\s*=\s*true/u);

  for (const icon of ['ic_launcher.xml', 'ic_launcher_round.xml']) {
    const adaptiveIcon = readClientFile(
      'app', 'src', 'main', 'res', 'mipmap-anydpi-v26', icon,
    );
    assert.match(adaptiveIcon, /<monochrome android:drawable="@mipmap\/ic_launcher_foreground"\s*\/>/u);
  }
});

test('Android launcher raster assets use consistent density-independent sizes', () => {
  const expectedSizes = new Map([
    ['mdpi', 48],
    ['hdpi', 72],
    ['xhdpi', 96],
    ['xxhdpi', 144],
    ['xxxhdpi', 192],
  ]);

  for (const [density, expectedSize] of expectedSizes) {
    for (const icon of ['ic_launcher.png', 'ic_launcher_round.png']) {
      assert.deepEqual(
        readPngDimensions('app', 'src', 'main', 'res', `mipmap-${density}`, icon),
        { width: expectedSize, height: expectedSize },
        `${density} launcher asset has the wrong pixel size: ${icon}`,
      );
    }
  }
});
