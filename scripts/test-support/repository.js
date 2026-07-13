import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
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
const repoRoot = path.resolve(__dirname, '..', '..');
const desktopCrateSegments = ['platforms', 'desktop'];
const desktopCratePath = (...segments) => path.join(repoRoot, ...desktopCrateSegments, ...segments);
const expectedUniffiExports = Object.freeze([
  'load_project_repository_state_json',
  'replace_projects_json',
  'create_project_json',
  'update_project_json',
  'delete_project',
  'reorder_projects_json',
  'set_active_project_id',
  'load_recovery_snapshot_json',
  'save_recovery_snapshot_json',
  'persist_recovery_queue_snapshot_json',
  'load_task_ledger_snapshot_json',
  'upsert_task_ledger_record_json',
  'patch_task_ledger_record_json',
  'remove_task_ledger_record_json',
  'clear_resolved_task_ledger_records_json',
  'load_automation_repository_state_json',
  'replace_automation_rules_json',
  'replace_automation_processed_entries_json',
  'replace_automation_repository_state_json',
  'validate_automation_rule_activation_json',
  'normalize_export_format',
  'export_transcript_file_json',
  'list_history_items_json',
  'query_history_workspace_json',
  'load_history_transcript_json',
  'list_history_transcript_snapshots_json',
  'load_history_transcript_snapshot_json',
  'default_vad_model_id',
  'default_punctuation_model_id',
  'preset_model_name',
  'preset_models',
  'model_catalog_snapshot',
  'model_catalog_selected_ids',
  'resolve_model_download',
  'resolve_gpu_acceleration',
  'default_config_json',
  'migrate_app_config_json',
  'resolve_effective_config_json',
  'load_app_config_json',
  'save_app_config_json',
  'get_app_setting_json',
  'set_app_setting_json',
  'load_dashboard_snapshot_json',
  'load_diagnostics_snapshot_json',
  'load_storage_usage_snapshot_json',
  'runtime_path_status',
  'create_online_asr_streaming_session',
  'default_batch_segmentation_mode',
  'online_asr_providers',
  'find_online_asr_provider',
  'online_asr_provider_request',
  'volcengine_doubao_asr_config_from_json',
  'llm_providers',
  'find_llm_provider_by_id_or_alias',
  'llm_config_from_json',
  'validate_llm_config_json',
  'validate_llm_generate_request_json',
  'validate_polish_segments_request_json',
  'validate_translate_segments_request_json',
  'validate_summarize_transcript_request_json',
  'llm_segment_inputs_from_transcript_json',
  'summary_segment_inputs_from_transcript_json',
  'merge_translated_items_into_transcript_json',
  'merge_polished_items_into_transcript_json',
  'summary_source_fingerprint_from_transcript_json',
  'build_polish_prompt_json',
  'build_translate_prompt_json',
  'build_summary_chunk_prompt_json',
  'build_summary_finalize_prompt_json',
  'plan_polish_prompt_chunks_json',
  'plan_translate_prompt_chunks_json',
  'plan_summary_prompt_chunks_json',
  'parse_polish_chunk_json',
  'parse_translate_chunk_json',
  'polish_segments_request_from_json',
  'translate_segments_request_from_json',
  'summarize_transcript_request_from_json',
]);
const expectedUniffiErrorVariants = Object.freeze([
  'InvalidInput',
  'Recovery',
  'TaskLedger',
  'Automation',
  'Project',
  'AsrRuntime',
  'ConfigRepository',
  'Dashboard',
  'Diagnostics',
  'StorageUsage',
  'Export',
  'HistoryQuery',
]);
const desktopFrontendDependencies = [
  '@dnd-kit/core',
  '@dnd-kit/modifiers',
  '@dnd-kit/sortable',
  '@dnd-kit/utilities',
  '@eslint/js',
  '@lexical/html',
  '@lexical/react',
  '@lexical/rich-text',
  '@playwright/test',
  '@tauri-apps/api',
  '@tauri-apps/cli',
  '@tauri-apps/plugin-dialog',
  '@tauri-apps/plugin-fs',
  '@tauri-apps/plugin-global-shortcut',
  '@tauri-apps/plugin-log',
  '@tauri-apps/plugin-opener',
  '@tauri-apps/plugin-process',
  '@tauri-apps/plugin-store',
  '@tauri-apps/plugin-updater',
  '@testing-library/dom',
  '@testing-library/react',
  '@types/node',
  '@types/react',
  '@types/react-dom',
  '@vitejs/plugin-react',
  'eslint',
  'eslint-plugin-react-hooks',
  'eslint-plugin-react-refresh',
  'globals',
  'i18next',
  'i18next-browser-languagedetector',
  'jsdom',
  'lexical',
  'lucide-react',
  'react',
  'react-dom',
  'react-i18next',
  'react-virtuoso',
  'recharts',
  'typescript',
  'typescript-eslint',
  'uuid',
  'vite',
  'vitest',
  'zustand',
];

function assertPrRecoveryCoverage(workflowSource) {
  const workflow = YAML.parse(workflowSource);
  const rustBackendSteps = workflow.jobs?.['rust-backend']?.steps ?? [];
  const packageTests = rustBackendSteps.find(
    (step) => step.name === 'Run core, adapters, bindings, and standalone CLI tests',
  )?.run;
  const desktopRecoveryTests = rustBackendSteps.find(
    (step) => step.name === 'Run desktop recovery integration tests',
  )?.run;

  assert.equal(typeof packageTests, 'string', 'missing Rust package test step');
  assert.match(
    packageTests,
    /(?:^|\s)-p sona-recovery-fs(?:\s|$)/u,
    'Rust package tests must include sona-recovery-fs',
  );
  assert.equal(
    desktopRecoveryTests,
    'cargo test -p sona --test recovery_repository',
    'desktop recovery integration tests must execute in PR guardrails',
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
  let tableEntry;

  for (const line of lines) {
    const structuralLine = stripTomlLineComment(line).trim();
    const sectionMatch = structuralLine.match(/^\[([^[\]]+)\]$/u);

    if (sectionMatch) {
      const cargoSection = sectionMatch[1];
      inSection = cargoSection === sectionName || cargoSection.endsWith(`.${sectionName}`);
      tableEntry = undefined;
      const tableMarker = `${sectionName}.`;
      const nestedTableMarker = `.${tableMarker}`;
      const tableName = cargoSection.startsWith(tableMarker)
        ? cargoSection.slice(tableMarker.length)
        : cargoSection.includes(nestedTableMarker)
          ? cargoSection.slice(cargoSection.lastIndexOf(nestedTableMarker) + nestedTableMarker.length)
          : '';
      if (/^[A-Za-z0-9_-]+$/u.test(tableName)) {
        tableEntry = { name: tableName, spec: '', packageName: undefined };
        entries.push(tableEntry);
      }
      continue;
    }
    if ((!inSection && !tableEntry) || structuralLine === '') {
      continue;
    }

    if (tableEntry) {
      const propertyMatch = structuralLine.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/u);
      if (propertyMatch) {
        const priorProperties = tableEntry.spec === '' ? '' : `${tableEntry.spec.slice(2, -2)}, `;
        tableEntry.spec = `{ ${priorProperties}${propertyMatch[1]} = ${propertyMatch[2]} }`;
        if (propertyMatch[1] === 'package') {
          const packageName = propertyMatch[2].match(/^(?:"([^"]+)"|'([^']+)')$/u);
          tableEntry.packageName = packageName?.[1] ?? packageName?.[2];
        }
      }
      continue;
    }

    const dependencyMatch = structuralLine.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/u);
    if (dependencyMatch) {
      const packageName = dependencyMatch[2]
        .match(/\bpackage\s*=\s*(?:"([^"]+)"|'([^']+)')/u);
      entries.push({
        name: dependencyMatch[1],
        spec: dependencyMatch[2],
        packageName: packageName?.[1] ?? packageName?.[2],
      });
    }
  }

  return entries;
}

function readCargoStringArray(cargoTomlPath, sectionName, keyName) {
  const lines = fs.readFileSync(cargoTomlPath, 'utf8').split(/\r?\n/u);
  let inSection = false;
  let collecting = false;
  let arraySource = '';

  for (const rawLine of lines) {
    const line = stripTomlLineComment(rawLine).trim();
    const sectionMatch = line.match(/^\[([^[\]]+)\]$/u);
    if (sectionMatch) {
      if (collecting) break;
      inSection = sectionMatch[1] === sectionName;
      continue;
    }
    if (!inSection) continue;

    if (!collecting) {
      const assignment = line.match(/^([A-Za-z0-9_-]+)\s*=\s*\[(.*)$/u);
      if (assignment?.[1] !== keyName) continue;
      collecting = true;
      arraySource = assignment[2];
    } else {
      arraySource += `\n${line}`;
    }

    const structuralArraySource = arraySource.replace(
      /"(?:\\.|[^"\\])*"|'[^']*'/gu,
      (value) => ' '.repeat(value.length),
    );
    const closingIndex = structuralArraySource.indexOf(']');
    if (closingIndex !== -1) {
      return [...arraySource.slice(0, closingIndex).matchAll(/"(?:\\.|[^"\\])*"|'[^']*'/gu)]
        .map((match) => match[0].startsWith('"')
          ? JSON.parse(match[0])
          : match[0].slice(1, -1));
    }
  }

  return [];
}

function stripTomlLineComment(line) {
  return line.replace(
    /"(?:\\.|[^"\\])*"|'[^']*'|#.*/gu,
    (value) => value.startsWith('#') ? '' : value,
  );
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

function rustCharacterLiteralAt(source, index) {
  return /^(?:b)?'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,6}\}|[nrt0\\'"])|[^'\\\r\n])'/u
    .exec(source.slice(index));
}

function stripRustComments(source) {
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
    const character = rustCharacterLiteralAt(source, index);
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

    const character = rustCharacterLiteralAt(source, index);
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
  const ownerHeader = structural.slice(previousBoundary + 1, owner.index);
  const parent = delimiters.at(-2);
  let parentHeader = '';
  if (parent?.character === '{') {
    const parentBoundary = Math.max(
      structural.lastIndexOf('{', parent.index - 1),
      structural.lastIndexOf('}', parent.index - 1),
      structural.lastIndexOf(';', parent.index - 1),
    );
    parentHeader = structural.slice(parentBoundary + 1, parent.index);
  }
  return {
    commaTerminated: /\b(?:struct|enum|union)\b/u.test(ownerHeader)
      || /\benum\b/u.test(parentHeader),
    parenthesisOwner: false,
  };
}

function rustProductionView(source) {
  let production = stripRustComments(source);
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
  let parenthesisDepth = [...lines[0]].reduce(
    (depth, character) => depth + (character === '(' ? 1 : character === ')' ? -1 : 0),
    0,
  );

  for (let index = 1; index < lines.length; index += 1) {
    const line = lines[index];
    const lineStartOffset = endOffset + 1;
    const trimmed = line.trim();
    const lineIndent = line.match(/^[ \\t]*/u)?.[0].length ?? 0;

    if (trimmed !== '' && lineIndent <= functionIndent && parenthesisDepth === 0) {
      return remainingSource.slice(0, endOffset);
    }
    endOffset = lineStartOffset + line.length;
    parenthesisDepth += [...line].reduce(
      (depth, character) => depth + (character === '(' ? 1 : character === ')' ? -1 : 0),
      0,
    );
  }

  return remainingSource;
}

function readKotlinObjectBlock(source, objectName) {
  const signature = new RegExp(`\\bobject\\s+${objectName}\\s*\\{`, 'u').exec(source);
  if (!signature) {
    return '';
  }

  const openingBrace = source.indexOf('{', signature.index);
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

function readKotlinDirectFunctionItem(objectBlock, functionName) {
  const normalizedSource = objectBlock.replace(/\r\n/gu, '\n');
  let braceDepth = 0;
  let lineStart = 0;

  for (let index = 0; index < normalizedSource.length; index += 1) {
    if (index === lineStart && braceDepth === 1) {
      const lineEnd = normalizedSource.indexOf('\n', lineStart);
      const line = normalizedSource.slice(
        lineStart,
        lineEnd === -1 ? normalizedSource.length : lineEnd,
      );
      if (new RegExp(`^[ \\t]*fun\\s+${functionName}\\b`, 'u').test(line)) {
        return readKotlinFunctionItem(normalizedSource.slice(lineStart), functionName);
      }
    }

    if (normalizedSource[index] === '{') {
      braceDepth += 1;
    } else if (normalizedSource[index] === '}') {
      braceDepth -= 1;
    } else if (normalizedSource[index] === '\n') {
      lineStart = index + 1;
    }
  }

  return '';
}

function stripKotlinCommentsAndLiterals(source) {
  let output = '';
  let index = 0;
  let state = 'code';
  let blockCommentDepth = 0;

  const blank = (character) => (character === '\n' || character === '\r' ? character : ' ');

  while (index < source.length) {
    const character = source[index];
    const next = source[index + 1];
    const nextTwo = source.slice(index, index + 3);

    if (state === 'line-comment') {
      output += blank(character);
      index += 1;
      if (character === '\n') state = 'code';
      continue;
    }

    if (state === 'block-comment') {
      if (character === '/' && next === '*') {
        output += '  ';
        index += 2;
        blockCommentDepth += 1;
      } else if (character === '*' && next === '/') {
        output += '  ';
        index += 2;
        blockCommentDepth -= 1;
        if (blockCommentDepth === 0) state = 'code';
      } else {
        output += blank(character);
        index += 1;
      }
      continue;
    }

    if (state === 'raw-string') {
      if (nextTwo === '"""') {
        output += '   ';
        index += 3;
        state = 'code';
      } else {
        output += blank(character);
        index += 1;
      }
      continue;
    }

    if (state === 'string' || state === 'character') {
      const terminator = state === 'string' ? '"' : "'";
      if (character === '\\') {
        output += '  ';
        index += Math.min(2, source.length - index);
      } else {
        output += blank(character);
        index += 1;
        if (character === terminator) state = 'code';
      }
      continue;
    }

    if (character === '/' && next === '/') {
      output += '  ';
      index += 2;
      state = 'line-comment';
    } else if (character === '/' && next === '*') {
      output += '  ';
      index += 2;
      blockCommentDepth = 1;
      state = 'block-comment';
    } else if (nextTwo === '"""') {
      output += '   ';
      index += 3;
      state = 'raw-string';
    } else if (character === '"') {
      output += ' ';
      index += 1;
      state = 'string';
    } else if (character === "'") {
      output += ' ';
      index += 1;
      state = 'character';
    } else {
      output += character;
      index += 1;
    }
  }

  return output;
}

function assertKotlinRecoveryImport(kotlinSmoke, generatedFunction) {
  const executableSource = stripKotlinCommentsAndLiterals(kotlinSmoke);
  assert.match(
    executableSource,
    new RegExp(`^[ \\t]*import\\s+uniffi\\.sona_uniffi_bind\\.${generatedFunction}\\s*$`, 'mu'),
    `missing generated Kotlin import for ${generatedFunction}`,
  );
}

function assertKotlinRecoveryCall(kotlinSmoke, methodName, directDelegation) {
  const executableSource = stripKotlinCommentsAndLiterals(kotlinSmoke);
  const method = readKotlinDirectFunctionItem(executableSource, methodName);

  assert.notEqual(method, '', `missing ${methodName} Kotlin smoke method`);
  assert.match(
    method,
    directDelegation,
    `${methodName} must directly delegate to the generated recovery binding`,
  );
}

function assertAndroidRecoverySampleSmoke(kotlinSmoke) {
  for (const generatedFunction of [
    'loadRecoverySnapshotJson',
    'saveRecoverySnapshotJson',
    'persistRecoveryQueueSnapshotJson',
  ]) {
    assertKotlinRecoveryImport(kotlinSmoke, generatedFunction);
  }

  const smokeObject = readKotlinObjectBlock(
    stripKotlinCommentsAndLiterals(kotlinSmoke),
    'SonaUniffiSmoke',
  );
  assert.notEqual(smokeObject, '', 'SonaUniffiSmoke must contain loadRecovery');

  assertKotlinRecoveryCall(
    smokeObject,
    'loadRecovery',
    /^[ \t]*fun loadRecovery\(appDataDir: String\): String\s*=\s*loadRecoverySnapshotJson\(appDataDir\)\s*$/u,
  );
  assertKotlinRecoveryCall(
    smokeObject,
    'saveRecovery',
    /^[ \t]*fun saveRecovery\(appDataDir: String, itemsJson: String\): String\s*=\s*saveRecoverySnapshotJson\(appDataDir, itemsJson\)\s*$/u,
  );
  assertKotlinRecoveryCall(
    smokeObject,
    'persistRecovery',
    /^[ \t]*fun persistRecovery\(\s*appDataDir: String,\s*queueItemsJson: String,\s*resolvedIds: List<String>,?\s*\): String\s*=\s*persistRecoveryQueueSnapshotJson\(appDataDir, queueItemsJson, resolvedIds\)\s*$/u,
  );
}

function assertAndroidRecoveryConsumerSmoke(kotlinSmoke) {
  assertKotlinRecoveryImport(kotlinSmoke, 'loadRecoverySnapshotJson');
  const smokeObject = readKotlinObjectBlock(
    stripKotlinCommentsAndLiterals(kotlinSmoke),
    'SonaUniffiConsumerSmoke',
  );
  assert.notEqual(smokeObject, '', 'SonaUniffiConsumerSmoke must contain loadRecovery');
  assertKotlinRecoveryCall(
    smokeObject,
    'loadRecovery',
    /^[ \t]*fun loadRecovery\(appDataDir: String\): String\s*=\s*loadRecoverySnapshotJson\(appDataDir\)\s*$/u,
  );
}

function assertStreamingAsrArchitecture(uniffiCargoPath, streamingBridge) {
  const dependencyNames = readCargoDependencyNames(uniffiCargoPath, 'dependencies');
  const localAsrSpec = readCargoDependencySpec(uniffiCargoPath, 'dependencies', 'sona-local-asr');
  const onlineAsrSpec = readCargoDependencySpec(uniffiCargoPath, 'dependencies', 'sona-online-asr');
  const uncommentedStreamingBridge = stripRustComments(streamingBridge);
  const factoryBlock = readRustFunctionBlock(
    uncommentedStreamingBridge,
    'create_online_asr_streaming_session',
  );

  assert.ok(dependencyNames.includes('sona-online-asr'));
  assert.ok(dependencyNames.includes('sona-local-asr'));
  assert.match(localAsrSpec, /\bpath\s*=\s*"\.\.\/local_asr"/u);
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
  assert.doesNotMatch(uncommentedStreamingBridge, /sona_local_asr|create_local_asr_streaming_session/u);
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

export { repoRoot, read, exists, desktopCrateSegments, desktopCratePath, desktopFrontendDependencies, expectedUniffiExports, expectedUniffiErrorVariants, assertPrRecoveryCoverage, rustFilesUnder, readCargoDependencyNames, readCargoDependencySpec, readCargoDependencyEntries, readCargoStringArray, stripTomlLineComment, assertCargoDependencyVersionAndFeature, stripRustComments, rustProductionView, readRustFunctionBlock, readJavaScriptFunctionBlock, readKotlinFunctionItem, readKotlinObjectBlock, readKotlinDirectFunctionItem, stripKotlinCommentsAndLiterals, assertKotlinRecoveryImport, assertKotlinRecoveryCall, assertAndroidRecoverySampleSmoke, assertAndroidRecoveryConsumerSmoke, assertStreamingAsrArchitecture, assertAndroidStreamingSmoke, scanRustSourcePolicyViolations, stripRustLineComment, readWorkflowStep, readWorkflowStepIndex, readWorkflowBuildTauriSteps };
