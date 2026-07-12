import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  assertCargoDependencyVersionAndFeature,
  desktopCratePath,
  desktopCrateSegments,
  expectedUniffiErrorVariants,
  expectedUniffiExports,
  exists,
  read,
  readCargoDependencyNames,
  readCargoDependencySpec,
  repoRoot,
  rustFilesUnder,
  rustProductionView,
  stripKotlinCommentsAndLiterals,
} from './test-support/repository.js';

function compactRust(source) {
  return source
    .replace(/\s+/gu, ' ')
    .replace(/\s*->\s*/gu, '->')
    .replace(/\s*([<>(){},:&;|=])\s*/gu, '$1')
    .trim();
}

function normalizeRustSignature(source) {
  return compactRust(source).replace(/,\)/gu, ')');
}

function readRustFunctionItem(source, name) {
  const start = new RegExp(
    `(?:#\\s*\\[[^\\]]+\\]\\s*)*(?:pub\\s+)?fn\\s+${name}\\b`,
    'u',
  ).exec(source);
  assert.ok(start, `missing Rust function ${name}`);
  const signatureStart = source.indexOf('fn', start.index);
  const publicSignatureStart = source.lastIndexOf('pub', signatureStart);
  const itemSignatureStart = publicSignatureStart >= start.index
    ? publicSignatureStart
    : signatureStart;
  const bodyStart = source.indexOf('{', signatureStart);
  assert.notEqual(bodyStart, -1, `missing body for ${name}`);
  let depth = 1;
  let bodyEnd = bodyStart + 1;
  while (bodyEnd < source.length && depth > 0) {
    if (source[bodyEnd] === '{') depth += 1;
    else if (source[bodyEnd] === '}') depth -= 1;
    bodyEnd += 1;
  }
  assert.equal(depth, 0, `unbalanced body for ${name}`);
  return {
    attributes: compactRust(source.slice(start.index, itemSignatureStart)),
    signature: normalizeRustSignature(source.slice(itemSignatureStart, bodyStart)),
    body: compactRust(source.slice(bodyStart + 1, bodyEnd - 1)),
    block: source.slice(start.index, bodyEnd),
  };
}

function assertRustFunctionContract(source, name, expected) {
  const actual = readRustFunctionItem(source, name);
  assert.equal(actual.attributes, expected.attributes, `${name} attributes changed`);
  assert.equal(actual.signature, expected.signature, `${name} signature changed`);
  assert.equal(actual.body, expected.body, `${name} body changed`);
  return actual;
}

test('desktop app config commands preserve public contract', () => {
  assert.equal(
    normalizeRustSignature('pub fn get_setting(key: String,) -> Result<(), String>'),
    normalizeRustSignature('pub fn get_setting(key: String) -> Result<(), String>'),
    'signature normalization must ignore a trailing parameter comma',
  );

  const rustFilteringFixture = rustProductionView(String.raw`
    // fn run_app_config_service<T>() { FakeCommentContract }
    /* outer comment
       /* fn run_app_config_service<T>() { FakeNestedContract } */
       AppConfigRepositoryService::new(&fake_store, &fake_clock)
    */
    const URL_TEXT: &str = "https://example.invalid/app-config";
    const COMMENT_TEXT: &str = "/* not a comment */ // still a string";
    const CFG_TEXT: &str = r###"#[cfg(test)]"###;
    #[cfg(test)]
    mod tests {
      const TEST_ONLY_FORBIDDEN: &str = "SqliteConfigStore";
    }
    pub fn production_after_tests() {}
  `);
  assert.match(rustFilteringFixture, /https:\/\/example\.invalid\/app-config/u);
  assert.match(rustFilteringFixture, /\/\* not a comment \*\/ \/\/ still a string/u);
  assert.match(rustFilteringFixture, /r###"#\[cfg\(test\)\]"###/u);
  assert.doesNotMatch(
    rustFilteringFixture,
    /FakeCommentContract|FakeNestedContract|fake_store|TEST_ONLY_FORBIDDEN|SqliteConfigStore/u,
  );
  assert.match(rustFilteringFixture, /pub fn production_after_tests\(\)/u);

  const rustMemberFilteringFixture = rustProductionView(String.raw`
    enum FixtureEnum {
      Variant {
        #[cfg(test)]
        hidden: SqliteConfigStore,
        production: i32,
      },
      #[cfg(test)]
      TestOnly,
      Production,
    }
    union FixtureUnion {
      #[cfg(test)]
      hidden: usize,
      production: u64,
    }
    #[cfg(test)]
    const HEX_ESCAPE: char = '\x7b';
    #[cfg(test)]
    const UNICODE_ESCAPE: char = '\u{7b}';
    #[cfg(test)]
    const BYTE_ESCAPE: u8 = b'\x7b';
    #[cfg(test)]
    const TEST_SIBLING_ONE: usize = 1;
    #[cfg(test)]
    const TEST_SIBLING_TWO: usize = 2;
    pub fn production_after_members() {}
  `);
  assert.doesNotMatch(
    rustMemberFilteringFixture,
    /hidden|TestOnly|HEX_ESCAPE|UNICODE_ESCAPE|BYTE_ESCAPE|TEST_SIBLING/u,
  );
  assert.match(rustMemberFilteringFixture, /production: i32/u);
  assert.match(rustMemberFilteringFixture, /Production,/u);
  assert.match(rustMemberFilteringFixture, /production: u64/u);
  assert.match(rustMemberFilteringFixture, /pub fn production_after_members\(\)/u);

  const commands = rustProductionView(read(
    'platforms', 'desktop', 'src', 'commands', 'system.rs',
  ));
  const platform = rustProductionView(read(
    'platforms', 'desktop', 'src', 'platform', 'app_config.rs',
  ));

  const commandContracts = new Map([
    ['load_app_config', {
      attributes: '#[tauri::command]',
      signature: 'pub fn load_app_config<R:Runtime>(app:AppHandle<R>)->Result<Option<Value>,String>',
      body: 'crate::platform::app_config::load_config(&app)',
    }],
    ['save_app_config', {
      attributes: '#[tauri::command]',
      signature: 'pub fn save_app_config<R:Runtime>(app:AppHandle<R>,config:Value)->Result<(),String>',
      body: 'crate::platform::app_config::save_config(&app,config)',
    }],
    ['get_app_setting', {
      attributes: '#[tauri::command]',
      signature: 'pub fn get_app_setting<R:Runtime>(app:AppHandle<R>,key:String)->Result<Option<Value>,String>',
      body: 'crate::platform::app_config::get_setting(&app,key)',
    }],
    ['set_app_setting', {
      attributes: '#[tauri::command]',
      signature: 'pub fn set_app_setting<R:Runtime>(app:AppHandle<R>,key:String,value:Value)->Result<(),String>',
      body: 'crate::platform::app_config::set_setting(&app,key,value)',
    }],
    ['migrate_app_config', {
      attributes: '#[tauri::command(rename_all="camelCase")]',
      signature: 'pub fn migrate_app_config(saved_config:Option<Value>,legacy_config:Option<Value>,default_rule_set_name:String)->sona_core::config::MigrationResult',
      body: 'sona_core::config::migrate_app_config(saved_config,legacy_config,default_rule_set_name)',
    }],
    ['resolve_effective_config', {
      attributes: '#[tauri::command(rename_all="camelCase")]',
      signature: 'pub fn resolve_effective_config(global_config:Value,project:Option<Value>)->Value',
      body: 'sona_core::config::resolve_effective_config(global_config,project)',
    }],
  ]);

  const platformContracts = new Map([
    ['load_config', {
      attributes: '',
      signature: 'pub fn load_config<R:Runtime>(app:&AppHandle<R>)->Result<Option<Value>,String>',
      body: 'let db=crate::platform::database::sqlite_database(app);run_app_config_service(db,|service|service.load_config())',
    }],
    ['save_config', {
      attributes: '',
      signature: 'pub fn save_config<R:Runtime>(app:&AppHandle<R>,config:Value)->Result<(),String>',
      body: 'let db=crate::platform::database::sqlite_database(app);run_app_config_service(db,|service|service.save_config(&config))',
    }],
    ['get_setting', {
      attributes: '',
      signature: 'pub fn get_setting<R:Runtime>(app:&AppHandle<R>,key:String)->Result<Option<Value>,String>',
      body: 'let db=crate::platform::database::sqlite_database(app);run_app_config_service(db,|service|service.get_setting(&key))',
    }],
    ['set_setting', {
      attributes: '',
      signature: 'pub fn set_setting<R:Runtime>(app:&AppHandle<R>,key:String,value:Value)->Result<(),String>',
      body: 'let db=crate::platform::database::sqlite_database(app);run_app_config_service(db,|service|service.set_setting(&key,&value))',
    }],
  ]);

  const commandItems = [...commandContracts].map(([name, contract]) =>
    assertRustFunctionContract(commands, name, contract));
  const platformItems = [...platformContracts].map(([name, contract]) =>
    assertRustFunctionContract(platform, name, contract));

  const wrongRouteFixture = rustProductionView(`
    pub fn save_config<R: Runtime>(app: &AppHandle<R>, config: Value) -> Result<(), String> {
      let db = crate::platform::database::sqlite_database(app);
      run_app_config_service(db, |service| service.load_config())
    }
  `);
  assert.throws(
    () => assertRustFunctionContract(
      wrongRouteFixture,
      'save_config',
      platformContracts.get('save_config'),
    ),
    /save_config body changed/u,
  );
  const ignoredArgumentFixture = rustProductionView(`
    #[tauri::command]
    pub fn set_app_setting<R: Runtime>(
      app: AppHandle<R>,
      key: String,
      value: Value,
    ) -> Result<(), String> {
      crate::platform::app_config::set_setting(&app, key)
    }
  `);
  assert.throws(
    () => assertRustFunctionContract(
      ignoredArgumentFixture,
      'set_app_setting',
      commandContracts.get('set_app_setting'),
    ),
    /set_app_setting body changed/u,
  );

  assert.doesNotMatch(
    commandItems.map(({ block }) => block).join('\n'),
    /SqliteConfigStore|AppConfigRepositoryService|SystemClock|UnixMillisClock|normalize_|repair_|serde_json::(?:from|to)/u,
    'target desktop commands must not own persistence, normalization, or clocks',
  );
  assert.doesNotMatch(
    platformItems.map(({ body }) => body).join('\n'),
    /SqliteConfigStore|AppConfigRepositoryService|SystemClock|UnixMillisClock|normalize_|repair_|serde_json::(?:from|to)/u,
    'persistence entry points must leave composition and policy to their delegates',
  );

  assertRustFunctionContract(platform, 'run_app_config_service', {
    attributes: '',
    signature: "fn run_app_config_service<T>(db:Arc<Database>,operation:impl FnOnce(&AppConfigRepositoryService<'_>)->Result<T,String>)->Result<T,String>",
    body: 'let store=SqliteConfigStore::new(db);operation(&AppConfigRepositoryService::new(&store,&SystemClock))',
  });
});

test('SQLite app config store implements the core port', () => {
  const sqliteLib = read('adapters', 'sqlite', 'src', 'lib.rs');
  const sqliteConfigStore = read('adapters', 'sqlite', 'src', 'config_store', 'mod.rs');
  const platformDatabase = read(...desktopCrateSegments, 'src', 'platform', 'database.rs');

  assert.equal(exists('adapters', 'sqlite', 'src', 'config_store.rs'), false);
  for (const fileName of ['mod.rs', 'app_config.rs', 'library.rs', 'settings.rs']) {
    assert.ok(exists('adapters', 'sqlite', 'src', 'config_store', fileName));
  }
  assert.match(sqliteLib, /^pub mod config_store;/mu);
  assert.match(sqliteLib, /^pub use config_store::SqliteConfigStore;/mu);
  assert.match(sqliteConfigStore, /impl<D> AppConfigStore for SqliteConfigStore<D>/u);
  assert.doesNotMatch(platformDatabase, /SqliteConfigStore|sqlite_config_store/u);
});

test('desktop app config composes the core service behind platform commands', () => {
  const platformMod = read(...desktopCrateSegments, 'src', 'platform', 'mod.rs');
  const systemCommand = read(...desktopCrateSegments, 'src', 'commands', 'system.rs');
  const platformAppConfigPath = desktopCratePath('src', 'platform', 'app_config.rs');

  assert.equal(fs.existsSync(platformAppConfigPath), true);
  const platformAppConfig = fs.readFileSync(platformAppConfigPath, 'utf8');

  assert.match(platformMod, /^pub mod app_config;/mu);
  assert.match(platformAppConfig, /crate::platform::database::sqlite_database\(app\)/u);
  assert.match(platformAppConfig, /AppConfigRepositoryService/u);
  assert.match(platformAppConfig, /SqliteConfigStore/u);
  assert.doesNotMatch(platformAppConfig, /rusqlite|serde_json::(?:from|to)/u);
  assert.match(systemCommand, /crate::platform::app_config::load_config\(&app\)/u);
  assert.match(systemCommand, /crate::platform::app_config::save_config\(&app, config\)/u);
  assert.match(systemCommand, /crate::platform::app_config::get_setting\(&app, key\)/u);
  assert.match(systemCommand, /crate::platform::app_config::set_setting\(&app, key, value\)/u);
  assert.doesNotMatch(systemCommand, /sqlite_config_store|SqliteConfigStore/u);
});

test('app config repository policy is shared across hosts', () => {
  const coreConfigRoot = path.join(repoRoot, 'core', 'src', 'config');
  const coreConfig = rustFilesUnder(coreConfigRoot)
    .sort()
    .map((filePath) => rustProductionView(fs.readFileSync(filePath, 'utf8')))
    .join('\n');
  const coreRepository = rustProductionView(read('core', 'src', 'config', 'repository.rs'));
  const coreService = rustProductionView(read('core', 'src', 'config', 'service.rs'));
  const coreModule = rustProductionView(read('core', 'src', 'config', 'mod.rs'));
  const coreTime = rustProductionView(read('core', 'src', 'ports', 'time.rs'));
  const runtimeFs = rustProductionView(read('adapters', 'runtime_fs', 'src', 'lib.rs'));
  const sqliteRoot = path.join(repoRoot, 'adapters', 'sqlite', 'src', 'config_store');
  const sqliteFiles = ['mod.rs', 'app_config.rs', 'library.rs', 'settings.rs'];
  const sqliteModules = new Map(sqliteFiles.map((fileName) => [
    fileName,
    rustProductionView(fs.readFileSync(path.join(sqliteRoot, fileName), 'utf8')),
  ]));
  const sqliteConfig = [...sqliteModules.values()].join('\n');
  const desktopAppConfig = rustProductionView(read(
    ...desktopCrateSegments, 'src', 'platform', 'app_config.rs',
  ));
  const apiServerConfig = rustProductionView(read(
    ...desktopCrateSegments, 'src', 'platform', 'api_server_config.rs',
  ));
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  const uniffiFacade = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'facade.rs'));
  const uniffiBridge = rustProductionView(read(
    'adapters', 'uniffi_bind', 'src', 'app_config_repository_bridge.rs',
  ));
  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliAppConfig = rustProductionView(read('platforms', 'cli', 'src', 'app_config.rs'));
  const androidSample = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'sample-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'sample', 'SonaUniffiSmoke.kt',
  ));
  const androidConsumer = stripKotlinCommentsAndLiterals(read(
    'platforms', 'android', 'sample-consumer', 'consumer-library', 'src', 'main', 'kotlin',
    'com', 'sona', 'uniffi', 'consumer', 'SonaUniffiConsumerSmoke.kt',
  ));

  const rustFilteringFixture = rustProductionView(String.raw`
    // SqliteConfigStore Database::open SystemTime
    /* AppConfigRepositoryService::new(&fake_store, &fake_clock) */
    const URL: &str = "https://example.invalid/app-config";
    #[cfg(test)]
    mod tests { fn save_config() { Database::open("test"); } }
    pub fn production_after_tests() {}
  `);
  assert.doesNotMatch(
    rustFilteringFixture,
    /fake_store|SqliteConfigStore|Database::open|SystemTime|fn save_config/u,
  );
  assert.match(rustFilteringFixture, /https:\/\/example\.invalid\/app-config/u);
  assert.match(rustFilteringFixture, /pub fn production_after_tests\(\)/u);

  const kotlinFilteringFixture = stripKotlinCommentsAndLiterals(String.raw`
    // import uniffi.sona_uniffi_bind.loadAppConfigJson
    /* saveAppConfigJson(appDataDir, configJson) */
    val fakeCall = "loadAppConfigJson(appDataDir)"
    fun realCall(appDataDir: String) = inspectAppConfig(appDataDir)
  `);
  assert.doesNotMatch(kotlinFilteringFixture, /loadAppConfigJson|saveAppConfigJson/u);
  assert.match(kotlinFilteringFixture, /inspectAppConfig\(appDataDir\)/u);

  const cargoFixtureDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-app-config-cargo-'));
  const cargoFixturePath = path.join(cargoFixtureDirectory, 'Cargo.toml');
  let cargoFixtureDependencies;
  try {
    fs.writeFileSync(cargoFixturePath, `
      [dependencies]
      # rusqlite = "should-not-count"
      host-db = { package = "rusqlite", version = "0.32" }

      [dependencies.host-clock]
      package = "web-time"
      version = "1"
    `);
    cargoFixtureDependencies = readCargoDependencyNames(cargoFixturePath, 'dependencies');
  } finally {
    fs.rmSync(cargoFixtureDirectory, { recursive: true, force: true });
  }
  assert.deepEqual(
    cargoFixtureDependencies,
    ['host-clock', 'host-db', 'rusqlite', 'web-time'],
    'Cargo dependency parsing must include renamed packages and ignore comments',
  );

  assert.match(coreModule, /^pub mod repository;/mu);
  assert.match(coreModule, /^pub mod service;/mu);
  assert.match(coreModule, /^pub use repository::\*;/mu);
  assert.match(coreModule, /^pub use service::AppConfigRepositoryService;/mu);
  assert.match(coreRepository, /pub trait AppConfigStore: Send \+ Sync/u);
  for (const operation of [
    'load_state', 'load_base_config_json', 'load_startup_projection',
    'replace_state', 'load_setting_json', 'set_setting_json',
  ]) {
    assert.match(coreRepository, new RegExp(`fn ${operation}\\(`, 'u'));
  }
  assert.match(coreRepository, /pub struct AppConfigStoredState\b/u);
  assert.match(coreRepository, /pub struct AppConfigRepositorySnapshot\b/u);
  assert.match(coreService, /pub struct AppConfigRepositoryService\b/u);
  for (const operation of [
    'load_config', 'inspect_state', 'save_config', 'get_setting', 'set_setting',
    'load_app_config_payload', 'load_serve_startup_settings',
  ]) {
    assert.match(coreService, new RegExp(`pub fn ${operation}\\(`, 'u'));
  }
  for (const policyOwner of [
    'app_config_payload', 'extract_library_config', 'inject_library_config',
    'repair_library_config_ids', 'config_version_from_config',
  ]) {
    assert.match(coreService, new RegExp(`fn ${policyOwner}\\(`, 'u'));
  }
  assert.match(coreService, /self\.clock\.now_ms\(\)/u);
  const payloadLoad = readRustFunctionItem(coreService, 'load_app_config_payload').body;
  const startupLoad = readRustFunctionItem(coreService, 'load_serve_startup_settings').body;
  assert.match(payloadLoad, /self\s*\.store\s*\.load_base_config_json\(\)/u);
  assert.doesNotMatch(payloadLoad, /load_state\(\)/u);
  assert.match(startupLoad, /self\s*\.store\s*\.load_startup_projection\(\)/u);
  assert.doesNotMatch(startupLoad, /load_state\(\)/u);
  assert.match(coreTime, /pub trait UnixMillisClock: Send \+ Sync/u);
  assert.match(runtimeFs, /impl UnixMillisClock for SystemClock/u);
  assert.match(runtimeFs, /SystemTime::now\(\)/u);
  assert.doesNotMatch(
    `${coreConfig}\n${coreTime}`,
    /\b(?:rusqlite|tauri|uniffi|uuid|clap|tokio|reqwest|hyper)::|\b(?:Uuid|SystemTime|UNIX_EPOCH)\b|\bstd\s*::\s*(?:\{[^}]*\b(?:fs|net|path|process|time)\b|(?:fs|net|path|process|time)\b)/u,
    'core app-config policy must not use host capabilities',
  );
  const coreDependencies = readCargoDependencyNames(
    path.join(repoRoot, 'core', 'Cargo.toml'),
    'dependencies',
  );
  const forbiddenCoreDependencies = new Set([
    'async-fs', 'cap-std', 'clap', 'diesel', 'directories', 'duct', 'fs-err',
    'hyper', 'quanta', 'reqwest', 'rusqlite', 'sqlx', 'subprocess', 'tauri',
    'tokio', 'uniffi', 'uuid', 'walkdir', 'web-time',
  ]);
  assert.deepEqual(
    coreDependencies.filter((name) => forbiddenCoreDependencies.has(name.replaceAll('_', '-'))),
    [],
    'core Cargo dependencies must not provide host capabilities',
  );

  assert.equal(exists('adapters', 'sqlite', 'src', 'config_store.rs'), false);
  for (const fileName of sqliteFiles) {
    const lineCount = fs.readFileSync(path.join(sqliteRoot, fileName), 'utf8')
      .split(/\r?\n/u).length;
    assert.ok(lineCount <= 800, `${fileName} exceeds the 800-line adapter limit`);
  }
  const sqliteMod = sqliteModules.get('mod.rs');
  const sqliteLibrary = sqliteModules.get('library.rs');
  assert.match(sqliteMod, /impl<D> AppConfigStore for SqliteConfigStore<D>/u);
  assert.match(sqliteMod, /let tx = conn\.unchecked_transaction\(\)/u);
  assert.match(sqliteMod, /library::load\(&tx\)/u);
  assert.match(sqliteMod, /tx\.commit\(\)/u);
  assert.match(sqliteMod, /with_connection\(app_config::load_base_config_json\)/u);
  assert.match(sqliteMod, /with_connection\(app_config::load_startup_projection\)/u);
  assert.match(sqliteMod, /fn replace_state[\s\S]*?with_rw_transaction/u);
  assert.match(sqliteMod, /fn set_setting_json[\s\S]*?with_rw_transaction/u);
  for (const table of [
    'summary_templates', 'polish_presets', 'vocabulary_sets',
    'vocabulary_rules', 'speaker_profiles', 'speaker_profile_samples',
  ]) {
    assert.match(sqliteLibrary, new RegExp(table, 'u'));
  }
  assert.doesNotMatch(
    sqliteConfig,
    /HashSet|summaryCustomTemplates|repair_library|hash_string|SystemTime|UNIX_EPOCH|serde_json::json|\bUuid\b|uuid::/u,
    'SQLite adapter must map typed state without owning config policy',
  );

  for (const host of [desktopAppConfig, apiServerConfig, uniffiBridge, cliAppConfig]) {
    assert.match(host, /AppConfigRepositoryService::new\(/u);
    assert.match(host, /SqliteConfigStore::new\(/u);
    assert.match(host, /SystemClock/u);
  }
  assert.match(desktopAppConfig, /crate::platform::database::sqlite_database\(app\)/u);
  assert.match(apiServerConfig, /Database::global_arc\(\)/u);
  assert.match(apiServerConfig, /database\.is_for_app_local_data_dir\(app_local_data_dir\)/u);
  assert.match(apiServerConfig, /service\.load_app_config_payload\(\)/u);
  assert.match(apiServerConfig, /service\.load_serve_startup_settings\(\)/u);

  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  for (const [dependency, expectedSpec] of [
    ['sona-core', '{ path = "../../core" }'],
    ['sona-runtime-fs', '{ path = "../runtime_fs" }'],
    ['sona-sqlite', '{ path = "../sqlite" }'],
  ]) {
    assert.equal(
      readCargoDependencySpec(uniffiCargoPath, 'dependencies', dependency),
      expectedSpec,
    );
  }
  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');
  const currentExports = [
    ...uniffiLib.matchAll(/#\[uniffi::export\]\s*pub fn ([a-z0-9_]+)\s*\(/gu),
  ].map((match) => match[1]);
  assert.deepEqual(currentExports, expectedUniffiExports);
  const firstConfigExport = currentExports.indexOf('load_app_config_json');
  assert.deepEqual(currentExports.slice(firstConfigExport - 1, firstConfigExport + 4), [
    'resolve_effective_config_json',
    'load_app_config_json',
    'save_app_config_json',
    'get_app_setting_json',
    'set_app_setting_json',
  ]);
  const bindingErrorBody = /pub enum SonaCoreBindingError\s*\{([\s\S]*?)\n\}/u
    .exec(uniffiLib)?.[1] ?? assert.fail('missing SonaCoreBindingError');
  const bindingErrors = [
    ...bindingErrorBody.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*\{/gmu),
  ].map((match) => match[1]);
  assert.deepEqual(bindingErrors, expectedUniffiErrorVariants);
  for (const exportName of [
    'load_app_config_json', 'save_app_config_json',
    'get_app_setting_json', 'set_app_setting_json',
  ]) {
    assert.match(uniffiLib, new RegExp(`pub fn ${exportName}\\(`, 'u'));
    assert.match(uniffiLib, new RegExp(`SonaCoreFacade::${exportName}\\(`, 'u'));
    assert.match(uniffiFacade, new RegExp(`pub fn ${exportName}\\(`, 'u'));
    assert.match(uniffiBridge, new RegExp(`pub\\(crate\\) fn ${exportName}\\(`, 'u'));
    assert.doesNotMatch(uniffiLib, new RegExp(`pub async fn ${exportName}\\(`, 'u'));
  }

  assert.match(androidSample, /^\s*import\s+uniffi\.sona_uniffi_bind\.loadAppConfigJson\s*$/mu);
  assert.match(androidSample, /^\s*import\s+uniffi\.sona_uniffi_bind\.saveAppConfigJson\s*$/mu);
  assert.match(androidSample, /fun\s+loadAppConfig\([^)]*\)\s*:\s*String\?\s*=\s*loadAppConfigJson\(appDataDir\)/u);
  assert.match(androidSample, /fun\s+saveAppConfig\([^)]*\)[\s\S]*?saveAppConfigJson\(appDataDir,\s*configJson\)/u);
  assert.match(androidConsumer, /^\s*import\s+uniffi\.sona_uniffi_bind\.loadAppConfigJson\s*$/mu);
  assert.match(androidConsumer, /fun\s+loadAppConfig\([^)]*\)\s*:\s*String\?\s*=\s*loadAppConfigJson\(appDataDir\)/u);

  assert.match(cliLib, /^mod app_config;/mu);
  assert.match(cliLib, /Commands::AppConfig\(args\) => app_config::run_app_config\(args\)/u);
  assert.match(cliAppConfig, /Database::open_read_only\(&args\.app_data_dir\)/u);
  assert.match(cliAppConfig, /\.inspect_state\(\)/u);
  assert.match(
    cliAppConfig,
    /enum AppConfigCommands\s*\{\s*Show\(AppConfigShowArgs\),?\s*\}/u,
  );
  const cliPersistencePattern = /Database\s*::\s*open\s*\(|\b(?:save(?:_[a-z0-9]+)*|replace(?:_[a-z0-9]+)*|set_setting(?:_[a-z0-9]+)*|with_(?:write|rw_)?transaction|execute(?:_batch)?)\s*\(/u;
  for (const forbiddenCall of [
    'Database::open(&args.app_data_dir)',
    'service.save_config(&config)',
    'store.replace_state(state)',
    'service.set_setting("key", &value)',
    'database.with_rw_transaction(|tx| Ok(()))',
    'connection.execute("DELETE FROM app_config", [])',
  ]) {
    assert.match(forbiddenCall, cliPersistencePattern);
  }
  assert.doesNotMatch('Database::open_read_only(&args.app_data_dir)', cliPersistencePattern);
  assert.doesNotMatch(cliAppConfig, cliPersistencePattern);
  assert.doesNotMatch(
    cliAppConfig,
    /\.(?:load_config|get_setting|load_app_config_payload|load_serve_startup_settings)\s*\(/u,
    'CLI inspection must call only inspect_state',
  );

  const architectureLineCount = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'app-config-architecture.test.js'),
    'utf8',
  ).split(/\r?\n/u).length;
  const architectureTestCount = [
    ...fs.readFileSync(
      path.join(repoRoot, 'scripts', 'app-config-architecture.test.js'),
      'utf8',
    ).matchAll(/^test\(/gmu),
  ].length;
  const repositoryHelperLineCount = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'test-support', 'repository.js'),
    'utf8',
  ).split(/\r?\n/u).length;
  assert.ok(architectureLineCount <= 2000);
  assert.ok(architectureTestCount <= 60);
  assert.ok(repositoryHelperLineCount < 1000);
});
