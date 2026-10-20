import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { CJK_REGEX, deriveSearchTerms } from '../../../utils/segmentUtils';
import { foldText } from '../../../utils/textFold';
import {
  groupKeyForTab,
  SETTINGS_SEARCH_REGISTRY,
  type SettingsSearchItem,
} from './settingsSearchRegistry';

export type SettingsSearchHit = SettingsSearchItem & {
  score: number;
  resolvedTitle: string;
  resolvedHint?: string;
  resolvedSection?: string;
  resolvedTab: string;
  resolvedGroup: string;
};

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
export function calculateItemScore(
  item: SettingsSearchItem,
  normalizedQuery: string,
  t: (key: string, options?: Record<string, unknown>) => string
): number {
  const q = foldText(normalizedQuery.trim().toLowerCase());
  if (!q) return 0;

  const isSingleLatin = q.length === 1 && !CJK_REGEX.test(q);

  const displayTitle = resolveDisplayTitle(item, t);
  const title = foldText(displayTitle.toLowerCase());
  const rawHint = item.hintKey ? t(item.hintKey, { defaultValue: '' }) : '';
  const hint = foldText(rawHint && rawHint !== item.hintKey ? rawHint.toLowerCase() : '');
  const rawSection = item.sectionKey ? t(item.sectionKey, { defaultValue: '' }) : '';
  const section = foldText(
    rawSection && rawSection !== item.sectionKey ? rawSection.toLowerCase() : ''
  );
  const rawTab = t(item.tabKey, { defaultValue: '' });
  const tab = foldText(rawTab && rawTab !== item.tabKey ? rawTab.toLowerCase() : '');

  const wsTokens = q.split(/\s+/).filter(Boolean);
  const terms = [...new Set(wsTokens.flatMap((token) => deriveSearchTerms(token)))];
  const keywords = (item.keywords || []).map((keyword) => foldText(keyword.toLowerCase()));

  const matchesKeyword = (term: string) => {
    const termIsSingleLatin = term.length === 1 && !CJK_REGEX.test(term);
    return keywords.some(
      (keyword) =>
        keyword === term ||
        (!termIsSingleLatin &&
          (keyword.includes(term) ||
            (term.length > 2 &&
              keyword.length >= (CJK_REGEX.test(keyword) ? 2 : 3) &&
              term.includes(keyword))))
    );
  };

  const matchesAttribute = (term: string) =>
    title.includes(term) ||
    matchesKeyword(term) ||
    hint.includes(term) ||
    section.includes(term) ||
    tab.includes(term);

  // If multi-token query, every whitespace token must match at least one attribute
  if (wsTokens.length > 1) {
    const allTokensMatch = wsTokens.every((token) =>
      deriveSearchTerms(token).some(matchesAttribute)
    );
    if (!allTokensMatch) {
      return 0;
    }
  }

  // Tier 1: Title starts with the full query, or equals/starts with a full whitespace token.
  // CJK sub-bigrams are excluded so partial n-grams never hijack the prefix tier.
  const hasTitlePrefix =
    title.startsWith(q) ||
    (wsTokens.length > 1 && wsTokens.some((token) => title === token || title.startsWith(token)));

  // Tier 2: Title contains the complete query, or contains a full whitespace token.
  const hasTitleMatch =
    hasTitlePrefix ||
    (!isSingleLatin && title.includes(q)) ||
    (wsTokens.length > 1 &&
      wsTokens.some((token) => {
        const isSingleLatinToken = token.length === 1 && !CJK_REGEX.test(token);
        return !isSingleLatinToken && title.includes(token);
      }));

  let score = 0;

  // Title matching
  if (title === q) {
    score += 100;
  } else if (hasTitlePrefix) {
    score += 80;
  } else if (hasTitleMatch) {
    score += 60;
  } else if (
    terms.length > 1 &&
    terms.some((term) => {
      const isSingleLatinTerm = term.length === 1 && !CJK_REGEX.test(term);
      return !isSingleLatinTerm && title.includes(term);
    })
  ) {
    score += 40;
  }

  // If title is an exact match (+100), it's already the highest relevance match.
  if (title === q) {
    return score;
  }

  // Keyword / alias matching: exact keyword match requires matching the full query q
  // or a full whitespace token, preventing partial CJK bigrams from claiming exact keyword bonus.
  const hasExactKeyword = keywords.some(
    (keyword) =>
      keyword === q || (wsTokens.length > 1 && wsTokens.some((token) => keyword === token))
  );
  const hasSubKeyword =
    !hasExactKeyword &&
    !isSingleLatin &&
    (terms.length > 1
      ? terms.some(matchesKeyword)
      : keywords.some(
          (keyword) =>
            keyword.includes(q) ||
            (q.length > 2 &&
              keyword.length >= (CJK_REGEX.test(keyword) ? 2 : 3) &&
              q.includes(keyword))
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
    (section.includes(q) ||
      tab.includes(q) ||
      (terms.length > 1 && terms.some((term) => section.includes(term) || tab.includes(term))))
  ) {
    score += 20;
  }

  if (score === 0) {
    return 0;
  }

  // Strict tier hierarchy:
  // - Title prefix / exact token (Tier 1): capped at 99
  // - Title full query substring (Tier 2): capped at 79
  // - Partial bigram / keyword / hint / section (Tier 3): strictly capped at 59
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
  const wsTokens = query.trim().split(/\s+/).filter(Boolean);
  const terms = [...new Set(wsTokens.flatMap((token) => deriveSearchTerms(token)))];
  if (terms.length === 0 || !text) {
    return text;
  }

  // NFKC-only folding keeps match offsets aligned with the original string
  const normalizedText = text.normalize('NFKC');
  if (normalizedText.length !== text.length) {
    return text;
  }

  // Unique normalized terms sorted descending by length to prioritize longer phrases
  const uniqueTerms = Array.from(
    new Set(terms.map((term) => term.normalize('NFKC').toLowerCase()).filter(Boolean))
  );
  if (uniqueTerms.length === 0) {
    return text;
  }
  const pattern = uniqueTerms
    .sort((a, b) => b.length - a.length)
    .map((term) => term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
    .join('|');
  const regex = new RegExp(`(${pattern})`, 'gi');
  const nodes: React.ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null = regex.exec(normalizedText);

  while (match !== null) {
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }
    nodes.push(
      React.createElement(
        'mark',
        { key: match.index, className: 'settings-search-highlight' },
        text.slice(match.index, match.index + match[0].length)
      )
    );
    lastIndex = match.index + match[0].length;
    match = regex.exec(normalizedText);
  }

  if (nodes.length === 0) {
    return text;
  }

  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
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

        const rawHint = item.hintKey ? t(item.hintKey, { defaultValue: '' }) : undefined;
        const resolvedHint = rawHint && rawHint !== item.hintKey ? rawHint : undefined;

        const rawSection = item.sectionKey ? t(item.sectionKey, { defaultValue: '' }) : undefined;
        const resolvedSection =
          rawSection && rawSection !== item.sectionKey ? rawSection : undefined;

        const rawTab = t(item.tabKey, { defaultValue: '' });
        const resolvedTab = rawTab && rawTab !== item.tabKey ? rawTab : item.tabKey;

        const groupKey = groupKeyForTab(item.tab);
        const rawGroup = t(groupKey, { defaultValue: '' });
        const resolvedGroup = rawGroup && rawGroup !== groupKey ? rawGroup : groupKey;

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
