import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { SETTINGS_SEARCH_REGISTRY, type SettingsSearchItem } from './settingsSearchRegistry';

export interface SettingsSearchHit extends SettingsSearchItem {
  score: number;
  resolvedTitle: string;
  resolvedHint?: string;
  resolvedSection?: string;
  resolvedTab: string;
  resolvedGroup: string;
}

/**
 * Calculates a relevance score for a search item against a normalized query string.
 *
 * Scoring rules:
 * - Title exact match: +100
 * - Title starts with query: +80
 * - Title contains query: +60
 * - Keyword/alias exact match: +50
 * - Keyword/alias contains query: +40
 * - Hint contains query: +30
 * - Section or Tab title contains query: +20
 *
 * When title is an exact match (+100), redundant keyword/hint bonuses are not added
 * so exact title match has a clean, definitive top score of 100.
 */
export function resolveDisplayTitle(
  item: SettingsSearchItem,
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  const rawTitle = t(item.titleKey, { defaultValue: '' });
  if (rawTitle && rawTitle !== item.titleKey) {
    return rawTitle;
  }
  return item.id.replace(/-/g, ' ');
}

export function calculateItemScore(
  item: SettingsSearchItem,
  normalizedQuery: string,
  t: (key: string, options?: Record<string, unknown>) => string
): number {
  const q = normalizedQuery.trim().toLowerCase();
  if (!q) return 0;

  const isCjk = (str: string) => /[\u4e00-\u9fa5\u3040-\u309f\u30a0-\u30ff\uac00-\ud7af]/.test(str);
  const isSingleLatin = q.length === 1 && !isCjk(q);

  const displayTitle = resolveDisplayTitle(item, t);
  const title = (displayTitle || '').toLowerCase();
  const rawHint = item.hintKey ? t(item.hintKey, { defaultValue: '' }) : '';
  const hint = (rawHint && rawHint !== item.hintKey ? rawHint : '').toLowerCase();
  const rawSection = item.sectionKey ? t(item.sectionKey, { defaultValue: '' }) : '';
  const section = (rawSection && rawSection !== item.sectionKey ? rawSection : '').toLowerCase();
  const rawTab = t(item.tabKey, { defaultValue: '' });
  const tab = (rawTab && rawTab !== item.tabKey ? rawTab : '').toLowerCase();

  const terms = q.split(/\s+/).filter(Boolean);
  const keywords = (item.keywords || []).map((k) => k.toLowerCase());

  const matchesKeyword = (term: string) => {
    const termIsSingleLatin = term.length === 1 && !isCjk(term);
    return keywords.some(
      (k) =>
        k === term ||
        (!termIsSingleLatin &&
          (k.includes(term) ||
            (term.length > 2 && k.length >= (isCjk(k) ? 2 : 3) && term.includes(k))))
    );
  };

  // If multi-token query, every token must match at least one attribute of the item
  if (terms.length > 1) {
    const allTokensMatch = terms.every(
      (term) =>
        title.includes(term) ||
        matchesKeyword(term) ||
        hint.includes(term) ||
        section.includes(term) ||
        tab.includes(term)
    );
    if (!allTokensMatch) {
      return 0;
    }
  }

  let score = 0;

  // Title matching
  if (title === q) {
    score += 100;
  } else if (
    title.startsWith(q) ||
    (terms.length > 1 && terms.some((term) => title === term || title.startsWith(term)))
  ) {
    score += 80;
  } else if (
    (!isSingleLatin && title.includes(q)) ||
    (terms.length > 1 && terms.some((term) => title.includes(term)))
  ) {
    score += 60;
  }

  // If title is an exact match (+100), it's already the highest relevance match.
  if (title === q) {
    return score;
  }

  // Keyword / alias matching
  const hasExactKeyword = keywords.some(
    (k) => k === q || (terms.length > 1 && terms.some((t) => k === t))
  );
  const hasSubKeyword =
    !hasExactKeyword &&
    !isSingleLatin &&
    (terms.length > 1
      ? terms.some(matchesKeyword)
      : keywords.some(
          (k) => k.includes(q) || (q.length > 2 && k.length >= (isCjk(k) ? 2 : 3) && q.includes(k))
        ));
  if (hasExactKeyword) {
    score += 50;
  } else if (hasSubKeyword) {
    score += 40;
  }

  // Hint matching
  if (
    !isSingleLatin &&
    hint &&
    (hint.includes(q) || (terms.length > 1 && terms.some((term) => hint.includes(term))))
  ) {
    score += 30;
  }

  // Section or Tab title matching
  if (
    !isSingleLatin &&
    (section?.includes(q) ||
      tab?.includes(q) ||
      (terms.length > 1 && terms.some((term) => section.includes(term) || tab.includes(term))))
  ) {
    score += 20;
  }

  // Multi-token fallback when no individual scoring applied but all tokens matched
  if (terms.length > 1 && score === 0) {
    score += 45;
  }

  if (score === 0) {
    return 0;
  }

  // Prefix matches outrank substring matches; substring matches must not exceed prefix threshold
  const hasTitlePrefix =
    title.startsWith(q) || (terms.length > 1 && terms.some((term) => title.startsWith(term)));
  const hasTitleMatch =
    hasTitlePrefix ||
    (!isSingleLatin && title.includes(q)) ||
    (terms.length > 1 && terms.some((term) => title.includes(term)));

  if (hasTitlePrefix) {
    return Math.min(score, 99);
  }
  if (hasTitleMatch) {
    return Math.min(score, 79);
  }
  return Math.min(score, 59);
}

/**
 * Splits text by case-insensitive query substring and wraps matches in
 * `<mark className="settings-search-highlight">`.
 */
export function highlightMatches(text: string, query: string): React.ReactNode {
  const terms = query.trim().split(/\s+/).filter(Boolean);
  if (terms.length === 0 || !text) {
    return text;
  }

  // Unique lowercased terms sorted descending by length to prioritize longer phrases
  const uniqueTerms = Array.from(new Set(terms.map((t) => t.toLowerCase())));
  const pattern = uniqueTerms
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
    .sort((a, b) => b.length - a.length)
    .join('|');

  const regex = new RegExp(`(${pattern})`, 'gi');
  const parts = text.split(regex);
  if (parts.length === 1) {
    return text;
  }

  const nodes: React.ReactNode[] = parts.map((part, index) => {
    if (uniqueTerms.includes(part.toLowerCase())) {
      return React.createElement(
        'mark',
        { key: index, className: 'settings-search-highlight' },
        part
      );
    }
    return part;
  });

  return React.createElement(React.Fragment, null, ...nodes);
}

export interface UseSettingsSearchOptions {
  registry?: readonly SettingsSearchItem[];
}

/**
 * Hook for searching settings across all 14 tabs with global relevance ranking.
 */
export function useSettingsSearch(
  searchQuery: string,
  options?: UseSettingsSearchOptions
): {
  hits: SettingsSearchHit[];
  query: string;
} {
  const { t } = useTranslation();
  const registry = options?.registry ?? SETTINGS_SEARCH_REGISTRY;
  const trimmed = searchQuery.trim();
  const normalizedQuery = trimmed.toLowerCase();

  const hits = useMemo(() => {
    if (!normalizedQuery) {
      return [];
    }

    const matched: SettingsSearchHit[] = [];

    for (const item of registry) {
      const score = calculateItemScore(item, normalizedQuery, t);
      if (score > 0) {
        const displayTitle = resolveDisplayTitle(item, t);

        const rawHint = item.hintKey ? t(item.hintKey, { defaultValue: '' }) : undefined;
        const resolvedHint = rawHint && rawHint !== item.hintKey ? rawHint : undefined;

        const rawSection = item.sectionKey ? t(item.sectionKey, { defaultValue: '' }) : undefined;
        const resolvedSection =
          rawSection && rawSection !== item.sectionKey ? rawSection : undefined;

        const rawTab = t(item.tabKey, { defaultValue: '' });
        const resolvedTab = rawTab && rawTab !== item.tabKey ? rawTab : item.tabKey;

        const rawGroup = t(item.groupKey, { defaultValue: '' });
        const resolvedGroup = rawGroup && rawGroup !== item.groupKey ? rawGroup : item.groupKey;

        matched.push({
          ...item,
          score,
          resolvedTitle: displayTitle,
          resolvedHint,
          resolvedSection,
          resolvedTab,
          resolvedGroup,
        });
      }
    }
    return matched.sort((a, b) => b.score - a.score);
  }, [normalizedQuery, registry, t]);

  return useMemo(
    () => ({
      hits,
      query: trimmed,
    }),
    [hits, trimmed]
  );
}
