import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import {
  exists,
  read,
  readCargoDependencyNames,
  repoRoot,
  rustProductionView,
} from './test-support/repository.js';

function maskRanges(source, ranges) {
  const masked = source.split('');
  for (const [start, end] of ranges) {
    for (let index = start; index < end; index += 1) {
      if (masked[index] !== '\n') masked[index] = ' ';
    }
  }
  return masked.join('');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

function typescriptProductionView(source) {
  const ranges = [];
  for (let index = 0; index < source.length;) {
    if (source.startsWith('//', index)) {
      const end = source.indexOf('\n', index + 2);
      ranges.push([index, end === -1 ? source.length : end]);
      index = end === -1 ? source.length : end;
      continue;
    }
    if (source.startsWith('/*', index)) {
      const end = source.indexOf('*/', index + 2);
      const commentEnd = end === -1 ? source.length : end + 2;
      ranges.push([index, commentEnd]);
      index = commentEnd;
      continue;
    }
    if (source[index] === '"' || source[index] === '\'' || source[index] === '`') {
      const quote = source[index];
      let end = index + 1;
      while (end < source.length) {
        if (source[end] === '\\') end += 2;
        else if (source[end++] === quote) break;
      }
      ranges.push([index, end]);
      index = end;
      continue;
    }
    index += 1;
  }
  return maskRanges(source, ranges);
}

function maskRustLiterals(source) {
  const ranges = [];
  for (let index = 0; index < source.length;) {
    const raw = /^(?:br|rb|r)(#*)"/u.exec(source.slice(index));
    if (raw) {
      const closing = `"${raw[1]}`;
      const end = source.indexOf(closing, index + raw[0].length);
      const literalEnd = end === -1 ? source.length : end + closing.length;
      ranges.push([index, literalEnd]);
      index = literalEnd;
      continue;
    }
    const quoteOffset = source[index] === 'b' && source[index + 1] === '"' ? 1 : 0;
    const character = /^(?:b)?'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,6}\}|[nrt0\\'"])|[^'\\\r\n])'/u
      .exec(source.slice(index));
    if (character) {
      ranges.push([index, index + character[0].length]);
      index += character[0].length;
      continue;
    }
    if (source[index + quoteOffset] === '"') {
      const quote = source[index + quoteOffset];
      let end = index + quoteOffset + 1;
      while (end < source.length) {
        if (source[end] === '\\') end += 2;
        else if (source[end++] === quote) break;
      }
      ranges.push([index, end]);
      index = end;
      continue;
    }
    index += 1;
  }
  return maskRanges(source, ranges);
}

function readBracedBlock(source, structural, signatureIndex) {
  if (signatureIndex === -1) return { production: '', structural: '' };
  const openingBrace = structural.indexOf('{', signatureIndex);
  let depth = 0;
  for (let index = openingBrace; index < structural.length; index += 1) {
    if (structural[index] === '{') depth += 1;
    if (structural[index] === '}') {
      depth -= 1;
      if (depth === 0) {
        return {
          production: source.slice(signatureIndex, index + 1),
          structural: structural.slice(signatureIndex, index + 1),
        };
      }
    }
  }
  return { production: '', structural: '' };
}

function readTypeScriptMethod(source, methodName) {
  const structural = typescriptProductionView(source);
  const signatureIndex = structural.indexOf(`async ${methodName}(`);
  return readBracedBlock(source, structural, signatureIndex).structural;
}

function readRustFunction(source, functionName) {
  const production = rustProductionView(source);
  const structural = maskRustLiterals(production);
  const signature = new RegExp(`\\bfn\\s+${functionName}(?:<[^>{}]*>)?\\s*\\(`, 'u')
    .exec(structural);
  return readBracedBlock(production, structural, signature?.index ?? -1);
}

function readRustStruct(source, structName) {
  const production = rustProductionView(source);
  const structural = maskRustLiterals(production);
  const signature = new RegExp(`\\bstruct\\s+${structName}\\s*\\{`, 'u').exec(structural);
  return readBracedBlock(production, structural, signature?.index ?? -1).production;
}

function addUnexpectedRustParameter(signature) {
  return signature.replace(/,?\s*\)\s*->/u, ', unexpected: String) ->');
}

test('UniFFI backup bridge exposes exact async delegates with lazy target access', () => {
  const cargoDependencies = readCargoDependencyNames(
    path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml'),
    'dependencies',
  );
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  const facade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const bridge = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'backup_bridge.rs'));

  assert.ok(cargoDependencies.includes('sona-archive'));
  assert.match(uniffiLib, /^mod backup_bridge;$/mu);
  assert.match(uniffiLib, /Backup\s*\{\s*reason:\s*String\s*\}/u);

  const operations = [
    {
      name: 'export_backup_archive_json',
      signature: /pub\(crate\) async fn export_backup_archive_json\(\s*app_data_dir: String,\s*archive_path: String,\s*app_version: String,\s*\) -> SonaCoreBindingResult<String>/u,
      facadeSignature: /pub async fn export_backup_archive_json\(\s*app_data_dir: String,\s*archive_path: String,\s*app_version: String,?\s*\) -> SonaCoreBindingResult<String>/u,
      topSignature: /#\[uniffi::export\]\s*pub async fn export_backup_archive_json\(\s*app_data_dir: String,\s*archive_path: String,\s*app_version: String,?\s*\) -> SonaCoreBindingResult<String>/u,
      facadeCall: /backup_bridge::export_backup_archive_json\(app_data_dir, archive_path, app_version\)\s*\.await/u,
      topCall: /SonaCoreFacade::export_backup_archive_json\(app_data_dir, archive_path, app_version\)\s*\.await/u,
      request: /BackupExportRequest\s*\{/u,
      serviceCall: /\.export_archive\(/u,
      transportFields: ['app_data_dir', 'archive_path', 'app_version'],
    },
    {
      name: 'inspect_backup_archive_json',
      signature: /pub\(crate\) async fn inspect_backup_archive_json\(\s*archive_path: String,\s*\) -> SonaCoreBindingResult<String>/u,
      facadeSignature: /pub async fn inspect_backup_archive_json\(\s*archive_path: String,?\s*\) -> SonaCoreBindingResult<String>/u,
      topSignature: /#\[uniffi::export\]\s*pub async fn inspect_backup_archive_json\(\s*archive_path: String,?\s*\) -> SonaCoreBindingResult<String>/u,
      facadeCall: /backup_bridge::inspect_backup_archive_json\(archive_path\)\s*\.await/u,
      topCall: /SonaCoreFacade::inspect_backup_archive_json\(archive_path\)\s*\.await/u,
      request: /BackupInspectRequest\s*\{/u,
      serviceCall: /\.inspect_archive\(/u,
      transportFields: ['archive_path'],
    },
    {
      name: 'import_backup_archive_json',
      signature: /pub\(crate\) async fn import_backup_archive_json\(\s*app_data_dir: String,\s*archive_path: String,\s*default_rule_set_name: String,\s*confirm_replace: bool,\s*\) -> SonaCoreBindingResult<String>/u,
      facadeSignature: /pub async fn import_backup_archive_json\(\s*app_data_dir: String,\s*archive_path: String,\s*default_rule_set_name: String,\s*confirm_replace: bool,?\s*\) -> SonaCoreBindingResult<String>/u,
      topSignature: /#\[uniffi::export\]\s*pub async fn import_backup_archive_json\(\s*app_data_dir: String,\s*archive_path: String,\s*default_rule_set_name: String,\s*confirm_replace: bool,?\s*\) -> SonaCoreBindingResult<String>/u,
      facadeCall: /backup_bridge::import_backup_archive_json\(\s*app_data_dir,\s*archive_path,\s*default_rule_set_name,\s*confirm_replace,\s*\)\s*\.await/u,
      topCall: /SonaCoreFacade::import_backup_archive_json\(\s*app_data_dir,\s*archive_path,\s*default_rule_set_name,\s*confirm_replace,\s*\)\s*\.await/u,
      request: /BackupImportRequest\s*\{/u,
      serviceCall: /\.import_archive\(/u,
      transportFields: ['app_data_dir', 'archive_path', 'default_rule_set_name'],
    },
  ];

  const bridgeAsyncNames = [
    ...bridge.matchAll(/pub\(crate\)\s+async\s+fn\s+([a-z0-9_]+)\s*\(/gu),
  ].map((match) => match[1]);
  assert.deepEqual(bridgeAsyncNames, operations.map(({ name }) => name));

  for (const operation of operations) {
    const bridgeFunction = readRustFunction(bridge, operation.name).structural;
    const facadeFunction = readRustFunction(facade, operation.name).structural;
    const topFunction = readRustFunction(uniffiLib, operation.name).structural;
    assert.match(bridge, operation.signature);
    assert.match(facade, operation.facadeSignature);
    assert.match(uniffiLib, operation.topSignature);
    const facadeSignature = facade.match(operation.facadeSignature)?.[0] ?? '';
    const topSignature = uniffiLib.match(operation.topSignature)?.[0] ?? '';
    assert.doesNotMatch(
      addUnexpectedRustParameter(facadeSignature),
      operation.facadeSignature,
    );
    assert.doesNotMatch(
      addUnexpectedRustParameter(topSignature),
      operation.topSignature,
    );
    assert.notEqual(facadeFunction, '', `missing facade delegate ${operation.name}`);
    assert.notEqual(topFunction, '', `missing top-level export ${operation.name}`);
    assert.match(facadeFunction, operation.facadeCall);
    assert.match(topFunction, operation.topCall);
    assert.match(bridgeFunction, /tokio::task::spawn_blocking/u);
    assert.equal(bridgeFunction.match(/FsBackupArchiveRepository::new\(\)/gu)?.length, 1);
    assert.match(bridgeFunction, /BackupService::new\([^,]+,\s*[^,]+,\s*&SystemClock\)/u);
    assert.match(bridgeFunction, operation.request);
    assert.match(bridgeFunction, operation.serviceCall);
    assert.match(bridgeFunction, /std::path::absolute\(/u);
    assert.doesNotMatch(bridgeFunction, /ensure_existing_directory\(|Database::open\(/u);
    assert.doesNotMatch(bridgeFunction, /to_string_pretty/u);
    const blockingIndex = bridgeFunction.indexOf('tokio::task::spawn_blocking');
    for (const field of operation.transportFields) {
      const validationIndex = bridgeFunction.search(
        new RegExp(`require_non_empty\\(\\s*&${field}\\b`, 'u'),
      );
      assert.notEqual(validationIndex, -1, `${operation.name} must validate ${field}`);
      assert.ok(validationIndex < blockingIndex, `${field} validation must precede spawn_blocking`);
    }
  }

  const snapshot = readRustFunction(bridge, 'snapshot').structural;
  const replaceAll = readRustFunction(bridge, 'replace_all').structural;
  assert.match(bridge, /struct LazySqliteBackupStateRepository/u);
  assert.match(bridge, /impl BackupStateRepository for LazySqliteBackupStateRepository/u);
  assert.match(snapshot, /ensure_existing_directory\(/u);
  assert.match(snapshot, /Database::open\(/u);
  const preflightIndex = replaceAll.indexOf('validate_backup_restore_dataset(');
  const directoryIndex = replaceAll.indexOf('ensure_existing_directory(');
  const databaseIndex = replaceAll.indexOf('Database::open(');
  assert.notEqual(preflightIndex, -1, 'lazy replacement must run shared SQLite preflight');
  assert.ok(preflightIndex < directoryIndex, 'preflight must precede target directory access');
  assert.ok(directoryIndex < databaseIndex, 'directory validation must precede target Database::open');

  const canonical = readRustFunction(bridge, 'canonical_json').structural;
  assert.match(canonical, /serde_json::to_value\(/u);
  assert.match(canonical, /serde_json::to_string\(/u);
  assert.doesNotMatch(canonical, /to_string_pretty/u);
});

test('desktop backup commands preserve transport signatures and compose Core in blocking runners', () => {
  const commands = rustProductionView(read('platforms', 'desktop', 'src', 'commands', 'history.rs'));
  const platform = rustProductionView(read('platforms', 'desktop', 'src', 'platform', 'history_repository.rs'));
  const coreHistory = read('core', 'src', 'history', 'mod.rs');

  const commandExpectations = [
    ['export_backup_archive', /request: ExportBackupArchiveRequest[\s\S]*Result<BackupManifest, String>/u],
    ['prepare_backup_import', /archive_path: String[\s\S]*Result<PreparedBackupImport, String>/u],
    ['apply_prepared_history_import', /import_id: String[\s\S]*Result<\(\), String>/u],
    ['dispose_prepared_backup_import', /import_id: String[\s\S]*Result<\(\), String>/u],
  ];
  for (const [name, signature] of commandExpectations) {
    const command = readRustFunction(commands, name).structural;
    const runner = readRustFunction(platform, name).structural;
    assert.notEqual(command, '', `missing Tauri command ${name}`);
    assert.match(command, signature);
    assert.match(command, new RegExp(`history_repository::${name}\\b`, 'u'));
    assert.match(runner, /run_backup_service\(/u);
    assert.match(runner, /BackupService::new\(/u);
  }

  const composition = readRustFunction(platform, 'run_backup_service').structural;
  assert.match(composition, /tauri::async_runtime::spawn_blocking/u);
  assert.match(composition, /FsBackupArchiveRepository/u);
  assert.match(composition, /SqliteBackupStateRepository/u);
  assert.match(platform, /sona_runtime_fs::SystemClock/u);
  assert.match(
    readRustFunction(platform, 'apply_prepared_history_import').production,
    /default_rule_set_name:\s*"Default Rules"\.to_string\(\)/u,
  );
  const compatibilityRequest = readRustStruct(coreHistory, 'ExportBackupArchiveRequest');
  for (const [field, type] of [
    ['config', 'Value'],
    ['projects', 'Vec<Value>'],
    ['automation_rules', 'Vec<Value>'],
    ['automation_processed_entries', 'Vec<Value>'],
    ['analytics_content', 'String'],
  ]) {
    assert.match(
      compatibilityRequest,
      new RegExp(`#\\[serde\\(default\\)\\]\\s*pub ${field}: ${escapeRegExp(type)},`, 'u'),
    );
  }
});

test('desktop backup state owns one archive adapter and legacy SQLite tar orchestration is absent', () => {
  const state = rustProductionView(read(
    'platforms', 'desktop', 'src', 'platform', 'history_repository', 'state.rs',
  ));
  const sqliteLib = rustProductionView(read('adapters', 'sqlite', 'src', 'lib.rs'));
  const platform = rustProductionView(read(
    'platforms', 'desktop', 'src', 'platform', 'history_repository.rs',
  ));
  const fsUtils = rustProductionView(read('adapters', 'sqlite', 'src', 'history_fs_utils.rs'));
  const desktopLib = rustProductionView(read('platforms', 'desktop', 'src', 'lib.rs'));
  const sqliteDependencies = readCargoDependencyNames(
    path.join(repoRoot, 'adapters', 'sqlite', 'Cargo.toml'),
    'dependencies',
  );

  assert.match(state, /Arc<FsBackupArchiveRepository>/u);
  assert.match(
    desktopLib,
    /\.manage\(crate::platform::history_repository::PreparedBackupImportState::default\(\)\)/u,
  );
  assert.doesNotMatch(state, /HashMap|PreparedBackupImportSnapshot/u);
  assert.equal(exists('adapters', 'sqlite', 'src', 'history_backup.rs'), false);
  assert.doesNotMatch(sqliteLib, /history_backup/u);
  assert.doesNotMatch(
    platform,
    /history_backup|backup::(?:export_backup_archive_inner|prepare_backup_import_inner|apply_prepared_history_import_inner)|fs_utils/u,
  );
  assert.equal(sqliteDependencies.includes('bzip2'), false);
  assert.equal(sqliteDependencies.includes('tar'), false);
  assert.doesNotMatch(fsUtils, /create_tar_bz2_archive|extract_tar_bz2_archive/u);
});

test('frontend backup transport keeps four payloads while new exports contain only path and version', () => {
  const contracts = typescriptProductionView(read(
    'platforms', 'desktop', 'frontend', 'src', 'services', 'tauri', 'contracts.ts',
  ));
  const transport = typescriptProductionView(read(
    'platforms', 'desktop', 'frontend', 'src', 'services', 'tauri', 'backup.ts',
  ));

  assert.match(contracts, /type ExportBackupArchiveRequest\s*=\s*\{\s*archivePath: string;\s*appVersion: string;\s*\}/u);
  assert.match(contracts, /backup\.exportArchive\]: \{\s*args: \{ request: ExportBackupArchiveRequest \};\s*result: BackupManifestV1;/u);
  assert.match(contracts, /backup\.prepareImport\]: \{\s*args: \{ archivePath: string \};\s*result: unknown;/u);
  assert.match(contracts, /backup\.applyPreparedImport\]: \{\s*args: \{ importId: string \};\s*result: void;/u);
  assert.match(contracts, /backup\.disposePreparedImport\]: \{\s*args: \{ importId: string \};\s*result: void;/u);
  assert.match(transport, /exportArchive, \{ request \}/u);
  assert.match(transport, /prepareImport, \{ archivePath \}/u);
  assert.match(transport, /applyPreparedImport, \{ importId \}/u);
  assert.match(transport, /disposePreparedImport, \{ importId \}/u);
});

test('frontend import performs one atomic replacement then reloads and always disposes', () => {
  const source = read('platforms', 'desktop', 'frontend', 'src', 'services', 'backupService.ts');
  const apply = readTypeScriptMethod(source, 'applyImportBackup');

  assert.notEqual(apply, '', 'missing applyImportBackup method');
  assert.equal(apply.match(/applyPreparedHistoryImport\(/gu)?.length, 1);
  assert.match(apply, /reloadConfig\(/u);
  assert.match(apply, /loadProjects\(/u);
  assert.match(apply, /loadHistoryItems\(/u);
  assert.match(apply, /loadAndStartAutomation\(/u);
  assert.match(apply, /syncOpenTranscriptAfterImport\(/u);
  assert.match(apply, /finally[\s\S]*disposePreparedImport\(/u);
  assert.doesNotMatch(
    apply,
    /settingsStore(?:Set|Save)|setConfig\(|projectServiceSaveAll|saveAutomation|llmUsageReplaceRaw|migrateConfig/u,
  );

  const controller = typescriptProductionView(read(
    'platforms', 'desktop', 'frontend', 'src', 'components', 'settings', 'backup',
    'useBackupSettingsController.ts',
  ));
  assert.equal(controller.match(/\.\.\.preparedBackupImportActions/gu)?.length, 2);
  assert.match(controller, /apply:\s*applyImportBackup/u);
  assert.match(controller, /dispose:\s*disposePreparedImport/u);
  assert.doesNotMatch(
    controller,
    /backupService\.(?:applyImportBackup|disposePreparedImport)/u,
  );
});

test('CLI backup exposes exact grammar, lazy SQLite state, one-shot Core routes, and canonical output', () => {
  const cli = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const backup = read('platforms', 'cli', 'src', 'backup.rs');
  const production = rustProductionView(backup);
  const dependencies = readCargoDependencyNames(
    path.join(repoRoot, 'platforms', 'cli', 'Cargo.toml'),
    'dependencies',
  );

  assert.match(cli, /mod backup;/u);
  assert.match(cli, /Backup\(backup::BackupArgs\)/u);
  assert.match(cli, /Commands::Backup\(args\)\s*=>\s*backup::run_backup\(args\)/u);
  assert.equal(dependencies.includes('sona-archive'), true);

  for (const command of ['Export', 'Inspect', 'Import']) {
    assert.match(production, new RegExp(`\\b${command}\\(Backup${command}Args\\)`, 'u'));
  }
  for (const [field, valueName] of [
    ['app_data_dir', 'DIR'],
    ['output', 'ARCHIVE'],
    ['archive', 'ARCHIVE'],
    ['app_version', 'VERSION'],
    ['default_rule_set_name', 'NAME'],
  ]) {
    assert.match(
      backup,
      new RegExp(`#\\[arg\\(long, value_name = "${valueName}"\\)\\]\\s*${field}: PathBuf|#\\[arg\\(long, value_name = "${valueName}"\\)\\]\\s*${field}: String`, 'u'),
    );
  }
  assert.match(backup, /#\[arg\(long\)\]\s*confirm_replace:\s*bool/u);
  assert.doesNotMatch(backup, /confirm_replace[^\n]*(?:required|default_value|action|prompt)/u);
  const validateImport = readRustFunction(backup, 'validate_import').structural;
  assert.match(validateImport, /confirm_replace:\s*args\.confirm_replace/u);
  assert.doesNotMatch(validateImport, /BackupError::ConfirmationRequired/u);
  assert.doesNotMatch(validateImport, /require_existing_directory\(/u);

  const run = readRustFunction(backup, 'run_backup').structural;
  assert.equal(run.match(/FsBackupArchiveRepository::new\(\)/gu)?.length, 1);
  assert.equal(run.match(/BackupService::new\(/gu)?.length, 1);
  assert.match(run, /CliBackupStateRepository/u);
  assert.match(production, /use sona_runtime_fs::SystemClock;/u);
  assert.match(run, /let clock = SystemClock;/u);
  assert.doesNotMatch(run, /Database::open/u);

  const snapshot = readRustFunction(backup, 'snapshot').structural;
  const replaceAll = readRustFunction(backup, 'replace_all').structural;
  for (const method of [snapshot, replaceAll]) {
    assert.match(method, /Database::open\(/u);
    assert.match(method, /SqliteBackupStateRepository::new\(/u);
  }
  const preflightIndex = replaceAll.indexOf('validate_backup_restore_dataset(');
  const targetOpenIndex = replaceAll.indexOf('Database::open(');
  assert.notEqual(preflightIndex, -1, 'lazy CLI replacement must run shared SQLite preflight');
  assert.ok(preflightIndex < targetOpenIndex, 'shared restore preflight must run before target Database::open');

  assert.equal(production.match(/\.export_archive\(/gu)?.length, 1);
  assert.equal(production.match(/\.inspect_archive\(/gu)?.length, 1);
  assert.equal(production.match(/\.import_archive\(/gu)?.length, 1);
  assert.doesNotMatch(production, /\.prepare_import\(|\.apply_prepared_import\(|\.dispose_prepared_import\(/u);
  assert.match(production, /serde_json::to_value\(/u);
  assert.match(production, /serde_json::to_string\(/u);
  assert.doesNotMatch(production, /to_string_pretty/u);
  assert.match(production, /BackupError::InvalidRequest/u);
  assert.match(production, /BackupError::InvalidBackup/u);
  assert.match(production, /BackupError::ConfirmationRequired/u);
  assert.match(production, /BackupError::Archive/u);
  assert.match(production, /BackupError::State/u);
  assert.match(production, /BackupError::Config/u);
});

test('CLI backup documentation has exact bilingual examples and destructive warning', () => {
  const english = read('docs', 'cli.md');
  const chinese = read('docs', 'cli.zh-CN.md');
  const examples = [
    'sona-cli backup export --app-data-dir ./sona-data --output ./sona-backup.sona-backup --app-version 0.8.0',
    'sona-cli backup inspect --archive ./sona-backup.sona-backup',
    'sona-cli backup import --app-data-dir ./sona-data --archive ./sona-backup.sona-backup --default-rule-set-name "Default Rules" --confirm-replace',
  ];

  for (const document of [english, chinese]) {
    for (const example of examples) assert.match(document, new RegExp(escapeRegExp(example), 'u'));
    for (const scope of ['config', 'workspace', 'history', 'automation', 'analytics']) {
      assert.match(document, new RegExp(`\\b${scope}\\b`, 'u'));
    }
    assert.match(document, /--confirm-replace/u);
  }
  assert.match(english, /atomically replaces/u);
  assert.match(english, /never opens an interactive prompt/u);
  assert.match(english, /task ledger/ui);
  assert.match(english, /original audio/ui);
  assert.match(chinese, /原子|替换/u);
  assert.match(chinese, /不会.*交互|不.*交互/u);
  assert.match(chinese, /任务账本/u);
  assert.match(chinese, /原始音频/u);
});
