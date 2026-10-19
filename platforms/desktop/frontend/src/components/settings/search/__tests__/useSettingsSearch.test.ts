import { renderHook } from '@testing-library/react';
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
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
    expect(scoreZh).toBeGreaterThan(0);

    const enTranslate = (key: string) => {
      const map: Record<string, string> = {
        'settings.language': 'Language',
        'settings.general': 'General',
      };
      return map[key] || '';
    };
    const scoreEn = calculateItemScore(item, 'general language', enTranslate);
    expect(scoreEn).toBeGreaterThan(0);
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
    expect(llmHit!.tab).toBe('llm_service');
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
});
