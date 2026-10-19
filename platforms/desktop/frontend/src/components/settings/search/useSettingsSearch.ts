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

  const displayTitle = resolveDisplayTitle(item, t);
  const title = (displayTitle || '').toLowerCase();
  const rawHint = item.hintKey ? t(item.hintKey, { defaultValue: '' }) : '';
  const hint = (rawHint || '').toLowerCase();
  const rawSection = item.sectionKey ? t(item.sectionKey, { defaultValue: '' }) : '';
  const section = (rawSection || '').toLowerCase();
  const rawTab = t(item.tabKey, { defaultValue: '' });
  const tab = (rawTab || '').toLowerCase();

  let score = 0;

  // Title matching
  if (title === q) {
    score += 100;
  } else if (title.startsWith(q)) {
    score += 80;
  } else if (title.includes(q)) {
    score += 60;
  }

  // If title is an exact match (+100), it's already the highest relevance match.
  if (title === q) {
    return score;
  }

  // Keyword / alias matching
  const keywords = (item.keywords || []).map((k) => k.toLowerCase());
  const terms = q.split(/\s+/).filter(Boolean);

  const matchesKeyword = (term: string) =>
    keywords.some((k) => k === term || k.includes(term) || term.includes(k));

  const hasExactKeyword = keywords.some((k) => k === q);
  const hasSubKeyword =
    !hasExactKeyword &&
    (keywords.some((k) => k.includes(q) || q.includes(k)) ||
      (terms.length > 1 && terms.every(matchesKeyword)));

  if (hasExactKeyword) {
    score += 50;
  } else if (hasSubKeyword) {
    score += 40;
  }

  // Hint matching
  if (
    hint &&
    (hint.includes(q) || (terms.length > 1 && terms.every((term) => hint.includes(term))))
  ) {
    score += 30;
  }

  // Section or Tab title matching
  if (
    section?.includes(q) ||
    tab?.includes(q) ||
    (terms.length > 1 &&
      (terms.every((term) => section.includes(term)) || terms.every((term) => tab.includes(term))))
  ) {
    score += 20;
  }

  // Multi-token match across title + keywords + hint
  if (terms.length > 1 && score === 0) {
    const allTokensMatch = terms.every(
      (term) => title.includes(term) || matchesKeyword(term) || hint.includes(term)
    );
    if (allTokensMatch) {
      score += 45;
    }
  }

  if (score === 0) {
    return 0;
  }

  // Cap matches lacking a direct title match below title-prefix threshold (80)
  // so items whose visible title starts with the query always outrank secondary metadata hits
  const hasTitleMatch = title.startsWith(q) || title.includes(q);
  return Math.min(score, hasTitleMatch ? 99 : 79);
}
/**
 * Splits text by case-insensitive query substring and wraps matches in
 * `<mark className="settings-search-highlight">`.
 */
export function highlightMatches(text: string, query: string): React.ReactNode {
  const trimmed = query.trim();
  if (!trimmed || !text) {
    return text;
  }

  const lowerText = text.toLowerCase();
  const lowerQuery = trimmed.toLowerCase();
  const firstIndex = lowerText.indexOf(lowerQuery);

  if (firstIndex === -1) {
    return text;
  }

  const nodes: React.ReactNode[] = [];
  let currentIndex = 0;

  while (currentIndex < text.length) {
    const matchIndex = lowerText.indexOf(lowerQuery, currentIndex);
    if (matchIndex === -1) {
      nodes.push(text.slice(currentIndex));
      break;
    }

    if (matchIndex > currentIndex) {
      nodes.push(text.slice(currentIndex, matchIndex));
    }

    const matchEnd = matchIndex + lowerQuery.length;
    nodes.push(
      React.createElement(
        'mark',
        { key: matchIndex, className: 'settings-search-highlight' },
        text.slice(matchIndex, matchEnd)
      )
    );

    currentIndex = matchEnd;
  }

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

        matched.push({
          ...item,
          score,
          resolvedTitle: displayTitle,
          resolvedHint: item.hintKey ? t(item.hintKey, { defaultValue: '' }) : undefined,
          resolvedSection: item.sectionKey ? t(item.sectionKey, { defaultValue: '' }) : undefined,
          resolvedTab: t(item.tabKey, { defaultValue: item.tabKey }),
          resolvedGroup: t(item.groupKey, { defaultValue: item.groupKey }),
        });
      }
    }
    return matched.sort((a, b) => b.score - a.score);
  }, [normalizedQuery, registry, t]);

  return {
    hits,
    query: trimmed,
  };
}
