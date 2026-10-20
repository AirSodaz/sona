import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import en from '../../../../locales/en.json';
import { groupKeyForTab, SETTINGS_SEARCH_REGISTRY } from '../settingsSearchRegistry';

const componentsDir = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const ANCHOR_PATTERN = /(['"])(settings-[a-z0-9-]+)\1/g;
const CONDITIONAL_ANCHORS = new Set([
  'settings-storage-cleanup',
  'settings-sync-recovery-key',
  'settings-vad-path',
  'settings-punctuation-path',
  'settings-batch-vad',
  'settings-download-mirror',
  'settings-vocabulary-replacements',
]);

function collectTsxSources(dir: string): string[] {
  const sources: string[] = [];
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    if (statSync(fullPath).isDirectory()) {
      if (entry === 'search') {
        continue;
      }
      sources.push(...collectTsxSources(fullPath));
    } else if (entry.endsWith('.tsx')) {
      sources.push(readFileSync(fullPath, 'utf8'));
    }
  }
  return sources;
}

function resolveLocaleKey(key: string): string | undefined {
  let current: unknown = en;
  for (const part of key.split('.')) {
    if (typeof current !== 'object' || current === null) {
      return undefined;
    }
    current = (current as Record<string, unknown>)[part];
  }
  return typeof current === 'string' ? current : undefined;
}

describe('settings search registry integrity', () => {
  it('resolves every targetId and fallbackTargetId to a real component anchor', () => {
    const anchors = new Set<string>();
    for (const source of collectTsxSources(componentsDir)) {
      for (const match of source.matchAll(ANCHOR_PATTERN)) {
        anchors.add(match[2]);
      }
    }

    const missing = SETTINGS_SEARCH_REGISTRY.flatMap((item) =>
      [item.targetId, item.fallbackTargetId]
        .filter((anchor): anchor is string => typeof anchor === 'string' && !anchors.has(anchor))
        .map((anchor) => `${item.id} -> ${anchor}`)
    );

    expect(missing).toEqual([]);
  });

  it('declares a fallback for every conditionally rendered anchor', () => {
    const missingFallbacks = SETTINGS_SEARCH_REGISTRY.filter(
      (item) => item.targetId && CONDITIONAL_ANCHORS.has(item.targetId) && !item.fallbackTargetId
    ).map((item) => item.id);

    expect(missingFallbacks).toEqual([]);
  });

  it('resolves every registry i18n key in the shipped English locale', () => {
    const missing = SETTINGS_SEARCH_REGISTRY.flatMap((item) =>
      [item.titleKey, item.hintKey, item.sectionKey, item.tabKey, groupKeyForTab(item.tab)]
        .filter((key): key is string => Boolean(key))
        .filter((key) => {
          const value = resolveLocaleKey(key);
          return !value || value.trim().length === 0;
        })
        .map((key) => `${item.id} -> ${key}`)
    );

    expect(missing).toEqual([]);
  });
});
