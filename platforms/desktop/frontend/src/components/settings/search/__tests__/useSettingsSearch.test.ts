import { renderHook } from '@testing-library/react';
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { deriveSearchTerms } from '../../../../utils/segmentUtils';
import { SETTINGS_SEARCH_REGISTRY } from '../settingsSearchRegistry';
import { calculateItemScore, highlightMatches, useSettingsSearch } from '../useSettingsSearch';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => {
      const translations: Record<string, string> = {
        'settings.language': '语言',
        'settings.language_hint': '修改界面显示语言',
        'settings.general': '常规设置',
        'settings.theme': '外观主题',
        'settings.llm.title': '大模型服务',
        'settings.llm.credentials_section': '模型供应商',
        'settings.llm.credentials_hint': '配置供应商密钥与接口',
        'settings.llm.api_key': 'API 密钥',
        'settings.llm.base_url': '基础 URL',
      };
      if (key in translations) return translations[key];
      if (options?.defaultValue !== undefined) return options.defaultValue;
      return key;
    },
  }),
}));

describe('calculateItemScore', () => {
  const mockTranslate = (key: string, options?: { defaultValue?: string }) => {
    const map: Record<string, string> = {
      'settings.language': '语言',
      'settings.language_hint': '修改界面显示语言',
      'settings.general': '常规',
      'settings.llm.title': '大模型服务',
      'settings.llm.credentials_section': '供应商凭据',
    };
    if (key in map) return map[key];
    if (options?.defaultValue !== undefined) return options.defaultValue;
    return key;
  };

  it('exact query matches: "语言" returns general-language as top hit with score 100', () => {
    const item = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'general-language')!;
    expect(item).toBeDefined();

    const score = calculateItemScore(item, '语言', mockTranslate);
    expect(score).toBe(100);
  });

  it('keyword matches: "DeepSeek" returns LLM service item with positive score', () => {
    const llmItem = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'llm-providers')!;
    expect(llmItem).toBeDefined();

    const score = calculateItemScore(llmItem, 'DeepSeek', mockTranslate);
    expect(score).toBeGreaterThanOrEqual(50);
  });

  it('returns 0 for empty or whitespace query', () => {
    const item = SETTINGS_SEARCH_REGISTRY[0];
    expect(calculateItemScore(item, '', mockTranslate)).toBe(0);
    expect(calculateItemScore(item, '   ', mockTranslate)).toBe(0);
  });

  it('supports compound CJK queries where query contains keyword', () => {
    const shortcutItem = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'shortcuts-record')!;
    expect(shortcutItem).toBeDefined();

    // User searches for "快捷键设置" while keyword is "快捷键"
    const score = calculateItemScore(shortcutItem, '快捷键设置', mockTranslate);
    expect(score).toBeGreaterThan(0);
  });

  it('supports multi-word query tokens', () => {
    const llmItem = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'llm-api-keys')!;
    expect(llmItem).toBeDefined();

    const score = calculateItemScore(llmItem, 'api key', mockTranslate);
    expect(score).toBeGreaterThan(0);
  });

  it('caps items lacking title matches at 59 below title prefix threshold (80)', () => {
    const item = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'llm-providers')!;
    // Query matches keyword + hint + tab but NOT title
    const score = calculateItemScore(item, 'deepseek', mockTranslate);
    expect(score).toBeLessThanOrEqual(59);
  });

  it('strictly maintains tier hierarchy: title prefix (80-99) > title substring (60-79) > non-title matches (<=59)', () => {
    const prefixItem = {
      id: 'test-prefix',
      titleKey: 'test.prefix',
      tabKey: 'test.tab',
      groupKey: 'test.group',
      tab: 'general' as const,
      keywords: ['foo'],
    };
    const substringItem = {
      id: 'test-substring',
      titleKey: 'test.substring',
      tabKey: 'test.tab',
      groupKey: 'test.group',
      tab: 'general' as const,
      keywords: ['foo'],
    };
    const keywordItem = {
      id: 'test-keyword',
      titleKey: 'test.other',
      hintKey: 'test.hint',
      tabKey: 'test.tab',
      groupKey: 'test.group',
      tab: 'general' as const,
      keywords: ['foobar', 'extra keyword'],
    };

    const customTranslate = (key: string, options?: { defaultValue?: string }) => {
      if (key === 'test.prefix') return 'Foobar Setting';
      if (key === 'test.substring') return 'Special Foobar Setting';
      if (key === 'test.other') return 'Completely Unrelated';
      if (key === 'test.hint') return 'Mentions foobar here';
      if (key === 'test.tab') return 'foobar tab';
      if (options?.defaultValue !== undefined) return options.defaultValue;
      return key;
    };

    const prefixScore = calculateItemScore(prefixItem, 'foobar', customTranslate);
    const substringScore = calculateItemScore(substringItem, 'foobar', customTranslate);
    const keywordScore = calculateItemScore(keywordItem, 'foobar', customTranslate);

    expect(prefixScore).toBeGreaterThanOrEqual(80);
    expect(prefixScore).toBeLessThanOrEqual(99);

    expect(substringScore).toBeGreaterThanOrEqual(60);
    expect(substringScore).toBeLessThanOrEqual(79);

    expect(keywordScore).toBeLessThanOrEqual(59);

    expect(prefixScore).toBeGreaterThan(substringScore);
    expect(substringScore).toBeGreaterThan(keywordScore);
  });

  it('does not match short Latin keyword (<=2 chars like "ai") against English words like "email", "detail", "main"', () => {
    const aiItem = {
      id: 'test-ai',
      titleKey: 'test.title',
      tabKey: 'test.tab',
      groupKey: 'test.group',
      tab: 'subtitle' as const,
      subTab: 'voice_typing' as const,
      keywords: ['ai', 'voice typing'],
    };
    const t = (k: string) => (k === 'test.title' ? 'Speech To Text' : '');

    expect(calculateItemScore(aiItem, 'email', t)).toBe(0);
    expect(calculateItemScore(aiItem, 'detail', t)).toBe(0);
    expect(calculateItemScore(aiItem, 'main', t)).toBe(0);
    // But exact match for "ai" still matches
    expect(calculateItemScore(aiItem, 'ai', t)).toBeGreaterThan(0);
  });

  it('supports multi-token queries combining tab and title', () => {
    const item = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'general-language')!;
    expect(item).toBeDefined();

    const scoreZh = calculateItemScore(item, '常规 语言', mockTranslate);
    expect(scoreZh).toBeGreaterThanOrEqual(80);

    const enTranslate = (key: string) => {
      const map: Record<string, string> = {
        'settings.language': 'Language',
        'settings.general': 'General',
      };
      return map[key] || '';
    };
    const scoreEn = calculateItemScore(item, 'general language', enTranslate);
    expect(scoreEn).toBeGreaterThanOrEqual(80);
  });
  it('handles queries containing regex special characters safely without throwing', () => {
    const item = SETTINGS_SEARCH_REGISTRY[0];
    const regexQuery = '[test] (.*)+?^$\\';
    expect(() => calculateItemScore(item, regexQuery, mockTranslate)).not.toThrow();
    expect(() => highlightMatches('Some Text [test]', regexQuery)).not.toThrow();
  });

  it('does not match uninitialized/missing translation keys against "settings"', () => {
    const uninitializedItem = {
      id: 'test-item',
      titleKey: 'settings.untranslated.title',
      hintKey: 'settings.untranslated.hint',
      sectionKey: 'settings.untranslated.section',
      tabKey: 'settings.untranslated.tab',
      groupKey: 'settings.untranslated.group',
      tab: 'general' as const,
    };
    // When t() returns the key itself as fallback
    const rawKeyFallbackTranslate = (key: string) => key;

    const score = calculateItemScore(uninitializedItem, 'settings', rawKeyFallbackTranslate);
    expect(score).toBe(0);
  });

  it('supports Korean Hangul and Japanese Kana compound matching', () => {
    const koItem = {
      id: 'test-ko',
      titleKey: 'test.ko.title',
      tabKey: 'test.tab',
      groupKey: 'test.group',
      tab: 'general' as const,
      keywords: ['언어'],
    };
    const jaItem = {
      id: 'test-ja',
      titleKey: 'test.ja.title',
      tabKey: 'test.tab',
      groupKey: 'test.group',
      tab: 'general' as const,
      keywords: ['キー'],
    };

    const t = () => '';
    // Compound queries containing 2-char Korean Hangul keyword or Japanese Kana keyword
    expect(calculateItemScore(koItem, '언어설정', t)).toBeGreaterThan(0);
    expect(calculateItemScore(jaItem, 'キー設定', t)).toBeGreaterThan(0);
  });

  it('does not flood matches for single-character Latin queries', () => {
    const item = {
      id: 'test-latin',
      titleKey: 'test.title',
      hintKey: 'test.hint',
      tabKey: 'test.tab',
      sectionKey: 'test.section',
      groupKey: 'test.group',
      tab: 'general' as const,
      keywords: ['automation'],
    };
    const t = (k: string) => {
      if (k === 'test.title') return 'General Settings';
      if (k === 'test.hint') return 'Manage your audio configurations';
      if (k === 'test.tab') return 'Appearance';
      if (k === 'test.section') return 'Advanced';
      return '';
    };

    // "a" is present as substring in title ("General"), hint ("Manage"), tab ("Appearance"), section ("Advanced"), keyword ("automation")
    // But none start with "a" except section/tab which should be ignored for single Latin characters
    // Title starts with "G", not "a".
    expect(calculateItemScore(item, 'a', t)).toBe(0);
  });

  it('matches multi-concept CJK queries through bigrams', () => {
    const languageItem = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'general-language')!;
    const themeItem = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'general-theme')!;

    expect(calculateItemScore(languageItem, '语言主题', mockTranslate)).toBeGreaterThan(0);
    expect(calculateItemScore(themeItem, '语言主题', mockTranslate)).toBeGreaterThan(0);
  });

  it('matches Japanese Kana bigrams against translated tab and hint text', () => {
    const syncItem = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'sync-credentials')!;
    const jaTranslate = (key: string) => {
      const map: Record<string, string> = {
        'settings.sync.title': 'クラウド同期',
        'settings.sync.storage_type_label': 'ストレージタイプ',
        'settings.sync.storage_type_hint': 'パスワードとサーバーを設定',
      };
      return map[key] ?? '';
    };

    expect(calculateItemScore(syncItem, '同期パスワード', jaTranslate)).toBeGreaterThan(0);
  });

  it('folds diacritics and full-width characters', () => {
    const cafeItem = {
      id: 'test-cafe',
      titleKey: 'test.title',
      tabKey: 'test.tab',
      tab: 'general' as const,
    };
    const apiItem = {
      id: 'test-api',
      titleKey: 'test.title',
      tabKey: 'test.tab',
      tab: 'general' as const,
    };
    const cafeTranslate = (key: string) => (key === 'test.title' ? 'Café Settings' : '');
    const apiTranslate = (key: string) => (key === 'test.title' ? 'API Settings' : '');

    expect(calculateItemScore(cafeItem, 'cafe', cafeTranslate)).toBeGreaterThan(0);
    expect(calculateItemScore(apiItem, 'ＡＰＩ', apiTranslate)).toBeGreaterThan(0);
  });

  it('searching Japanese Katakana words with ー matches expected items and does not score unrelated items', () => {
    const serverItem = {
      id: 'server-config',
      titleKey: 'settings.server',
      tabKey: 'settings.general',
      tab: 'general' as const,
    };
    const unrelatedItem = {
      id: 'unrelated',
      titleKey: 'settings.other',
      tabKey: 'settings.general',
      tab: 'general' as const,
    };
    const jaTranslate = (key: string) => {
      const map: Record<string, string> = {
        'settings.server': 'サーバー設定',
        'settings.other': 'ログ-管理 (詳細)',
      };
      return map[key] ?? '';
    };

    expect(calculateItemScore(serverItem, 'サーバー', jaTranslate)).toBeGreaterThan(0);
    expect(calculateItemScore(unrelatedItem, 'サーバー', jaTranslate)).toBe(0);
  });

  it('prevents CJK bigram splitting from causing prefix scoring inversion', () => {
    const fullPrefixItem = {
      id: 'full-prefix',
      titleKey: 'title.full_prefix',
      tabKey: 'tab.key',
      tab: 'general' as const,
    };
    const fullSubstringItem = {
      id: 'full-substring',
      titleKey: 'title.full_substring',
      tabKey: 'tab.key',
      tab: 'general' as const,
    };
    const bigramPrefixItem = {
      id: 'bigram-prefix',
      titleKey: 'title.bigram_prefix',
      tabKey: 'tab.key',
      tab: 'general' as const,
      keywords: ['快捷'], // extra keyword matching the sub-bigram
    };
    const exactTitleItem = {
      id: 'exact-title',
      titleKey: 'title.exact',
      tabKey: 'tab.key',
      tab: 'general' as const,
    };

    const t = (key: string) => {
      const map: Record<string, string> = {
        'title.exact': '快捷键',
        'title.full_prefix': '快捷键设置',
        'title.full_substring': '自定义快捷键',
        'title.bigram_prefix': '快捷方式管理',
        'tab.key': '常规',
      };
      return map[key] ?? '';
    };

    const exactScore = calculateItemScore(exactTitleItem, '快捷键', t);
    const prefixScore = calculateItemScore(fullPrefixItem, '快捷键', t);
    const substringScore = calculateItemScore(fullSubstringItem, '快捷键', t);
    const bigramScore = calculateItemScore(bigramPrefixItem, '快捷键', t);

    // Exact title is definitive 100
    expect(exactScore).toBe(100);

    // Full query prefix match belongs to Tier 1 [80, 99]
    expect(prefixScore).toBeGreaterThanOrEqual(80);
    expect(prefixScore).toBeLessThanOrEqual(99);

    // Full query substring match belongs to Tier 2 [60, 79]
    expect(substringScore).toBeGreaterThanOrEqual(60);
    expect(substringScore).toBeLessThanOrEqual(79);

    // Sub-bigram prefix match must NEVER enter Tier 1 or Tier 2; strictly capped in Tier 3 (<= 59)
    expect(bigramScore).toBeLessThanOrEqual(59);

    // Clean monotonic tier hierarchy: Exact (100) > Full Prefix (80..99) > Full Substring (60..79) > Sub-bigram (<=59)
    expect(exactScore).toBeGreaterThan(prefixScore);
    expect(prefixScore).toBeGreaterThan(substringScore);
    expect(substringScore).toBeGreaterThan(bigramScore);
  });

  it('does not treat partial CJK bigrams as exact keyword matches', () => {
    const exactKeywordItem = {
      id: 'item-exact-keyword',
      titleKey: 'title.other',
      tabKey: 'tab.key',
      tab: 'general' as const,
      keywords: ['快捷键设置'],
    };
    const subBigramKeywordItem = {
      id: 'item-sub-keyword',
      titleKey: 'title.other',
      tabKey: 'tab.key',
      tab: 'general' as const,
      keywords: ['快捷'],
    };

    const t = () => '';
    const exactScore = calculateItemScore(exactKeywordItem, '快捷键设置', t);
    const subScore = calculateItemScore(subBigramKeywordItem, '快捷键设置', t);

    expect(exactScore).toBe(50); // Exact keyword match (+50)
    expect(subScore).toBe(40); // Sub-keyword match (+40)
    expect(exactScore).toBeGreaterThan(subScore);
  });
});

describe('useSettingsSearch hook', () => {
  it('returns empty array when query is empty', () => {
    const { result } = renderHook(() => useSettingsSearch(''));
    expect(result.current.hits).toEqual([]);
    expect(result.current.query).toBe('');
  });

  it('exact query matches: "语言" returns general-language as top hit with score 100', () => {
    const { result } = renderHook(() => useSettingsSearch('语言'));
    expect(result.current.hits.length).toBeGreaterThan(0);
    const topHit = result.current.hits[0];
    expect(topHit.id).toBe('general-language');
    expect(topHit.score).toBe(100);
  });

  it('substring & keyword matches: "DeepSeek" returns LLM service item', () => {
    const { result } = renderHook(() => useSettingsSearch('DeepSeek'));
    expect(result.current.hits.length).toBeGreaterThan(0);
    const llmHit = result.current.hits.find((h) => h.id === 'llm-providers');
    expect(llmHit).toBeDefined();
    expect(llmHit!.tab).toBe('capabilities');
  });

  it('sorts hits descending by score', () => {
    const { result } = renderHook(() => useSettingsSearch('语言'));
    const hits = result.current.hits;
    for (let i = 1; i < hits.length; i++) {
      expect(hits[i - 1].score).toBeGreaterThanOrEqual(hits[i].score);
    }
  });

  it('ranks exact title matches higher than multiple partial/additive matches', () => {
    const { result } = renderHook(() => useSettingsSearch('语言'));
    expect(result.current.hits[0].id).toBe('general-language');
    expect(result.current.hits[0].score).toBe(100);
    for (let i = 1; i < result.current.hits.length; i++) {
      expect(result.current.hits[i].score).toBeLessThanOrEqual(99);
    }
  });

  it('sanitizes untranslated keys and empty strings to undefined for resolvedHint and resolvedSection', () => {
    const itemWithMissingKeys = {
      id: 'test-untranslated',
      titleKey: 'settings.language',
      hintKey: 'settings.missing.hint',
      sectionKey: 'settings.missing.section',
      tabKey: 'settings.general',
      groupKey: 'settings.group.general',
      tab: 'general' as const,
      keywords: ['lang'],
    };

    const { result } = renderHook(() =>
      useSettingsSearch('lang', { registry: [itemWithMissingKeys] })
    );

    expect(result.current.hits.length).toBe(1);
    const hit = result.current.hits[0];
    expect(hit.resolvedHint).toBeUndefined();
    expect(hit.resolvedSection).toBeUndefined();
  });

  it('ranks full query substring matches strictly above sub-bigram prefix matches in search results', () => {
    // In mock translation:
    // 'settings.theme' -> '外观主题' (contains full query '主题')
    // 'test.bigram' -> defaults to '主卧模式' (starts with bigram '主', but does NOT contain '主题')
    const fullSubstringItem = {
      id: 'theme-setting',
      titleKey: 'settings.theme', // '外观主题'
      tabKey: 'settings.general',
      tab: 'general' as const,
    };
    const subBigramItem = {
      id: 'master-mode',
      titleKey: 'test.bigram',
      tabKey: 'settings.general',
      tab: 'general' as const,
      keywords: ['主题'], // extra keyword
    };

    const { result } = renderHook(() =>
      useSettingsSearch('主题', { registry: [subBigramItem, fullSubstringItem] })
    );

    expect(result.current.hits.length).toBeGreaterThanOrEqual(1);
    expect(result.current.hits[0].id).toBe('theme-setting');
  });
});

describe('highlightMatches', () => {
  it('returns plain text when query is empty', () => {
    expect(highlightMatches('Hello World', '')).toBe('Hello World');
    expect(highlightMatches('Hello World', '   ')).toBe('Hello World');
  });

  it('returns plain text when no match is found', () => {
    expect(highlightMatches('Hello World', 'xyz')).toBe('Hello World');
  });

  it('returns <mark> elements around matching substring', () => {
    const rendered = highlightMatches('Application Language', 'lang');
    expect(React.isValidElement(rendered)).toBe(true);

    const fragment = rendered as React.ReactElement<{ children: React.ReactNode[] }>;
    const children = React.Children.toArray(fragment.props.children);

    expect(children.length).toBe(3);
    expect(children[0]).toBe('Application ');
    const mark = children[1] as React.ReactElement<{ className: string; children: string }>;
    expect(mark.type).toBe('mark');
    expect(mark.props.className).toBe('settings-search-highlight');
    expect(mark.props.children).toBe('Lang');
    expect(children[2]).toBe('uage');
  });

  it('highlights multiple occurrences', () => {
    const rendered = highlightMatches('test and test again', 'test');
    const fragment = rendered as React.ReactElement<{ children: React.ReactNode[] }>;
    const children = React.Children.toArray(fragment.props.children);

    const markElements = children.filter((c) => React.isValidElement(c) && c.type === 'mark');
    expect(markElements.length).toBe(2);
  });
  it('highlights multiple distinct query terms', () => {
    const rendered = highlightMatches('API Key Provider', 'api provider');
    expect(React.isValidElement(rendered)).toBe(true);
    const fragment = rendered as React.ReactElement<{ children: React.ReactNode[] }>;
    const children = React.Children.toArray(fragment.props.children);
    const markElements = children.filter((c) => React.isValidElement(c) && c.type === 'mark');
    expect(markElements.length).toBe(2);
  });

  it('highlights full-width query variants using the original text', () => {
    const rendered = highlightMatches('API Settings', 'ＡＰＩ');
    const fragment = rendered as React.ReactElement<{ children: React.ReactNode[] }>;
    const children = React.Children.toArray(fragment.props.children);

    expect(children.length).toBe(2);
    const mark = children[0] as React.ReactElement<{ children: string }>;
    expect(mark.type).toBe('mark');
    expect(mark.props.children).toBe('API');
    expect(children[1]).toBe(' Settings');
  });

  it('CJK compound search "语言主题" renders highlight when matching "应用语言"', () => {
    const rendered = highlightMatches('应用语言', '语言主题');
    expect(React.isValidElement(rendered)).toBe(true);
    const fragment = rendered as React.ReactElement<{ children: React.ReactNode[] }>;
    const children = React.Children.toArray(fragment.props.children);
    const markElements = children.filter((c) => React.isValidElement(c) && c.type === 'mark');
    expect(markElements.length).toBe(1);
    const mark = markElements[0] as React.ReactElement<{ className: string; children: string }>;
    expect(mark.props.className).toBe('settings-search-highlight');
    expect(mark.props.children).toBe('语言');
  });
});

describe('deriveSearchTerms', () => {
  it('strips punctuation and brackets from search terms', () => {
    expect(deriveSearchTerms('(语言)')).toEqual(['语言']);
    expect(deriveSearchTerms('【快捷键】')).toEqual(['快捷键', '快捷', '捷键']);
    expect(deriveSearchTerms('(WebDAV)')).toEqual(['WebDAV']);
    expect(deriveSearchTerms('()')).toEqual([]);
  });
});
