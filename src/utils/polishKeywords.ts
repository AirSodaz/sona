import type { PolishKeywordRuleSet } from '../types/config';

export function normalizePolishKeywordSets(
  sets: PolishKeywordRuleSet[] | null | undefined,
): PolishKeywordRuleSet[] {
  if (!Array.isArray(sets) || sets.length === 0) {
    return [];
  }

  const normalized: PolishKeywordRuleSet[] = [];
  const seenIds = new Set<string>();

  for (let index = 0; index < sets.length; index += 1) {
    const set = sets[index];
    if (!set || typeof set !== 'object') {
      continue;
    }

    const keywords = typeof set.keywords === 'string' ? set.keywords : '';
    const name = typeof set.name === 'string' && set.name.trim()
      ? set.name.trim()
      : buildFallbackPolishKeywordSetName(keywords, index);
    const id = typeof set.id === 'string' && set.id.trim()
      ? set.id.trim()
      : createPolishKeywordSetId(`${name}-${keywords}-${index}`);

    if (seenIds.has(id)) {
      continue;
    }

    normalized.push({
      id,
      name,
      enabled: typeof set.enabled === 'boolean' ? set.enabled : true,
      keywords,
    });
    seenIds.add(id);
  }

  return normalized;
}

export function migrateLegacyPolishKeywords(
  legacyKeywords: string | null | undefined,
  existingSets: PolishKeywordRuleSet[] | null | undefined,
): PolishKeywordRuleSet[] {
  const normalizedSets = normalizePolishKeywordSets(existingSets);
  const normalizedKeywords = typeof legacyKeywords === 'string' ? legacyKeywords.trim() : '';

  if (!normalizedKeywords) {
    return normalizedSets;
  }

  const existingIndex = normalizedSets.findIndex((set) => set.keywords.trim() === normalizedKeywords);
  if (existingIndex >= 0) {
    if (normalizedSets[existingIndex].enabled) {
      return normalizedSets;
    }

    return normalizedSets.map((set, index) => (
      index === existingIndex
        ? { ...set, enabled: true }
        : set
    ));
  }

  return [
    ...normalizedSets,
    {
      id: createPolishKeywordSetId(normalizedKeywords),
      name: `Imported Keywords (${hashString(normalizedKeywords).slice(0, 6)})`,
      enabled: true,
      keywords: normalizedKeywords,
    },
  ];
}

export function resolvePolishKeywords(
  sets: PolishKeywordRuleSet[] | null | undefined,
): string {
  return normalizePolishKeywordSets(sets)
    .filter((set) => set.enabled && set.keywords.trim())
    .map((set) => set.keywords.trim())
    .join('\n\n');
}

function createPolishKeywordSetId(seed: string): string {
  return `polish-keywords-${hashString(seed)}`;
}

function buildFallbackPolishKeywordSetName(keywords: string, index: number): string {
  const trimmedKeywords = keywords.trim();
  if (!trimmedKeywords) {
    return `Untitled Keywords ${index + 1}`;
  }

  return `Imported Keywords (${hashString(trimmedKeywords).slice(0, 6)})`;
}

function hashString(value: string): string {
  let hash = 5381;
  for (let index = 0; index < value.length; index += 1) {
    hash = ((hash << 5) + hash) ^ value.charCodeAt(index);
  }

  return Math.abs(hash >>> 0).toString(16).padStart(8, '0');
}
