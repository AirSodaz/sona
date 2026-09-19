import type { HotwordRuleSet, TextReplacementRuleSet } from '../types/config';

export interface DictionaryTerm {
  raw: string;
  text: string; // Hotword or target word for ASR
  from?: string; // Original text to replace
  to?: string; // Replacement text
  isReplacement: boolean;
  line: number;
}

export interface DictionaryProjectSection {
  projectId?: string;
  projectName: string;
  line: number;
  terms: DictionaryTerm[];
  isDeleted?: boolean;
}

export interface ParsedDictionary {
  globalTerms: DictionaryTerm[];
  projects: DictionaryProjectSection[];
}

export interface DictionaryDiagnostic {
  line: number;
  column?: number;
  message: string;
  severity: 'error' | 'warning' | 'info';
}

export interface KnownProjectRef {
  id: string;
  name: string;
  pipeline?: {
    customTerms?: string[];
  };
}

export const VOICE_TYPING_SCOPE_ID = 'voice-typing';
export const VOICE_TYPING_SCOPE_NAME = 'Voice Typing';

export function isVoiceTypingScopeId(id?: string | null): boolean {
  if (!id) return false;
  const normalized = id.trim().toLowerCase();
  return normalized === 'voice-typing' || normalized === 'voice_typing';
}

export function isVoiceTypingScopeName(name?: string | null): boolean {
  if (!name) return false;
  const trimmed = name.trim();
  const lower = trimmed.toLowerCase();
  return (
    lower === 'voice typing' ||
    lower === 'voice-typing' ||
    lower === 'voice_typing' ||
    trimmed === '语音输入' ||
    trimmed === '語音輸入' ||
    trimmed === '音声入力' ||
    trimmed === '음성 입력' ||
    trimmed === '음성입력'
  );
}

export function isVoiceTypingScope(id?: string | null, name?: string | null): boolean {
  return isVoiceTypingScopeId(id) || isVoiceTypingScopeName(name);
}

const ARROW_REGEX = /\s*(?:=>|->)\s*/;
/**
 * Parses a YAML-like dictionary text into structured global terms and project sections.
 */
export function parseYamlDictionary(
  content: string,
  knownProjects: KnownProjectRef[] = []
): ParsedDictionary {
  const lines = content.split('\n');
  const globalTerms: DictionaryTerm[] = [];
  const projects: DictionaryProjectSection[] = [];

  let inProjectsSection = false;
  let currentProject: DictionaryProjectSection | null = null;

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const lineNum = i + 1;
    const trimmed = rawLine.trim();

    // Skip empty lines and comment lines
    if (!trimmed || trimmed.startsWith('#')) {
      continue;
    }

    // Check for "projects:" root section header
    if (trimmed === 'projects:' || trimmed === 'projects') {
      inProjectsSection = true;
      currentProject = null;
      continue;
    }

    // Inside projects section
    if (inProjectsSection) {
      // Check if line is a project declaration: "  Project Name (id:123):" or "  Project Name:"
      // Match 1 or more leading spaces, then project name + optional (id:xxx), ending with colon
      const projectHeaderMatch = rawLine.match(
        /^(\s{1,4})(?:["']?(.*?)["']?)(?:\s*\((?:id:)?([a-zA-Z0-9_-]+)\))?\s*:\s*$/
      );
      if (projectHeaderMatch) {
        const rawName = projectHeaderMatch[2]?.trim() || '';
        const explicitId = projectHeaderMatch[3]?.trim();

        let resolvedId = explicitId;
        let isDeleted = false;

        if (isVoiceTypingScopeId(explicitId)) {
          resolvedId = VOICE_TYPING_SCOPE_ID;
          isDeleted = false;
        } else if (!explicitId && isVoiceTypingScopeName(rawName)) {
          resolvedId = VOICE_TYPING_SCOPE_ID;
          isDeleted = false;
        } else if (explicitId) {
          const matched = knownProjects.find((p) => p.id === explicitId);
          if (!matched) {
            isDeleted = true;
          }
        } else if (rawName) {
          // If no explicit ID, attempt matching by name
          const matched = knownProjects.find((p) => p.name.toLowerCase() === rawName.toLowerCase());
          if (matched) {
            resolvedId = matched.id;
          }
        }

        currentProject = {
          projectId: resolvedId,
          projectName: rawName,
          line: lineNum,
          terms: [],
          isDeleted,
        };
        projects.push(currentProject);
        continue;
      }

      // Check if line is a term under current project: "    - Term"
      if (currentProject && trimmed.startsWith('-')) {
        const term = parseTermLine(trimmed, lineNum);
        if (term) {
          currentProject.terms.push(term);
        }
        continue;
      }
    }

    // Root / Global section item
    if (trimmed.startsWith('-')) {
      const term = parseTermLine(trimmed, lineNum);
      if (term) {
        globalTerms.push(term);
      }
    }
  }

  return { globalTerms, projects };
}

/**
 * Parses an individual list item line like "- Sona" or "- 苦伯内提斯 -> Kubernetes".
 */
function parseTermLine(trimmedLine: string, lineNum: number): DictionaryTerm | null {
  const content = trimmedLine.replace(/^-\s*/, '').trim();
  if (!content) return null;

  if (ARROW_REGEX.test(content)) {
    const parts = content.split(ARROW_REGEX);
    const from = parts[0]?.trim() || '';
    const to = parts[1]?.trim() || '';
    return {
      raw: trimmedLine,
      text: to,
      from,
      to,
      isReplacement: true,
      line: lineNum,
    };
  }

  return {
    raw: trimmedLine,
    text: content,
    isReplacement: false,
    line: lineNum,
  };
}

/**
 * Validates the dictionary YAML syntax and checks project linking integrity.
 */
export function validateYamlDictionary(
  content: string,
  knownProjects: KnownProjectRef[] = []
): DictionaryDiagnostic[] {
  const diagnostics: DictionaryDiagnostic[] = [];
  const lines = content.split('\n');

  let inProjectsSection = false;
  let currentProjectName: string | null = null;
  const seenGlobalTerms = new Set<string>();
  const seenProjectTerms = new Map<string, Set<string>>();
  const seenProjectIds = new Set<string>();

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const lineNum = i + 1;
    const trimmed = rawLine.trim();

    if (!trimmed || trimmed.startsWith('#')) {
      continue;
    }

    if (trimmed === 'projects:' || trimmed === 'projects') {
      inProjectsSection = true;
      currentProjectName = null;
      continue;
    }

    if (inProjectsSection) {
      // Check for project header
      const projectHeaderMatch = rawLine.match(
        /^(\s{1,4})(?:["']?(.*?)["']?)(?:\s*\((?:id:)?([a-zA-Z0-9_-]+)\))?\s*(:?)\s*$/
      );
      if (projectHeaderMatch && !trimmed.startsWith('-')) {
        const rawName = projectHeaderMatch[2]?.trim() || '';
        const explicitId = projectHeaderMatch[3]?.trim();
        const hasColon = Boolean(projectHeaderMatch[4]);

        if (!hasColon) {
          diagnostics.push({
            line: lineNum,
            message: `Project header "${rawName}" must end with a colon ':'`,
            severity: 'error',
          });
        }

        if (explicitId) {
          if (seenProjectIds.has(explicitId)) {
            diagnostics.push({
              line: lineNum,
              message: `Duplicate project ID reference "(id:${explicitId})"`,
              severity: 'warning',
            });
          }
          seenProjectIds.add(explicitId);

          if (!isVoiceTypingScopeId(explicitId)) {
            const matched = knownProjects.find((p) => p.id === explicitId);
            if (!matched) {
              // Check if there is a newly created project with the same name
              const sameNameProject = knownProjects.find(
                (p) => p.name.toLowerCase() === rawName.toLowerCase()
              );
              if (sameNameProject) {
                diagnostics.push({
                  line: lineNum,
                  message: `Project "${rawName}" (id:${explicitId}) was deleted. A new project with the same name exists (id:${sameNameProject.id}). Format or relink to update.`,
                  severity: 'warning',
                });
              } else {
                diagnostics.push({
                  line: lineNum,
                  message: `Linked project "${rawName}" (id:${explicitId}) no longer exists.`,
                  severity: 'warning',
                });
              }
            }
          }
        } else if (rawName) {
          // No explicit ID
          if (!isVoiceTypingScopeName(rawName)) {
            const matched = knownProjects.find(
              (p) => p.name.toLowerCase() === rawName.toLowerCase()
            );
            if (!matched) {
              diagnostics.push({
                line: lineNum,
                message: `Project "${rawName}" not found in projects. Link to a project by adding "(id:project_id)".`,
                severity: 'info',
              });
            }
          }
        }

        currentProjectName = rawName || explicitId || 'unknown';
        if (!seenProjectTerms.has(currentProjectName)) {
          seenProjectTerms.set(currentProjectName, new Set());
        }
        continue;
      }

      // Check item under project
      if (trimmed.startsWith('-')) {
        if (!currentProjectName) {
          diagnostics.push({
            line: lineNum,
            message: 'List item inside "projects:" must be placed under a project header.',
            severity: 'error',
          });
        }
        validateTermContent(
          trimmed,
          lineNum,
          diagnostics,
          seenProjectTerms.get(currentProjectName || '')
        );
        continue;
      }

      // Neither project header nor term
      diagnostics.push({
        line: lineNum,
        message: `Unexpected text "${trimmed}". Expected a project header or an item starting with "- ".`,
        severity: 'error',
      });
      continue;
    }

    // Global section
    if (trimmed.startsWith('-')) {
      validateTermContent(trimmed, lineNum, diagnostics, seenGlobalTerms);
      continue;
    }

    // Unexpected root line
    diagnostics.push({
      line: lineNum,
      message: `Invalid line "${trimmed}". Global terms must start with "- ", or start a "projects:" section.`,
      severity: 'error',
    });
  }

  return diagnostics;
}

function validateTermContent(
  trimmedLine: string,
  lineNum: number,
  diagnostics: DictionaryDiagnostic[],
  seenSet?: Set<string>
) {
  const content = trimmedLine.replace(/^-\s*/, '').trim();
  if (!content) {
    diagnostics.push({
      line: lineNum,
      message: 'Empty list item.',
      severity: 'warning',
    });
    return;
  }

  // Count arrows
  const arrowMatches = content.match(/=>|->/g);
  if (arrowMatches && arrowMatches.length > 1) {
    diagnostics.push({
      line: lineNum,
      message: 'Multiple replacement arrows found. Only one "->" or "=>" is allowed per line.',
      severity: 'error',
    });
    return;
  }

  if (arrowMatches && arrowMatches.length === 1) {
    const sep = arrowMatches[0];
    const parts = content.split(sep);
    const from = parts[0]?.trim();
    const to = parts[1]?.trim();
    if (!from || !to) {
      diagnostics.push({
        line: lineNum,
        message: 'Replacement rule must have both source and target (e.g. "- From -> To").',
        severity: 'error',
      });
      return;
    }
  }

  if (seenSet) {
    if (seenSet.has(content.toLowerCase())) {
      diagnostics.push({
        line: lineNum,
        message: `Duplicate entry "${content}".`,
        severity: 'info',
      });
    } else {
      seenSet.add(content.toLowerCase());
    }
  }
}

/**
 * Formats the YAML dictionary text cleanly:
 * - Normalizes indentation (2 spaces for projects, 4 spaces for items).
 * - Synchronizes project display names with known active projects.
 * - Auto-links unlinked project names to their IDs if a matching project exists.
 * - Canonicalizes arrows to "->".
 */
export function formatYamlDictionary(
  content: string,
  knownProjects: KnownProjectRef[] = []
): string {
  const parsed = parseYamlDictionary(content, knownProjects);
  const lines: string[] = [];

  // 1. Global section
  lines.push('# Global Vocabulary & Replacements');
  if (parsed.globalTerms.length === 0) {
    lines.push('# - ExampleTerm');
    lines.push('# - MisrecognizedWord -> CorrectWord');
  } else {
    for (const term of parsed.globalTerms) {
      if (term.isReplacement && term.from && term.to) {
        lines.push(`- ${term.from} -> ${term.to}`);
      } else {
        lines.push(`- ${term.text}`);
      }
    }
  }

  // 2. Projects section
  if (parsed.projects.length > 0 || knownProjects.length > 0) {
    lines.push('');
    lines.push('# Project Specific Terms & Replacements');
    lines.push('projects:');

    for (const proj of parsed.projects) {
      // Find if project is in knownProjects or is voice typing
      let displayName = proj.projectName;
      let resolvedId = proj.projectId;

      if (isVoiceTypingScopeId(resolvedId) || isVoiceTypingScopeName(displayName)) {
        resolvedId = VOICE_TYPING_SCOPE_ID;
        displayName = proj.projectName || VOICE_TYPING_SCOPE_NAME;
      } else if (resolvedId) {
        const matched = knownProjects.find((p) => p.id === resolvedId);
        if (matched) {
          displayName = matched.name; // Synchronize name in case it was renamed!
        }
      } else {
        // Try linking by name
        const matched = knownProjects.find(
          (p) => p.name.toLowerCase() === displayName.toLowerCase()
        );
        if (matched) {
          resolvedId = matched.id;
          displayName = matched.name;
        }
      }

      const idAnnotation = resolvedId ? ` (id:${resolvedId})` : '';
      lines.push(`  ${displayName}${idAnnotation}:`);

      if (proj.terms.length === 0) {
        lines.push(`    # - Term`);
      } else {
        for (const term of proj.terms) {
          if (term.isReplacement && term.from && term.to) {
            lines.push(`    - ${term.from} -> ${term.to}`);
          } else {
            lines.push(`    - ${term.text}`);
          }
        }
      }
    }
  }

  return lines.join('\n');
}

/**
 * Serializes legacy rule sets and project pipeline terms into unified YAML dictionary.
 */
export function serializeToYamlDictionary(
  hotwordSets: HotwordRuleSet[] = [],
  textReplacementSets: TextReplacementRuleSet[] = [],
  knownProjects: KnownProjectRef[] = []
): string {
  const lines: string[] = [];

  lines.push('# Global Vocabulary & Replacements');

  // Collect global hotwords
  const globalWords = new Set<string>();
  for (const set of hotwordSets) {
    for (const rule of set.rules) {
      const trimmed = rule.text.trim();
      if (trimmed) globalWords.add(trimmed);
    }
  }

  // Collect global replacements
  const globalReplacements: Array<{ from: string; to: string }> = [];
  for (const set of textReplacementSets) {
    for (const rule of set.rules) {
      const from = rule.from.trim();
      const to = rule.to.trim();
      if (from && to) {
        globalReplacements.push({ from, to });
      }
    }
  }

  for (const word of globalWords) {
    lines.push(`- ${word}`);
  }
  for (const rep of globalReplacements) {
    lines.push(`- ${rep.from} -> ${rep.to}`);
  }

  if (globalWords.size === 0 && globalReplacements.length === 0) {
    lines.push('# - Sona');
    lines.push('# - DeepSeek');
    lines.push('# - 苦伯内提斯 -> Kubernetes');
  }

  // Projects with terms
  const projectsWithTerms = knownProjects.filter(
    (p) => p.pipeline?.customTerms && p.pipeline.customTerms.length > 0
  );

  if (projectsWithTerms.length > 0) {
    lines.push('');
    lines.push('# Project Specific Terms & Replacements');
    lines.push('projects:');

    for (const p of projectsWithTerms) {
      lines.push(`  ${p.name} (id:${p.id}):`);
      for (const rawTerm of p.pipeline?.customTerms || []) {
        const trimmed = rawTerm.trim();
        if (!trimmed) continue;
        if (trimmed.includes('=>') || trimmed.includes('->')) {
          const sep = trimmed.includes('=>') ? '=>' : '->';
          const parts = trimmed.split(sep);
          const from = parts[0]?.trim();
          const to = parts[1]?.trim();
          if (from && to) {
            lines.push(`    - ${from} -> ${to}`);
          }
        } else {
          lines.push(`    - ${trimmed}`);
        }
      }
    }
  }

  return lines.join('\n');
}

/**
 * Extracts terms and replacements for a specific project from the parsed dictionary.
 */
export function getTermsForProject(
  parsed: ParsedDictionary,
  projectId: string | null | undefined,
  projectName?: string
): {
  hotwords: string[];
  replacements: Array<{ from: string; to: string }>;
} {
  const hotwords: string[] = [];
  const replacements: Array<{ from: string; to: string }> = [];

  // Add global terms
  for (const term of parsed.globalTerms) {
    if (term.isReplacement && term.from && term.to) {
      replacements.push({ from: term.from, to: term.to });
      hotwords.push(term.to); // Target word is also a hotword
    } else if (term.text) {
      hotwords.push(term.text);
    }
  }

  // Find matching project section
  if (projectId || projectName) {
    const isTargetVoiceTyping = isVoiceTypingScope(projectId, projectName);
    const projectSection = parsed.projects.find((p) => {
      if (isTargetVoiceTyping) {
        return isVoiceTypingScope(p.projectId, p.projectName);
      }
      if (projectId && p.projectId === projectId) return true;
      if (projectName && p.projectName.toLowerCase() === projectName.toLowerCase()) return true;
      return false;
    });
    if (projectSection) {
      for (const term of projectSection.terms) {
        if (term.isReplacement && term.from && term.to) {
          replacements.push({ from: term.from, to: term.to });
          hotwords.push(term.to);
        } else if (term.text) {
          hotwords.push(term.text);
        }
      }
    }
  }

  return {
    hotwords: Array.from(new Set(hotwords)),
    replacements,
  };
}

/**
 * Appends or inserts a new term into the YAML dictionary under the specified scope.
 * Prevents duplicates within the target scope and formats clean YAML.
 */
export function addTermToYamlDictionary(
  content: string,
  term: string,
  targetScope: 'global' | { id: string; name: string } = 'global',
  knownProjects: KnownProjectRef[] = []
): string {
  const trimmed = term.trim();
  if (!trimmed) return content;

  let entry = trimmed;
  if (entry.includes('=>')) {
    entry = entry.replace('=>', '->');
  }
  if (!entry.startsWith('-')) {
    entry = `- ${entry}`;
  }

  const isEssentiallyEmpty =
    !content.trim() ||
    content
      .trim()
      .split('\n')
      .every((l) => !l.trim() || l.trim().startsWith('#'));

  if (targetScope === 'global') {
    if (isEssentiallyEmpty) {
      return `# Global Vocabulary & Replacements\n${entry}\n`;
    }

    const parsed = parseYamlDictionary(content, knownProjects);
    const normalizedText = entry.replace(/^-\s*/, '').trim().toLowerCase();
    const alreadyExists = parsed.globalTerms.some((t) => {
      const termNormalized =
        t.isReplacement && t.from && t.to
          ? `${t.from} -> ${t.to}`.toLowerCase()
          : t.text.toLowerCase();
      return termNormalized === normalizedText;
    });
    if (alreadyExists) {
      return content;
    }

    const lines = content.split('\n');
    const projectsIdx = lines.findIndex((l) => l.trim() === 'projects:' || l.trim() === 'projects');
    if (projectsIdx >= 0) {
      lines.splice(projectsIdx, 0, entry);
      return lines.join('\n');
    } else {
      return `${content.trimEnd()}\n${entry}\n`;
    }
  }

  // targetScope is { id, name }
  const targetId = targetScope.id;
  const isTargetVoiceTyping = isVoiceTypingScopeId(targetId);
  const targetName = targetScope.name || (isTargetVoiceTyping ? VOICE_TYPING_SCOPE_NAME : targetId);

  if (isEssentiallyEmpty) {
    return `# Global Vocabulary & Replacements\n\n# Project Specific Terms & Replacements\nprojects:\n  ${targetName} (id:${targetId}):\n    ${entry}\n`;
  }

  const parsed = parseYamlDictionary(content, knownProjects);
  const lines = content.split('\n');

  const existingProj = parsed.projects.find((p) => {
    if (isTargetVoiceTyping) {
      return isVoiceTypingScope(p.projectId, p.projectName);
    }
    return p.projectId === targetId || p.projectName.toLowerCase() === targetName.toLowerCase();
  });

  if (existingProj) {
    const normalizedText = entry.replace(/^-\s*/, '').trim().toLowerCase();
    const alreadyExists = existingProj.terms.some((t) => {
      const termNormalized =
        t.isReplacement && t.from && t.to
          ? `${t.from} -> ${t.to}`.toLowerCase()
          : t.text.toLowerCase();
      return termNormalized === normalizedText;
    });
    if (alreadyExists) {
      return content;
    }

    const headerLineIdx = existingProj.line - 1;
    lines.splice(headerLineIdx + 1, 0, `    ${entry}`);
    return lines.join('\n');
  }

  let projectsIdx = lines.findIndex((l) => l.trim() === 'projects:' || l.trim() === 'projects');
  if (projectsIdx === -1) {
    lines.push('', '# Project Specific Terms & Replacements', 'projects:');
    projectsIdx = lines.length - 1;
  }

  lines.push(`  ${targetName} (id:${targetId}):`, `    ${entry}`);
  return lines.join('\n');
}
