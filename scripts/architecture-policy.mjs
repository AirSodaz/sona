import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export const EXPECTED_ROLES = new Map([
  ['sona-core', 'core'],
  ['sona-sync', 'application'],
  ['sona-api-server', 'inbound-adapter'],
  ['sona-ts-bind', 'inbound-adapter'],
  ['sona-archive', 'outbound-adapter'],
  ['sona-export', 'outbound-adapter'],
  ['sona-local-asr', 'outbound-adapter'],
  ['sona-media-detector', 'outbound-adapter'],
  ['sona-model-downloads', 'outbound-adapter'],
  ['sona-online-asr', 'outbound-adapter'],
  ['sona-online-llm', 'outbound-adapter'],
  ['sona-recovery-fs', 'outbound-adapter'],
  ['sona-runtime-fs', 'outbound-adapter'],
  ['sona-sqlite', 'outbound-adapter'],
  ['sona-sync-webdav', 'outbound-adapter'],
  ['sona', 'host'],
  ['sona-cli', 'host'],
  ['sona-uniffi-bind', 'host'],
  ['sona-uniffi-bindgen', 'tool'],
]);

export const ALLOWED_TARGET_ROLES = new Map([
  ['core', new Set()],
  ['application', new Set(['core'])],
  ['inbound-adapter', new Set(['core', 'application'])],
  ['outbound-adapter', new Set(['core', 'application'])],
  [
    'host',
    new Set(['core', 'application', 'inbound-adapter', 'outbound-adapter']),
  ],
  ['tool', new Set()],
]);

export const REVIEWED_OUTBOUND_ADAPTER_EDGES = new Map([
  [
    'sona-model-downloads->sona-runtime-fs',
    'model installation completeness currently reuses the runtime filesystem probe',
  ],
  [
    'sona-recovery-fs->sona-runtime-fs',
    'the recovery adapter currently composes the shared real filesystem, path-status, clock, and atomic JSON helpers',
  ],
]);

export const CURRENT_PUBLIC_STRING_ERROR_DEBT = new Map();

function read(...parts) {
  return fs.readFileSync(path.join(repoRoot, ...parts), 'utf8');
}

function workspaceMemberPaths() {
  const workspace = read('Cargo.toml');
  const members = /\[workspace\][\s\S]*?\bmembers\s*=\s*\[([\s\S]*?)\]/u
    .exec(workspace)?.[1];
  assert.ok(members, 'root Cargo.toml must declare workspace members');
  return Array.from(members.matchAll(/"([^"]+)"/gu), (match) => match[1]);
}

function parseManifest(memberPath) {
  const source = read(...memberPath.split('/'), 'Cargo.toml');
  let section = '';
  let packageName;
  const roles = [];
  const runtimeDependencies = [];

  for (const rawLine of source.split(/\r?\n/u)) {
    const header = /^\s*\[([^\]]+)\]\s*$/u.exec(rawLine);
    if (header) {
      section = header[1];
      continue;
    }

    if (section === 'package') {
      const name = /^\s*name\s*=\s*"([^"]+)"/u.exec(rawLine)?.[1];
      if (name) {
        packageName = name;
      }
      continue;
    }

    if (section === 'package.metadata.sona') {
      const role = /^\s*role\s*=\s*"([^"]+)"/u.exec(rawLine)?.[1];
      if (role) {
        roles.push(role);
      }
      continue;
    }

    if (
      section === 'dependencies'
      || /^target\..+\.dependencies$/u.test(section)
    ) {
      const dependency = /^\s*([A-Za-z0-9_-]+)\s*=\s*(.+)$/u.exec(rawLine);
      if (!dependency) {
        continue;
      }
      const renamedPackage = /\bpackage\s*=\s*"([^"]+)"/u.exec(dependency[2])?.[1];
      runtimeDependencies.push(renamedPackage ?? dependency[1]);
    }
  }

  assert.ok(packageName, `${memberPath}/Cargo.toml must declare [package].name`);
  return {
    memberPath,
    packageName,
    role: roles[0],
    roles,
    runtimeDependencies,
  };
}

export function workspacePackages() {
  const packages = workspaceMemberPaths().map(parseManifest);
  const names = packages.map(({ packageName }) => packageName);
  assert.equal(new Set(names).size, names.length, 'workspace package names must be unique');
  return packages;
}

function rustSourceFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      return rustSourceFiles(entryPath);
    }
    return entry.isFile() && entry.name.endsWith('.rs') ? [entryPath] : [];
  });
}

function moduleDirectory(filePath) {
  const basename = path.basename(filePath, '.rs');
  return ['lib', 'main', 'mod'].includes(basename)
    ? path.dirname(filePath)
    : path.join(path.dirname(filePath), basename);
}

function externalCfgTestModuleFiles(sourceDirectory) {
  const excluded = new Set();
  const externalTestModule = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:#\[[^\]]+\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_]\w*)\s*;/gu;

  for (const filePath of rustSourceFiles(sourceDirectory)) {
    const source = fs.readFileSync(filePath, 'utf8');
    for (const match of source.matchAll(externalTestModule)) {
      const moduleRoot = path.join(moduleDirectory(filePath), match[1]);
      const moduleFile = `${moduleRoot}.rs`;
      const moduleDirectoryFile = path.join(moduleRoot, 'mod.rs');
      if (fs.existsSync(moduleFile)) {
        excluded.add(moduleFile);
      }
      if (fs.existsSync(moduleDirectoryFile)) {
        excluded.add(moduleDirectoryFile);
      }
      if (fs.existsSync(moduleRoot) && fs.statSync(moduleRoot).isDirectory()) {
        for (const nestedFile of rustSourceFiles(moduleRoot)) {
          excluded.add(nestedFile);
        }
      }
    }
  }

  return excluded;
}

function productionRustSourceFiles(sourceDirectory) {
  const excluded = externalCfgTestModuleFiles(sourceDirectory);
  return rustSourceFiles(sourceDirectory).filter((filePath) => !excluded.has(filePath));
}

function blockEnd(source, openingBrace) {
  let depth = 0;
  for (let index = openingBrace; index < source.length; index += 1) {
    if (source.startsWith('//', index)) {
      index = source.indexOf('\n', index + 2);
      if (index < 0) {
        return source.length;
      }
      continue;
    }
    if (source.startsWith('/*', index)) {
      let commentDepth = 1;
      index += 1;
      while (commentDepth > 0 && index + 1 < source.length) {
        index += 1;
        if (source.startsWith('/*', index)) {
          commentDepth += 1;
          index += 1;
        } else if (source.startsWith('*/', index)) {
          commentDepth -= 1;
          index += 1;
        }
      }
      continue;
    }
    const rawString = /^(?:br|r)(#+)?"/u.exec(source.slice(index));
    if (rawString) {
      const delimiter = `"${rawString[1] ?? ''}`;
      const end = source.indexOf(delimiter, index + rawString[0].length);
      if (end < 0) {
        throw new Error('unclosed Rust raw string while scanning public signatures');
      }
      index = end + delimiter.length - 1;
      continue;
    }
    const charLiteral = /^'(?:\\.|[^\\'])'/u.exec(source.slice(index));
    if (source[index] === '"' || charLiteral) {
      const delimiter = source[index];
      if (charLiteral) {
        index += charLiteral[0].length - 1;
        continue;
      }
      index += 1;
      while (index < source.length) {
        if (source[index] === '\\') {
          index += 2;
        } else if (source[index] === delimiter) {
          break;
        } else {
          index += 1;
        }
      }
      continue;
    }
    if (source[index] === '{') {
      depth += 1;
    } else if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) {
        return index + 1;
      }
    }
  }
  throw new Error('unclosed Rust block while scanning public signatures');
}

function withoutCfgTestModules(source) {
  let result = source;
  const cfgTest = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/gu;
  let match;
  while ((match = cfgTest.exec(result))) {
    const module = /^\s*(?:#\[[^\]]+\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_]\w*\s*\{/u
      .exec(result.slice(match.index + match[0].length));
    if (!module) {
      continue;
    }
    const openingBrace = match.index + match[0].length + module.index + module[0].lastIndexOf('{');
    let end;
    try {
      end = blockEnd(result, openingBrace);
    } catch (error) {
      throw new Error(
        `${error.message}: ${result.slice(match.index, match.index + 120).replace(/\s+/gu, ' ')}`,
      );
    }
    result = result.slice(0, match.index) + result.slice(end);
    cfgTest.lastIndex = match.index;
  }
  return result;
}

function resultErrorType(signature, separator = '->') {
  const separatorIndex = signature.indexOf(separator);
  if (separatorIndex < 0) {
    return undefined;
  }
  const resultStart = separatorIndex + separator.length;
  const result = /(?:[A-Za-z_]\w*::)*Result\s*</u.exec(signature.slice(resultStart));
  if (!result) {
    return undefined;
  }
  const openingBrace = resultStart + result.index + result[0].lastIndexOf('<');
  let depth = 0;
  for (let index = openingBrace; index < signature.length; index += 1) {
    if (signature[index] === '<') {
      depth += 1;
    } else if (signature[index] === '>') {
      depth -= 1;
      if (depth === 0) {
        const argumentsSource = signature.slice(openingBrace + 1, index);
        let nested = 0;
        for (let argumentIndex = argumentsSource.length - 1; argumentIndex >= 0; argumentIndex -= 1) {
          const character = argumentsSource[argumentIndex];
          if (character === '>' || character === ')' || character === ']') {
            nested += 1;
          } else if (character === '<' || character === '(' || character === '[') {
            nested -= 1;
          } else if (character === ',' && nested === 0) {
            return argumentsSource.slice(argumentIndex + 1).trim();
          }
        }
        return undefined;
      }
    }
  }
  return undefined;
}

function isStringError(errorType) {
  return errorType === 'String' || /^\(\s*StatusCode\s*,\s*String\s*\)$/u.test(errorType);
}

function functionSignatures(source, publicOnly) {
  const expression = publicOnly
    ? /\bpub\s+(async\s+)?fn\s+([A-Za-z_]\w*)\b[\s\S]*?(?=\{|;)/gu
    : /\b(?:async\s+)?fn\s+([A-Za-z_]\w*)\b[\s\S]*?(?=\{|;)/gu;
  return Array.from(source.matchAll(expression), (match) => ({
    symbol: match[publicOnly ? 2 : 1],
    signature: publicOnly && match[1] ? 'pub async fn' : 'pub fn',
    source: match[0],
  }));
}

function publicTraitMethods(source) {
  const traits = /\bpub\s+trait\s+[A-Za-z_]\w*[^\{]*\{/gu;
  const methods = [];
  for (const trait of source.matchAll(traits)) {
    const openingBrace = trait.index + trait[0].lastIndexOf('{');
    const body = source.slice(openingBrace + 1, blockEnd(source, openingBrace) - 1);
    methods.push(...functionSignatures(body, false).map((method) => ({
      ...method,
      signature: 'pub trait method',
    })));
  }
  return methods;
}

export function scanPublicStringErrorsInDirectory({
  packageName,
  sourceDirectory,
  relativeTo = repoRoot,
}) {
  if (!fs.existsSync(sourceDirectory)) {
    return [];
  }

  return productionRustSourceFiles(sourceDirectory).flatMap((filePath) => {
    const source = withoutCfgTestModules(fs.readFileSync(filePath, 'utf8'));
    const signatures = [
      ...functionSignatures(source, true),
      ...publicTraitMethods(source),
      ...Array.from(source.matchAll(/\bpub\s+type\s+([A-Za-z_]\w*)\b[\s\S]*?;/gu), (match) => ({
        symbol: match[1],
        signature: 'pub type',
        source: match[0],
      })),
    ];
    const file = path.relative(relativeTo, filePath).split(path.sep).join('/');
    return signatures
      .filter(({ source: signature, signature: kind }) => isStringError(
        resultErrorType(signature, kind === 'pub type' ? '=' : '->'),
      ))
      .map(({ symbol, signature }) => ({ package: packageName, file, symbol, signature }));
  });
}

function scanPackagePublicStringErrors({ memberPath, packageName }) {
  return scanPublicStringErrorsInDirectory({
    packageName,
    sourceDirectory: path.join(repoRoot, memberPath, 'src'),
  });
}

function validateAxumHandlerExemptions() {
  const expected = new Map([
    ['handle_info', 'pub async fn'],
    ['handle_job_status', 'pub async fn'],
    ['handle_transcribe', 'pub async fn'],
  ]);
  const observed = [];

  for (const { memberPath, packageName } of workspacePackages()) {
    const sourceDirectory = path.join(repoRoot, memberPath, 'src');
    if (!fs.existsSync(sourceDirectory)) {
      continue;
    }
    for (const filePath of productionRustSourceFiles(sourceDirectory)) {
      const source = withoutCfgTestModules(fs.readFileSync(filePath, 'utf8'));
      const file = path.relative(repoRoot, filePath).split(path.sep).join('/');
      for (const candidate of functionSignatures(source, true)) {
        if (expected.has(candidate.symbol) && resultErrorType(candidate.source) && /^\(\s*StatusCode\s*,\s*String\s*\)$/u.test(resultErrorType(candidate.source))) {
          observed.push({ package: packageName, file, symbol: candidate.symbol, signature: candidate.signature });
        }
      }
    }
  }

  assert.deepEqual(
    observed.sort((left, right) => left.symbol.localeCompare(right.symbol)),
    [...expected.entries()].map(([symbol, signature]) => ({
      package: 'sona-api-server',
      file: 'adapters/api_server/src/lib.rs',
      symbol,
      signature,
    })).sort((left, right) => left.symbol.localeCompare(right.symbol)),
    'only the reviewed API Server handlers may return the exact Axum (StatusCode, String) error tuple',
  );
}

export function excludeReviewedApiServerStringErrors(errors) {
  return errors.filter(({ package: packageName, file, symbol, signature }) => !(
    packageName === 'sona-api-server'
    && file === 'adapters/api_server/src/lib.rs'
    && ['handle_info', 'handle_job_status', 'handle_transcribe'].includes(symbol)
    && signature === 'pub async fn'
  ));
}

export function findPublicStringErrors() {
  validateAxumHandlerExemptions();
  const errors = workspacePackages()
    .filter(({ role }) =>
      ['core', 'application', 'inbound-adapter', 'outbound-adapter'].includes(role))
    .flatMap(scanPackagePublicStringErrors);
  return excludeReviewedApiServerStringErrors(errors);
}
