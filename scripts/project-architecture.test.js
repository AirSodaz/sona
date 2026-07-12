import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  read,
  readCargoDependencyNames,
  readCargoDependencySpec,
  readCargoStringArray,
  repoRoot,
  assertCargoDependencyVersionAndFeature,
  expectedUniffiExports,
  stripKotlinCommentsAndLiterals,
} from './test-support/repository.js';

function stripRustCommentsPreservingLiterals(source) {
  const stripped = source.split('');
  const blank = (index) => {
    if (stripped[index] !== '\n') stripped[index] = ' ';
  };

  for (let index = 0; index < source.length;) {
    const rawString = /^(?:br|rb|r)(#*)"/u.exec(source.slice(index));
    if (rawString) {
      const closing = `"${rawString[1]}`;
      const end = source.indexOf(closing, index + rawString[0].length);
      index = end === -1 ? source.length : end + closing.length;
      continue;
    }
    const quoteOffset = source[index] === 'b' && source[index + 1] === '"' ? 1 : 0;
    if (source[index + quoteOffset] === '"') {
      index += quoteOffset + 1;
      while (index < source.length) {
        if (source[index] === '\\') index += 2;
        else if (source[index++] === '"') break;
      }
      continue;
    }
    const character = /^(?:b)?'(?:\\.|[^'\\\n])'/u.exec(source.slice(index));
    if (character) {
      index += character[0].length;
      continue;
    }
    if (source.startsWith('//', index)) {
      while (index < source.length && source[index] !== '\n') blank(index++);
      continue;
    }
    if (source.startsWith('/*', index)) {
      let depth = 0;
      while (index < source.length) {
        if (source.startsWith('/*', index)) {
          blank(index++);
          blank(index++);
          depth += 1;
        } else if (source.startsWith('*/', index)) {
          blank(index++);
          blank(index++);
          depth -= 1;
          if (depth === 0) break;
        } else {
          blank(index++);
        }
      }
      continue;
    }
    index += 1;
  }
  return stripped.join('');
}

function maskRustLiterals(source) {
  const masked = source.split('');
  const maskRange = (start, end) => {
    for (let index = start; index < end; index += 1) {
      if (masked[index] !== '\n') masked[index] = ' ';
    }
  };

  for (let index = 0; index < source.length;) {
    const rawString = /^(?:br|rb|r)(#*)"/u.exec(source.slice(index));
    if (rawString) {
      const closing = `"${rawString[1]}`;
      const end = source.indexOf(closing, index + rawString[0].length);
      const literalEnd = end === -1 ? source.length : end + closing.length;
      maskRange(index, literalEnd);
      index = literalEnd;
      continue;
    }

    const quoteOffset = source[index] === 'b' && source[index + 1] === '"' ? 1 : 0;
    if (source[index + quoteOffset] === '"') {
      let end = index + quoteOffset + 1;
      while (end < source.length) {
        if (source[end] === '\\') end += 2;
        else if (source[end++] === '"') break;
      }
      maskRange(index, end);
      index = end;
      continue;
    }

    const character = /^(?:b)?'(?:\\.|[^'\\\n])'/u.exec(source.slice(index));
    if (character) {
      maskRange(index, index + character[0].length);
      index += character[0].length;
      continue;
    }
    index += 1;
  }
  return masked.join('');
}

function rustCfgMemberContext(structural, markerIndex) {
  const delimiters = [];
  for (let index = 0; index < markerIndex; index += 1) {
    if (structural[index] === '{' || structural[index] === '(') {
      delimiters.push({ character: structural[index], index });
    } else if (structural[index] === '}' || structural[index] === ')') {
      delimiters.pop();
    }
  }
  const owner = delimiters.at(-1);
  if (owner?.character === '(') {
    return { commaTerminated: true, parenthesisOwner: true };
  }
  if (owner?.character !== '{') {
    return { commaTerminated: false, parenthesisOwner: false };
  }

  const previousBoundary = Math.max(
    structural.lastIndexOf('{', owner.index - 1),
    structural.lastIndexOf('}', owner.index - 1),
    structural.lastIndexOf(';', owner.index - 1),
  );
  return {
    commaTerminated: /\b(?:struct|enum)\b/u.test(
      structural.slice(previousBoundary + 1, owner.index),
    ),
    parenthesisOwner: false,
  };
}

function rustProductionView(source) {
  let production = stripRustCommentsPreservingLiterals(source);
  const structural = maskRustLiterals(production);
  const removals = [];

  for (const marker of structural.matchAll(/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/gu)) {
    const { commaTerminated, parenthesisOwner } = rustCfgMemberContext(
      structural,
      marker.index,
    );
    let parentheses = 0;
    let brackets = 0;
    let angles = 0;
    let memberBraces = 0;
    let memberInitializer = false;
    let itemEnd = structural.length;

    for (let index = marker.index + marker[0].length; index < structural.length; index += 1) {
      const character = structural[index];
      if (character === '(') parentheses += 1;
      else if (character === ')' && parenthesisOwner && parentheses === 0) {
        itemEnd = index;
        break;
      } else if (character === ')') parentheses -= 1;
      else if (character === '[') brackets += 1;
      else if (character === ']') brackets -= 1;
      else if (commaTerminated && angles > 0 && character === '{') memberBraces += 1;
      else if (commaTerminated && memberBraces > 0 && character === '}') memberBraces -= 1;
      else if (
        commaTerminated
        && parentheses === 0
        && brackets === 0
        && angles === 0
        && character === '='
      ) {
        memberInitializer = true;
      } else if (
        commaTerminated
        && character === '<'
        && memberBraces === 0
        && (
          !memberInitializer
          || angles > 0
          || structural.slice(marker.index, index).trimEnd().endsWith('::')
        )
      ) angles += 1;
      else if (
        commaTerminated
        && memberBraces === 0
        && character === '>'
        && angles > 0
      ) angles -= 1;
      else if (
        commaTerminated
        && parentheses === 0
        && brackets === 0
        && angles === 0
        && memberBraces === 0
        && character === ','
      ) {
        itemEnd = index + 1;
        break;
      } else if (parentheses === 0 && brackets === 0 && angles === 0 && character === ';') {
        itemEnd = index + 1;
        break;
      } else if (parentheses === 0 && brackets === 0 && angles === 0 && character === '{') {
        let braces = 1;
        for (index += 1; index < structural.length && braces > 0; index += 1) {
          if (structural[index] === '{') braces += 1;
          else if (structural[index] === '}') braces -= 1;
        }
        while (/\s/u.test(structural[index] ?? '')) index += 1;
        if (structural[index] === ',') index += 1;
        itemEnd = index;
        break;
      }
    }
    removals.push([marker.index, itemEnd]);
  }

  for (const [start, end] of removals.reverse()) {
    const whitespace = production.slice(start, end).replace(/[^\n]/gu, ' ');
    production = `${production.slice(0, start)}${whitespace}${production.slice(end)}`;
  }
  return production;
}

const coreSourcePolicies = [
  [
    /\bstd\s*::\s*(?:\{[^}]*\b(?:fs|net|path|process|time)\b|(?:fs|net|path|process|time)\b)/u,
    'standard-library host capability',
  ],
  [
    /\b(?:rusqlite|tauri|uniffi|tokio|clap|uuid|reqwest|hyper|ureq|tonic|surf|isahc|async_fs|cap_std|fs_err|fs_extra|walkdir|directories|duct|subprocess|web_time|quanta)::/u,
    'third-party host capability',
  ],
  [/\b(?:Uuid|SystemTime|UNIX_EPOCH)\b/u, 'ambient ID or wall clock'],
];

function assertCoreSourcePurity(source) {
  for (const [pattern, label] of coreSourcePolicies) {
    assert.doesNotMatch(source, pattern, `core must not use ${label}`);
  }
}

const coreDependencyPolicies = [
  [new Set(['async-fs', 'cap-std', 'fs-err', 'fs-extra', 'walkdir', 'directories']), 'filesystem'],
  [new Set(['reqwest', 'hyper', 'ureq', 'tonic', 'surf', 'isahc', 'awc']), 'network'],
  [new Set(['duct', 'subprocess', 'command-group', 'shared-child']), 'process'],
  [new Set(['web-time', 'quanta', 'coarsetime']), 'wall clock'],
  [new Set(['uuid', 'ulid']), 'ambient ID'],
  [
    new Set(['rusqlite', 'sqlx', 'diesel', 'tauri', 'tauri-build', 'uniffi', 'clap', 'tokio']),
    'host runtime',
  ],
];

function coreDependencyViolations(dependencyNames) {
  return dependencyNames.flatMap((dependencyName) => {
    const normalizedName = dependencyName.replaceAll('_', '-');
    return coreDependencyPolicies
      .filter(([names]) => names.has(normalizedName))
      .map(([, label]) => `${dependencyName} (${label})`);
  });
}

function assertCoreDependencyPurity(dependencyNames) {
  assert.deepEqual(
    coreDependencyViolations(dependencyNames),
    [],
    'core dependencies must not provide host capabilities',
  );
}

const cliPersistencePattern = /\b(?:with_write_connection|with_rw_transaction|with_transaction|execute_batch|execute|insert_project|update_project|delete_project|replace_projects(?:_json)?|reorder_projects|set_active_project(?:_id|_setting_json))\s*\(/u;

test('project repository policy is shared across hosts', () => {
  const coreRepository = rustProductionView(read('core', 'src', 'project', 'repository.rs'));
  const coreService = rustProductionView(read('core', 'src', 'project', 'service.rs'));
  const coreModule = rustProductionView(read('core', 'src', 'project', 'mod.rs'));
  const coreTime = rustProductionView(read('core', 'src', 'ports', 'time.rs'));
  const coreProduction = `${coreRepository}\n${coreService}\n${coreModule}\n${coreTime}`;
  const sqliteProject = rustProductionView(read('adapters', 'sqlite', 'src', 'project.rs'));
  const runtimeFs = rustProductionView(read('adapters', 'runtime_fs', 'src', 'lib.rs'));
  const desktopProject = rustProductionView(
    read('platforms', 'desktop', 'src', 'platform', 'project_repository.rs'),
  );
  const uniffiProject = rustProductionView(
    read('adapters', 'uniffi_bind', 'src', 'project_bridge.rs'),
  );
  const uniffiLib = rustProductionView(read('adapters', 'uniffi_bind', 'src', 'lib.rs'));
  const cliLib = rustProductionView(read('platforms', 'cli', 'src', 'lib.rs'));
  const cliProjects = rustProductionView(read('platforms', 'cli', 'src', 'projects.rs'));
  const androidSample = stripKotlinCommentsAndLiterals(read(
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
  ));
  const androidConsumer = stripKotlinCommentsAndLiterals(read(
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
  ));

  const rustFilteringFixture = rustProductionView(`
    // pub trait ProjectStore
    /* ProjectRepositoryService::new */
    // #[cfg(test)]
    const CFG_TEST_TEXT: &str = "#[cfg(test)]";
    const URL_TEXT: &str = "https://example.invalid";
    const COMMENT_TEXT: &str = "//";
    #[cfg(test)]
    mod tests {
      fn replace_projects() {
        let nested = "}";
      }
    }
    pub struct ProductionAfterTests;
  `);
  const kotlinFilteringFixture = stripKotlinCommentsAndLiterals(`
    // import uniffi.sona_uniffi_bind.loadProjectRepositoryStateJson
    /* createProjectJson(appDataDir, inputJson) */
    val callShapedLiteral = "loadProjectRepositoryStateJson(appDataDir)"
  `);
  const rustCfgMemberFixture = rustProductionView(`
    struct FixtureStruct {
      #[cfg(test)]
      test_only: String,
      production: String,
    }
    enum FixtureEnum {
      #[cfg(test)]
      TestOnly,
      Production,
    }
    use std::fs;
  `);
  const rustCfgComparisonFixture = rustProductionView(`
    #[cfg(test)]
    const TEST_FLAG: bool = 1 < 2;
    use std::fs;
  `);
  const rustCfgDiscriminantFixture = rustProductionView(`
    enum ShiftedFixture {
      #[cfg(test)]
      TestOnly = 1 << 2,
      Production,
    }
    use std::fs;
  `);
  const rustCfgConstGenericFixture = rustProductionView(`
    struct ConstGenericFixture {
      #[cfg(test)]
      test_only: Foo<{ 1 << 2 }>,
      production: String,
    }
    use std::fs;
  `);
  const rustCfgTupleMemberFixture = rustProductionView(`
    struct TupleStruct(
      #[cfg(test)]
      TupleStructTestOnly,
      TupleStructProduction,
    );
    enum TupleEnum {
      Variant(
        #[cfg(test)]
        TupleEnumTestOnly,
        TupleEnumProduction,
      ),
    }
    use std::fs;
  `);
  assert.match(rustFilteringFixture, /const CFG_TEST_TEXT: &str = "#\[cfg\(test\)\]";/u);
  assert.match(rustFilteringFixture, /const URL_TEXT: &str = "https:\/\/example\.invalid";/u);
  assert.match(rustFilteringFixture, /const COMMENT_TEXT: &str = "\/\/";/u);
  assert.doesNotMatch(rustFilteringFixture, /fn replace_projects/u);
  assert.match(rustFilteringFixture, /pub struct ProductionAfterTests;/u);
  assert.doesNotMatch(rustCfgMemberFixture, /test_only|TestOnly/u);
  assert.match(rustCfgMemberFixture, /production: String/u);
  assert.match(rustCfgMemberFixture, /Production,/u);
  assert.match(rustCfgMemberFixture, /use std::fs/u);
  assert.doesNotMatch(rustCfgComparisonFixture, /TEST_FLAG/u);
  assert.match(rustCfgComparisonFixture, /use std::fs/u);
  assert.doesNotMatch(rustCfgDiscriminantFixture, /TestOnly/u);
  assert.match(rustCfgDiscriminantFixture, /Production,/u);
  assert.match(rustCfgDiscriminantFixture, /use std::fs/u);
  assert.doesNotMatch(rustCfgConstGenericFixture, /test_only/u);
  assert.match(rustCfgConstGenericFixture, /production: String/u);
  assert.match(rustCfgConstGenericFixture, /use std::fs/u);
  assert.doesNotMatch(
    rustCfgTupleMemberFixture,
    /TupleStructTestOnly|TupleEnumTestOnly/u,
  );
  assert.match(rustCfgTupleMemberFixture, /TupleStructProduction/u);
  assert.match(rustCfgTupleMemberFixture, /TupleEnumProduction/u);
  assert.match(rustCfgTupleMemberFixture, /use std::fs/u);
  assert.doesNotMatch(
    kotlinFilteringFixture,
    /loadProjectRepositoryStateJson|createProjectJson/u,
  );
  for (const forbiddenSource of [
    'use std::{fs, net, path, process, time};',
    'rusqlite::Connection::open("projects.db")',
    'uuid::Uuid::new_v4()',
  ]) {
    assert.ok(
      coreSourcePolicies.some(([pattern]) => pattern.test(forbiddenSource)),
      `missing core purity evidence for: ${forbiddenSource}`,
    );
  }
  const cargoFixtureDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-core-purity-'));
  const cargoFixturePath = path.join(cargoFixtureDirectory, 'Cargo.toml');
  let renamedDependencyNames;
  try {
    fs.writeFileSync(cargoFixturePath, `
      [dependencies]
      direct-host-db = { package = 'diesel', version = '2' } # direct dependency

      [dependencies.fs-err]
      version = "3"

      [dependencies.host-db] # dependency section
      package = 'rusqlite'
      version = '0.32'

      [dependencies.table-host-db]
      package = "sqlx" # database backend
      version = "0.32"
    `);
    renamedDependencyNames = readCargoDependencyNames(cargoFixturePath, 'dependencies');
  } finally {
    fs.rmSync(cargoFixtureDirectory, { recursive: true, force: true });
  }
  assert.deepEqual(
    renamedDependencyNames,
    [
      'diesel',
      'direct-host-db',
      'fs-err',
      'host-db',
      'rusqlite',
      'sqlx',
      'table-host-db',
    ],
  );
  for (const forbiddenDependencies of [
    renamedDependencyNames,
    ['fs-err'],
    ['reqwest'],
    ['duct'],
    ['web-time'],
    ['uuid'],
    ['tauri'],
  ]) {
    assert.notDeepEqual(
      coreDependencyViolations(forbiddenDependencies),
      [],
      `missing dependency purity evidence for: ${forbiddenDependencies.join(', ')}`,
    );
  }
  for (const forbiddenCliCall of [
    'database.with_write_connection(|connection| Ok(()))',
    'database.with_rw_transaction(|transaction| Ok(()))',
    'connection.execute("DELETE FROM projects", [])',
    'connection.execute_batch("DELETE FROM projects")',
    'service.with_transaction(|transaction| Ok(()))',
    'store.insert_project(project)',
    'store.update_project(project)',
    'store.delete_project(project_id)',
    'store.replace_projects(projects)',
    'store.replace_projects_json(input)',
    'store.reorder_projects(ids)',
    'store.set_active_project_id(project_id)',
    'store.set_active_project_setting_json(project_id)',
  ]) {
    assert.match(forbiddenCliCall, cliPersistencePattern);
  }
  assert.doesNotMatch('row.updated_at.to_string()', cliPersistencePattern);

  assert.match(coreRepository, /pub trait ProjectStore\b/u);
  for (const operation of [
    'load_state',
    'insert_project',
    'update_project',
    'delete_project',
    'replace_projects',
    'reorder_projects',
    'set_active_project_setting_json',
  ]) {
    assert.match(coreRepository, new RegExp(`fn ${operation}\\(`, 'u'));
  }
  assert.match(coreService, /pub trait ProjectIdGenerator\b/u);
  assert.match(coreTime, /pub trait UnixMillisClock\b/u);
  assert.match(coreService, /pub struct ProjectRepositoryService\b/u);
  for (const operation of [
    'load_state',
    'replace_projects_json',
    'create_project',
    'update_project_json',
    'delete_project',
    'reorder_projects',
    'get_active_project_id',
    'set_active_project_id',
  ]) {
    assert.match(coreService, new RegExp(`pub fn ${operation}\\(`, 'u'));
  }
  assert.match(coreService, /self\.ids\.generate_id\(\)/u);
  assert.match(coreService, /self\.clock\.now_ms\(\)/u);
  for (const storeOperation of [
    'load_state',
    'replace_projects',
    'insert_project',
    'update_project',
    'delete_project',
    'reorder_projects',
    'set_active_project_setting_json',
  ]) {
    assert.match(coreService, new RegExp(`self\\.store\\.${storeOperation}\\(`, 'u'));
  }
  assert.match(coreService, /fn parse_project_patch\(/u);
  assert.match(coreService, /fn normalize_replacement_project\(/u);
  assert.match(coreService, /fn snapshot_from_state\(/u);
  assert.match(coreModule, /pub use repository::\{[\s\S]*ProjectStore/u);
  assert.match(coreModule, /pub use service::\{ProjectIdGenerator, ProjectRepositoryService\}/u);
  assertCoreSourcePurity(coreProduction);
  assertCoreDependencyPurity(
    readCargoDependencyNames(path.join(repoRoot, 'core', 'Cargo.toml'), 'dependencies'),
  );

  assert.match(sqliteProject, /impl<D> ProjectStore for SqliteProjectRepository<D>/u);
  for (const operation of [
    'load_state',
    'insert_project',
    'update_project',
    'delete_project',
    'replace_projects',
    'reorder_projects',
    'set_active_project_setting_json',
  ]) {
    assert.match(sqliteProject, new RegExp(`fn ${operation}\\(`, 'u'));
  }
  assert.match(sqliteProject, /with_read_connection/u);
  assert.match(sqliteProject, /with_transaction/u);
  assert.doesNotMatch(
    sqliteProject,
    /serde_json|\bUuid\b|\buuid::|\bUnixMillisClock\b|\bProjectIdGenerator\b|\bSystemTime\b|\bUNIX_EPOCH\b|\.now_ms\(|\.generate_id\(/u,
  );

  assert.match(runtimeFs, /impl ProjectIdGenerator for UuidGenerator/u);
  assert.match(runtimeFs, /impl UnixMillisClock for SystemClock/u);
  assert.match(runtimeFs, /Uuid::new_v4\(\)/u);
  assert.match(runtimeFs, /SystemTime::now\(\)/u);

  for (const [hostName, hostSource] of [
    ['desktop', desktopProject],
    ['UniFFI', uniffiProject],
    ['CLI', cliProjects],
  ]) {
    assert.match(
      hostSource,
      /ProjectRepositoryService::new\(/u,
      `${hostName} must instantiate ProjectRepositoryService`,
    );
    assert.match(
      hostSource,
      /SqliteProjectRepository::new\(/u,
      `${hostName} must instantiate SqliteProjectRepository`,
    );
  }

  const workspaceCargoPath = path.join(repoRoot, 'Cargo.toml');
  const uniffiCargoPath = path.join(repoRoot, 'adapters', 'uniffi_bind', 'Cargo.toml');
  const cliCargoPath = path.join(repoRoot, 'platforms', 'cli', 'Cargo.toml');
  assert.ok(
    readCargoStringArray(workspaceCargoPath, 'workspace', 'members').includes('platforms/cli'),
  );
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
  for (const [dependency, expectedSpec] of [
    ['sona-core', '{ path = "../../core" }'],
    ['sona-runtime-fs', '{ path = "../../adapters/runtime_fs" }'],
    ['sona-sqlite', '{ path = "../../adapters/sqlite" }'],
  ]) {
    assert.equal(
      readCargoDependencySpec(cliCargoPath, 'dependencies', dependency),
      expectedSpec,
    );
  }
  assertCargoDependencyVersionAndFeature(uniffiCargoPath, 'uniffi', '0.32', 'tokio');

  const currentExports = [
    ...uniffiLib.matchAll(/#\[uniffi::export\]\s*pub (?:async )?fn ([a-z0-9_]+)\s*\(/gu),
  ].map((match) => match[1]);
  assert.deepEqual(currentExports, expectedUniffiExports);

  assert.match(cliLib, /^mod projects;/mu);
  assert.match(cliLib, /Commands::Projects\(args\) => projects::run_projects\(args\)/u);
  assert.match(cliProjects, /Database::open_read_only\(/u);
  assert.match(cliProjects, /\.load_state\(\)/u);
  assert.match(
    cliProjects,
    /enum ProjectsCommands\s*\{\s*List\(ProjectsListArgs\),?\s*\}/u,
  );
  assert.doesNotMatch(cliProjects, /Database::open\(/u);
  assert.doesNotMatch(
    cliProjects,
    cliPersistencePattern,
  );

  assert.match(
    androidSample,
    /^\s*import\s+uniffi\.sona_uniffi_bind\.loadProjectRepositoryStateJson\s*$/mu,
  );
  assert.match(
    androidSample,
    /^\s*import\s+uniffi\.sona_uniffi_bind\.createProjectJson\s*$/mu,
  );
  assert.match(
    androidSample,
    /fun\s+loadProjects\([^)]*\)\s*:\s*String\s*=\s*loadProjectRepositoryStateJson\(/u,
  );
  assert.match(
    androidSample,
    /fun\s+createProject\([^)]*\)\s*:\s*String\s*=\s*createProjectJson\(/u,
  );
  assert.match(
    androidConsumer,
    /^\s*import\s+uniffi\.sona_uniffi_bind\.loadProjectRepositoryStateJson\s*$/mu,
  );
  assert.match(
    androidConsumer,
    /fun\s+loadProjects\([^)]*\)\s*:\s*String\s*=\s*loadProjectRepositoryStateJson\(/u,
  );

});
