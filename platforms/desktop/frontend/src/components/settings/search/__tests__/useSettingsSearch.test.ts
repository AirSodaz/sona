import { renderHook } from '@testing-library/react';
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { SETTINGS_SEARCH_REGISTRY } from '../settingsSearchRegistry';
import { calculateItemScore, highlightMatches, useSettingsSearch } from '../useSettingsSearch';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
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
      return translations[key] ?? key;
    },
  }),
}));

describe('calculateItemScore', () => {
  const mockTranslate = (key: string) => {
    const map: Record<string, string> = {
      'settings.language': '语言',
      'settings.language_hint': '修改界面显示语言',
      'settings.general': '常规',
      'settings.llm.title': '大模型服务',
      'settings.llm.credentials_section': '供应商凭据',
    };
    return map[key] ?? key;
  };

  it('exact query matches: "语言" returns general.language as top hit with score 100', () => {
    const item = SETTINGS_SEARCH_REGISTRY.find((i) => i.id === 'general.language')!;
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
});

describe('useSettingsSearch hook', () => {
  it('returns empty array when query is empty', () => {
    const { result } = renderHook(() => useSettingsSearch(''));
    expect(result.current.hits).toEqual([]);
    expect(result.current.query).toBe('');
  });

  it('exact query matches: "语言" returns general.language as top hit with score 100', () => {
    const { result } = renderHook(() => useSettingsSearch('语言'));
    expect(result.current.hits.length).toBeGreaterThan(0);
    const topHit = result.current.hits[0];
    expect(topHit.id).toBe('general.language');
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
});
