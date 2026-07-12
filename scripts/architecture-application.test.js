import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { repoRoot, read, exists, desktopCrateSegments, desktopCratePath, assertPrRecoveryCoverage, rustFilesUnder, readCargoDependencyNames, readCargoDependencySpec, readCargoStringArray, assertCargoDependencyVersionAndFeature, stripRustComments, stripKotlinCommentsAndLiterals, scanRustSourcePolicyViolations } from './test-support/repository.js';
import { makeTempRepo } from './test-support/packaging-fixtures.js';

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
    /cargo test -p sona-core -p sona-api-server -p sona-archive -p sona-export -p sona-local-asr -p sona-media-detector -p sona-model-downloads -p sona-online-llm -p sona-online-asr -p sona-recovery-fs -p sona-runtime-fs -p sona-webdav -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
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
  assert.match(tauriRecoveryRepository, /RecoveryService/u);
  assert.match(tauriRecoveryRepository, /FsRecoverySnapshotStore/u);
  assert.match(tauriRecoveryRepository, /FsSourcePathStatusProvider/u);
  assert.doesNotMatch(tauriRecoveryRepository, /impl RecoveryRepository for/u);
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

test('desktop recovery host delegates persistence and normalization', () => {
  const desktopRecovery = fs.readFileSync(
    desktopCratePath('src', 'platform', 'recovery_repository.rs'),
    'utf8',
  );

  assert.match(desktopRecovery, /RecoveryService/u);
  assert.match(desktopRecovery, /FsRecoverySnapshotStore/u);
  assert.match(desktopRecovery, /FsSourcePathStatusProvider/u);
  assert.doesNotMatch(desktopRecovery, /impl RecoveryRepository for/u);
  assert.doesNotMatch(
    desktopRecovery,
    /empty_snapshot|recovered_item_from_(?:queue|saved)_value_with_source_paths|snapshot_from_(?:items_with_timestamp|value_with_source_paths_at)/u,
  );
});

test('pr guardrails run adapter tests with core bindings and standalone CLI', () => {
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.match(
    prWorkflow,
    /cargo test -p sona-core -p sona-api-server -p sona-archive -p sona-export -p sona-local-asr -p sona-media-detector -p sona-model-downloads -p sona-online-llm -p sona-online-asr -p sona-recovery-fs -p sona-runtime-fs -p sona-webdav -p sona-ts-bind -p sona-uniffi-bind -p sona-cli/u,
  );
  assertPrRecoveryCoverage(prWorkflow);
  assert.match(prWorkflow, /cargo test -p sona-core --test preset_models/u);
  assert.match(prWorkflow, /rustup target add aarch64-linux-android/u);
  assert.match(prWorkflow, /yes \| sdkmanager --licenses/u);
  assert.match(prWorkflow, /sdkmanager "ndk;29\.0\.14206865"/u);
  assert.match(prWorkflow, /ANDROID_NDK_HOME=\$ANDROID_HOME\/ndk\/29\.0\.14206865/u);
  assert.match(prWorkflow, /SONA_ANDROID_ABIS:\s*arm64-v8a/u);
  assert.match(prWorkflow, /pnpm run verify:android-uniffi:gradle/u);
  assert.doesNotMatch(prWorkflow, /core::preset_models::tests/u);
});

test('pr recovery guard rejects package and desktop integration omissions', () => {
  const prWorkflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'pr-guardrails.yml'),
    'utf8',
  );

  assert.throws(
    () => assertPrRecoveryCoverage(prWorkflow.replace(' -p sona-recovery-fs', '')),
    /sona-recovery-fs/u,
  );
  assert.throws(
    () => assertPrRecoveryCoverage(
      prWorkflow.replace(
        'cargo test -p sona --test recovery_repository',
        'cargo test --manifest-path platforms/desktop/Cargo.toml --no-run',
      ),
    ),
    /desktop recovery integration tests/u,
  );
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
  assert.match(platformRecovery, /FsRecoverySnapshotStore/u);
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
  assert.match(platformRecovery, /async fn run_recovery_service_task/u);
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
  assert.match(platformAutomation, /AutomationRepositoryService::new/u);
  assert.match(platformAutomation, /AutomationValidationService::new/u);
  assert.match(platformAutomation, /NativeAutomationFileSystem/u);
  assert.match(platformAutomation, /UuidGenerator/u);
  assert.doesNotMatch(platformAutomation, /std::fs/u);
  assert.doesNotMatch(platformAutomation, /should_prepare_export_directory/u);
  assert.doesNotMatch(platformAutomation, /validate_rule_activation_with_environment/u);
  assert.match(platformProject, /sona_sqlite::project::SqliteProjectRepository/u);
  assert.match(platformProject, /ProjectRepositoryService::new/u);
  assert.match(platformProject, /UuidGenerator/u);
  assert.match(platformProject, /SystemClock/u);
  for (const functionName of [
    'list_projects',
    'replace_projects',
    'create_project',
    'update_project',
    'delete_project',
    'reorder_projects',
    'get_active_project_id',
    'set_active_project_id',
  ]) {
    assert.match(platformProject, new RegExp(`pub async fn ${functionName}\\b`, 'u'));
  }
  assert.match(automationCommand, /crate::platform::automation_repository::load_repository_state/u);
  assert.match(projectCommand, /crate::platform::project_repository::\{/u);
  assert.match(automationCommand, /sona_core::automation::\{/u);
  assert.match(projectCommand, /sona_core::project::\{/u);
  assert.doesNotMatch(projectCommand, /ProjectCreateInput/u);
  assert.doesNotMatch(projectCommand, /ProjectListOptions/u);
  assert.doesNotMatch(projectCommand, /SqliteProjectRepository/u);
  assert.doesNotMatch(projectCommand, /\.list\s*\(/u);
  assert.doesNotMatch(projectCommand, /\.save_all_values\s*\(/u);
  assert.doesNotMatch(projectCommand, /\.create\s*\(/u);
  assert.doesNotMatch(projectCommand, /\.update\s*\(/u);
  assert.doesNotMatch(projectCommand, /\.delete\s*\(/u);
  assert.doesNotMatch(projectCommand, /\.reorder\s*\(/u);
  assert.doesNotMatch(projectCommand, /normalize_/u);
  assert.doesNotMatch(projectCommand, /UuidGenerator/u);
  assert.doesNotMatch(projectCommand, /SystemClock/u);
  assert.doesNotMatch(projectCommand, /std::time|SystemTime|Utc::now/u);
  assert.doesNotMatch(platformProject, /normalize_project_record_for_import/u);
  assert.doesNotMatch(platformProject, /normalize_project_value/u);
  assert.doesNotMatch(platformProject, /normalize_defaults/u);
  assert.doesNotMatch(platformProject, /apply_project_patch|patch_project/u);
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
  const llmProvidersTs = read('platforms', 'desktop', 'frontend', 'src', 'services', 'llm', 'providers.ts');

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
  assert.match(llmProvidersTs, /\.\.\/\.\.\/\.\.\/\.\.\/\.\.\/\.\.\/core\/src\/llm\/llm-providers\.json/u);
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
  const coreTaskLedgerService = read('core', 'src', 'task_ledger', 'service.rs');
  const sqliteTaskLedger = read('adapters', 'sqlite', 'src', 'task_ledger.rs');

  assert.equal(fs.existsSync(platformTaskLedgerPath), true);
  const platformTaskLedger = fs.readFileSync(platformTaskLedgerPath, 'utf8');

  assert.match(platformMod, /^pub mod task_ledger_repository;/mu);
  assert.match(platformTaskLedger, /sona_sqlite::task_ledger::SqliteLedgerRepository/u);
  assert.match(platformTaskLedger, /TaskLedgerService::new/u);
  assert.match(platformTaskLedger, /unix_timestamp_millis/u);
  assert.match(platformTaskLedger, /TauriEventEmitter/u);
  assert.match(platformTaskLedger, /TASK_LEDGER_UPDATED_EVENT/u);
  assert.match(platformTaskLedger, /async fn run_task_ledger_service_task/u);
  assert.match(platformTaskLedger, /fn emit_task_ledger_snapshot/u);
  assert.match(platformTaskLedger, /emitter\.emit\(/u);
  assert.match(coreTaskLedgerService, /pub struct TaskLedgerService/u);
  assert.match(sqliteTaskLedger, /impl<D> TaskLedgerStore for SqliteLedgerRepository<D>/u);
  assert.doesNotMatch(sqliteTaskLedger, /SystemTime|UNIX_EPOCH|normalize_record/u);
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

test('task ledger application policy is shared across hosts', () => {
  const coreStore = read('core', 'src', 'task_ledger', 'repository.rs');
  const coreService = read('core', 'src', 'task_ledger', 'service.rs');
  const sqliteAdapter = read('adapters', 'sqlite', 'src', 'task_ledger.rs');
  const desktopAdapter = read(...desktopCrateSegments, 'src', 'platform', 'task_ledger_repository.rs');
  const uniffiCargo = read('adapters', 'uniffi_bind', 'Cargo.toml');
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiBridge = read('adapters', 'uniffi_bind', 'src', 'task_ledger_bridge.rs');
  const cliCargo = read('platforms', 'cli', 'Cargo.toml');
  const cliTaskLedger = read('platforms', 'cli', 'src', 'task_ledger.rs');
  const androidSample = read(
    'platforms',
    'android',
    'sample-consumer',
    'sample-library',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'uniffi',
    'sample',
    'SonaUniffiSmoke.kt',
  );
  const androidConsumer = read(
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

  assert.match(coreStore, /pub trait TaskLedgerStore/u);
  for (const operation of [
    'load_snapshot_at',
    'upsert_task_at',
    'patch_task_at',
    'remove_task_at',
    'clear_resolved_at',
  ]) {
    assert.match(coreService, new RegExp(`pub fn ${operation}`));
  }
  assert.doesNotMatch(
    `${coreStore}\n${coreService}`,
    /rusqlite|tauri|uniffi|SystemTime|UNIX_EPOCH|std::fs|std::process|std::net/u,
  );

  assert.match(sqliteAdapter, /impl<D> TaskLedgerStore for SqliteLedgerRepository<D>/u);
  assert.doesNotMatch(
    sqliteAdapter,
    /normalize_record|normalize_loaded_record|is_retained_status|SystemTime|UNIX_EPOCH|TaskLedgerSnapshot/u,
  );
  assert.match(desktopAdapter, /TaskLedgerService::new/u);
  assert.match(uniffiBridge, /TaskLedgerService::new/u);
  assert.match(cliTaskLedger, /TaskLedgerService::new/u);
  assert.match(cliTaskLedger, /Database::open_read_only/u);
  assert.doesNotMatch(cliTaskLedger, /Database::open\(/u);
  assert.match(uniffiCargo, /sona-sqlite\s*=\s*\{\s*path\s*=\s*"\.\.\/sqlite"\s*\}/u);
  assert.match(cliCargo, /sona-sqlite\s*=\s*\{\s*path\s*=\s*"\.\.\/\.\.\/adapters\/sqlite"\s*\}/u);

  for (const exportName of [
    'load_task_ledger_snapshot_json',
    'upsert_task_ledger_record_json',
    'patch_task_ledger_record_json',
    'remove_task_ledger_record_json',
    'clear_resolved_task_ledger_records_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}\\(`));
  }

  assert.match(androidSample, /import uniffi\.sona_uniffi_bind\.loadTaskLedgerSnapshotJson/u);
  assert.match(androidSample, /import uniffi\.sona_uniffi_bind\.upsertTaskLedgerRecordJson/u);
  assert.match(androidSample, /loadTaskLedgerSnapshotJson\(appDataDir\)/u);
  assert.match(androidSample, /upsertTaskLedgerRecordJson\(appDataDir, recordJson\)/u);
  assert.match(androidConsumer, /import uniffi\.sona_uniffi_bind\.loadTaskLedgerSnapshotJson/u);
  assert.match(androidConsumer, /loadTaskLedgerSnapshotJson\(appDataDir\)/u);
});

test('automation application policy is shared across hosts', () => {
  const coreStore = read('core', 'src', 'automation', 'repository.rs');
  const coreService = read('core', 'src', 'automation', 'service.rs');
  const sqliteAdapter = read('adapters', 'sqlite', 'src', 'automation.rs');
  const runtimeFs = read('adapters', 'runtime_fs', 'src', 'lib.rs');
  const desktopAdapter = read(
    ...desktopCrateSegments,
    'src',
    'platform',
    'automation_repository.rs',
  );
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  const uniffiBridge = read('adapters', 'uniffi_bind', 'src', 'automation_bridge.rs');
  const uniffiLib = read('adapters', 'uniffi_bind', 'src', 'lib.rs');
  const uniffiFacade = read('adapters', 'uniffi_bind', 'src', 'facade.rs');
  const workspaceCargoPath = path.join(repoRoot, 'Cargo.toml');
  const cliCargoPath = path.join(repoRoot, 'platforms', 'cli', 'Cargo.toml');
  const cliLib = read('platforms', 'cli', 'src', 'lib.rs');
  const cliAutomation = read('platforms', 'cli', 'src', 'automation.rs');
  const androidSample = read(
    'platforms',
    'android',
    'sample-consumer',
    'sample-library',
    'src',
    'main',
    'kotlin',
    'com',
    'sona',
    'uniffi',
    'sample',
    'SonaUniffiSmoke.kt',
  );
  const androidConsumer = read(
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
  const rustProductionView = (source) => stripRustComments(
    source.split(/#\[cfg\(test\)\]/u)[0],
  );
  const coreStoreProduction = rustProductionView(coreStore);
  const coreServiceProduction = rustProductionView(coreService);
  const coreProduction = `${coreStoreProduction}\n${coreServiceProduction}`;
  const coreForbiddenCapability =
    /rusqlite|tauri|uniffi|std::fs|std::net|std::path|std::process|std::time|tokio|\bUuid\b|\bSystemTime\b|\bUNIX_EPOCH\b/u;
  const sqliteProduction = rustProductionView(sqliteAdapter);
  const runtimeFsProduction = rustProductionView(runtimeFs);
  const desktopProduction = rustProductionView(desktopAdapter);
  const uniffiBridgeProduction = rustProductionView(uniffiBridge);
  const uniffiLibProduction = rustProductionView(uniffiLib);
  const uniffiFacadeProduction = rustProductionView(uniffiFacade);
  const cliLibProduction = rustProductionView(cliLib);
  const cliAutomationProduction = rustProductionView(cliAutomation);
  const androidSampleExecutable = stripKotlinCommentsAndLiterals(androidSample);
  const androidConsumerExecutable = stripKotlinCommentsAndLiterals(androidConsumer);
  const rustFilteringFixture = rustProductionView(`
    // pub trait AutomationStore
    /* AutomationRepositoryService::new */
    #[cfg(test)]
    mod tests { fn replace_state() {} }
  `);
  const cargoFixturePath = path.join(makeTempRepo(), 'Cargo.toml');
  fs.writeFileSync(
    cargoFixturePath,
    `[workspace]
members = [
  # "platforms/cli",
  "core#tools]",
  "core",
]

[dependencies]
# sona-core = { path = "../../core" }
sona-sqlite = { path = "../sqlite" }
`,
  );
  const kotlinFilteringFixture = stripKotlinCommentsAndLiterals(`
    // import uniffi.sona_uniffi_bind.loadAutomationRepositoryStateJson
    /* validateAutomationRuleActivationJson(ruleJson, globalConfigJson, projectJson) */
    val callShapedLiteral = "loadAutomationRepositoryStateJson(appDataDir)"
  `);
  const coreWallClockCapabilityFixtures = ['std::time', 'SystemTime', 'UNIX_EPOCH'];

  assert.equal(rustFilteringFixture.trim(), '');
  assert.deepEqual(
    readCargoStringArray(cargoFixturePath, 'workspace', 'members'),
    ['core#tools]', 'core'],
  );
  assert.equal(readCargoDependencySpec(cargoFixturePath, 'dependencies', 'sona-core'), '');
  assert.equal(
    readCargoDependencySpec(cargoFixturePath, 'dependencies', 'sona-sqlite'),
    '{ path = "../sqlite" }',
  );
  assert.doesNotMatch(
    kotlinFilteringFixture,
    /loadAutomationRepositoryStateJson|validateAutomationRuleActivationJson/u,
  );
  for (const fixture of coreWallClockCapabilityFixtures) {
    assert.match(fixture, coreForbiddenCapability);
  }
  assert.match(coreStoreProduction, /pub trait AutomationStore/u);
  for (const operation of [
    'load_state',
    'replace_rules',
    'replace_processed_entries',
    'replace_state',
  ]) {
    assert.match(coreStoreProduction, new RegExp(`fn ${operation}\\(`));
  }
  assert.match(coreServiceProduction, /pub trait AutomationIdGenerator/u);
  assert.match(coreServiceProduction, /pub trait AutomationFileSystem/u);
  for (const operation of [
    'load_state',
    'replace_rules_json',
    'replace_processed_entries_json',
    'replace_state_json',
  ]) {
    assert.match(coreServiceProduction, new RegExp(`pub fn ${operation}\\(`));
  }
  for (const storeOperation of [
    'load_state',
    'replace_rules',
    'replace_processed_entries',
    'replace_state',
  ]) {
    assert.match(coreServiceProduction, new RegExp(`self\\.store\\.${storeOperation}\\(`));
  }
  assert.match(coreServiceProduction, /fn normalize_rule_record\(/u);
  assert.match(coreServiceProduction, /fn normalize_processed_record\(/u);
  assert.match(coreServiceProduction, /normalize_rule_record\(&rule, self\.ids\)/u);
  assert.match(coreServiceProduction, /normalize_processed_record\(&entry, self\.ids\)/u);
  assert.match(coreServiceProduction, /ids\.generate_id\(\)/u);
  assert.match(coreServiceProduction, /pub struct AutomationValidationService/u);
  assert.match(coreServiceProduction, /self\.fs\.path_exists\(/u);
  assert.match(coreServiceProduction, /self\.fs\.create_dir_all\(/u);
  assert.match(coreServiceProduction, /validate_rule_activation\(\s*rule,/u);
  assert.doesNotMatch(
    coreProduction,
    coreForbiddenCapability,
  );

  assert.match(
    sqliteProduction,
    /impl<D> AutomationStore for SqliteAutomationRepository<D>/u,
  );
  for (const operation of [
    'load_state',
    'replace_rules',
    'replace_processed_entries',
    'replace_state',
  ]) {
    assert.match(sqliteProduction, new RegExp(`fn ${operation}\\(`));
  }
  assert.match(sqliteProduction, /with_read_connection/u);
  assert.match(sqliteProduction, /with_transaction/u);
  assert.doesNotMatch(
    sqliteProduction,
    /serde_json|ensure_id|\bUuid\b|\buuid\b|\bValue\b/u,
  );

  assert.match(
    runtimeFsProduction,
    /impl AutomationFileSystem for NativeAutomationFileSystem/u,
  );
  assert.match(runtimeFsProduction, /impl AutomationIdGenerator for UuidGenerator/u);
  assert.match(runtimeFsProduction, /fs::create_dir_all\(path\)/u);
  assert.match(runtimeFsProduction, /Uuid::new_v4\(\)/u);

  assert.match(desktopProduction, /AutomationRepositoryService::new/u);
  assert.match(desktopProduction, /AutomationValidationService::new/u);
  assert.match(desktopProduction, /SqliteAutomationRepository::new/u);
  assert.match(desktopProduction, /NativeAutomationFileSystem/u);
  assert.match(desktopProduction, /UuidGenerator/u);

  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');
  assert.equal(
    readCargoDependencySpec(uniffiCargoPath, 'dependencies', 'sona-core'),
    '{ path = "../../core" }',
  );
  assert.equal(
    readCargoDependencySpec(uniffiCargoPath, 'dependencies', 'sona-runtime-fs'),
    '{ path = "../runtime_fs" }',
  );
  assert.equal(
    readCargoDependencySpec(uniffiCargoPath, 'dependencies', 'sona-sqlite'),
    '{ path = "../sqlite" }',
  );
  assert.match(uniffiBridgeProduction, /AutomationRepositoryService::new/u);
  assert.match(uniffiBridgeProduction, /AutomationValidationService::new/u);
  assert.match(uniffiBridgeProduction, /SqliteAutomationRepository::new/u);
  assert.match(uniffiBridgeProduction, /NativeAutomationFileSystem/u);
  assert.match(uniffiBridgeProduction, /UuidGenerator/u);

  const automationExports = [
    'load_automation_repository_state_json', 'replace_automation_rules_json',
    'replace_automation_processed_entries_json', 'replace_automation_repository_state_json',
    'validate_automation_rule_activation_json',
  ];
  const projectExports = [
    'load_project_repository_state_json', 'replace_projects_json',
    'create_project_json', 'update_project_json', 'delete_project',
    'reorder_projects_json', 'set_active_project_id',
  ];
  const preExistingUniffiExports = [
    'load_recovery_snapshot_json', 'save_recovery_snapshot_json',
    'persist_recovery_queue_snapshot_json', 'load_task_ledger_snapshot_json',
    'upsert_task_ledger_record_json', 'patch_task_ledger_record_json',
    'remove_task_ledger_record_json', 'clear_resolved_task_ledger_records_json',
    'normalize_export_format', 'default_vad_model_id',
    'default_punctuation_model_id', 'preset_model_name',
    'preset_models', 'model_catalog_snapshot',
    'model_catalog_selected_ids', 'resolve_model_download',
    'resolve_gpu_acceleration', 'default_config_json',
    'migrate_app_config_json', 'resolve_effective_config_json',
    'runtime_path_status', 'create_online_asr_streaming_session',
    'default_batch_segmentation_mode', 'online_asr_providers',
    'find_online_asr_provider', 'online_asr_provider_request',
    'volcengine_doubao_asr_config_from_json', 'llm_providers',
    'find_llm_provider_by_id_or_alias', 'llm_config_from_json',
    'validate_llm_config_json', 'validate_llm_generate_request_json',
    'validate_polish_segments_request_json', 'validate_translate_segments_request_json',
    'validate_summarize_transcript_request_json', 'llm_segment_inputs_from_transcript_json',
    'summary_segment_inputs_from_transcript_json', 'merge_translated_items_into_transcript_json',
    'merge_polished_items_into_transcript_json', 'summary_source_fingerprint_from_transcript_json',
    'build_polish_prompt_json', 'build_translate_prompt_json',
    'build_summary_chunk_prompt_json', 'build_summary_finalize_prompt_json',
    'plan_polish_prompt_chunks_json', 'plan_translate_prompt_chunks_json',
    'plan_summary_prompt_chunks_json', 'parse_polish_chunk_json',
    'parse_translate_chunk_json', 'polish_segments_request_from_json',
    'translate_segments_request_from_json', 'summarize_transcript_request_from_json',
  ];
  const automationExportInsertionIndex = preExistingUniffiExports.indexOf(
    'normalize_export_format',
  );
  const expectedUniffiExports = [
    ...projectExports,
    ...preExistingUniffiExports.slice(0, automationExportInsertionIndex),
    ...automationExports,
    ...preExistingUniffiExports.slice(automationExportInsertionIndex),
  ];
  const currentUniffiExports = [
    ...uniffiLibProduction.matchAll(/#\[uniffi::export\]\s*pub fn ([a-z0-9_]+)\s*\(/gu),
  ].map((match) => match[1]);
  assert.deepEqual(currentUniffiExports, expectedUniffiExports);
  for (const exportName of automationExports) {
    assert.match(uniffiBridgeProduction, new RegExp(`pub\\(crate\\) fn ${exportName}\\(`));
    assert.match(uniffiFacadeProduction, new RegExp(`pub fn ${exportName}\\(`));
    assert.match(
      uniffiFacadeProduction,
      new RegExp(`automation_bridge::${exportName}\\(`),
    );
    assert.match(uniffiLibProduction, new RegExp(`SonaCoreFacade::${exportName}\\(`));
  }

  assert.ok(
    readCargoStringArray(workspaceCargoPath, 'workspace', 'members').includes('platforms/cli'),
  );
  assert.equal(
    readCargoDependencySpec(cliCargoPath, 'dependencies', 'sona-core'),
    '{ path = "../../core" }',
  );
  assert.equal(
    readCargoDependencySpec(cliCargoPath, 'dependencies', 'sona-runtime-fs'),
    '{ path = "../../adapters/runtime_fs" }',
  );
  assert.equal(
    readCargoDependencySpec(cliCargoPath, 'dependencies', 'sona-sqlite'),
    '{ path = "../../adapters/sqlite" }',
  );
  assert.match(cliLibProduction, /^mod automation;/mu);
  assert.match(
    cliLibProduction,
    /Commands::Automation\(args\) => automation::run_automation\(args\)/u,
  );
  assert.match(cliAutomationProduction, /AutomationRepositoryService::new/u);
  assert.match(cliAutomationProduction, /SqliteAutomationRepository::new/u);
  assert.match(cliAutomationProduction, /Database::open_read_only/u);
  assert.match(cliAutomationProduction, /\.load_state\(\)/u);
  assert.doesNotMatch(cliAutomationProduction, /Database::open\(/u);
  const cliPersistencePattern =
    /\b(?:replace_rules|replace_processed_entries|replace_state)(?:_json)?\s*\(|\bwith_transaction\s*\(|\b(?:insert|update|delete)\b/iu;
  assert.doesNotMatch('AutomationRepositoryState', cliPersistencePattern);
  assert.doesNotMatch(cliAutomationProduction, cliPersistencePattern);

  assert.match(
    androidSampleExecutable,
    /^\s*import\s+uniffi\.sona_uniffi_bind\.loadAutomationRepositoryStateJson\s*$/mu,
  );
  assert.match(
    androidSampleExecutable,
    /^\s*import\s+uniffi\.sona_uniffi_bind\.validateAutomationRuleActivationJson\s*$/mu,
  );
  assert.match(androidSampleExecutable, /loadAutomationRepositoryStateJson\(appDataDir\)/u);
  assert.match(
    androidSampleExecutable,
    /validateAutomationRuleActivationJson\(ruleJson, globalConfigJson, projectJson\)/u,
  );
  assert.match(
    androidConsumerExecutable,
    /^\s*import\s+uniffi\.sona_uniffi_bind\.loadAutomationRepositoryStateJson\s*$/mu,
  );
  assert.match(
    androidConsumerExecutable,
    /loadAutomationRepositoryStateJson\(appDataDir\)/u,
  );
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
  assert.match(platformProjectRepository, /ProjectRepositoryService::new/u);
  assert.match(platformProjectRepository, /UuidGenerator/u);
  assert.match(platformProjectRepository, /SystemClock/u);
  assert.match(platformProjectRepository, /crate::platform::database::sqlite_database/u);
  assert.match(platformProjectRepository, /pub async fn get_active_project_id/u);
  assert.match(platformProjectRepository, /pub async fn set_active_project_id/u);
  assert.match(desktopProjectCommand, /sona_core::project::\{/u);
  assert.doesNotMatch(desktopProjectCommand, /ProjectCreateInput/u);
  assert.doesNotMatch(desktopProjectCommand, /ProjectListOptions/u);
  assert.doesNotMatch(desktopProjectCommand, /SqliteProjectRepository/u);
  assert.doesNotMatch(desktopProjectCommand, /normalize_/u);
  assert.doesNotMatch(desktopProjectCommand, /UuidGenerator/u);
  assert.doesNotMatch(desktopProjectCommand, /SystemClock/u);
  assert.doesNotMatch(desktopProjectCommand, /std::time|SystemTime|Utc::now/u);
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
