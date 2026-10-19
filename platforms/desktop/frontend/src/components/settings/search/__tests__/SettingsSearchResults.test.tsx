import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { SettingsSearchResults } from '../SettingsSearchResults';
import type { SettingsSearchHit } from '../useSettingsSearch';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number; defaultValue?: string }) => {
      if (key === 'settings.search_no_results') return '未找到相关设置';
      if (key === 'settings.search_no_results_hint') return '请尝试其他关键词';
      if (key === 'settings.search_matches_count') return `匹配项 (${options?.count ?? 0})`;
      return options?.defaultValue ?? key;
    },
  }),
}));

const mockHits: SettingsSearchHit[] = [
  {
    id: 'general-language',
    tab: 'general',
    targetId: 'settings-language',
    groupKey: 'settings.group.general',
    tabKey: 'settings.general',
    sectionKey: 'settings.general_title',
    titleKey: 'settings.language',
    score: 100,
    resolvedTitle: '应用语言',
    resolvedTab: '常规',
    resolvedGroup: '通用设置',
    resolvedSection: '基本设置',
  },
  {
    id: 'llm-providers',
    tab: 'llm_service',
    targetId: 'settings-llm-provider-list',
    groupKey: 'settings.group.intelligence',
    tabKey: 'settings.llm.title',
    titleKey: 'settings.llm.credentials_section',
    score: 50,
    resolvedTitle: '模型服务商',
    resolvedTab: '大模型服务',
    resolvedGroup: '智能增强',
  },
];

describe('SettingsSearchResults component', () => {
  it('renders empty state when query matches nothing (hits is empty)', () => {
    render(
      <SettingsSearchResults
        hits={[]}
        searchQuery="nonexistent"
        activeIndex={0}
        onSelectResult={vi.fn()}
      />
    );

    expect(screen.getByText('未找到相关设置')).toBeDefined();
    expect(screen.getByText('请尝试其他关键词')).toBeDefined();
  });

  it('renders correct count badge and breadcrumbs without tooltip attributes', () => {
    const { container } = render(
      <SettingsSearchResults
        hits={mockHits}
        searchQuery="语"
        activeIndex={0}
        onSelectResult={vi.fn()}
      />
    );

    expect(screen.getByText('匹配项 (2)')).toBeDefined();
    expect(screen.getByText('通用设置 > 常规 > 基本设置')).toBeDefined();
    expect(screen.getByText('智能增强 > 大模型服务')).toBeDefined();

    const header = container.querySelector('.settings-search-header');
    expect(header?.getAttribute('role')).toBe('presentation');
    expect(header?.getAttribute('aria-hidden')).toBe('true');

    const breadcrumbElements = container.querySelectorAll('.settings-search-result-breadcrumb');
    expect(breadcrumbElements).toHaveLength(2);
    expect(breadcrumbElements[0]?.getAttribute('title')).toBeNull();
    expect(breadcrumbElements[0]?.getAttribute('data-tooltip')).toBeNull();
    expect(breadcrumbElements[1]?.getAttribute('title')).toBeNull();
    expect(breadcrumbElements[1]?.getAttribute('data-tooltip')).toBeNull();
  });

  it('marks active item based on activeIndex prop', () => {
    render(
      <SettingsSearchResults
        hits={mockHits}
        searchQuery="语"
        activeIndex={1}
        onSelectResult={vi.fn()}
      />
    );

    const buttons = screen.getAllByRole('option');
    expect(buttons[0].getAttribute('data-active')).toBe('false');
    expect(buttons[0].getAttribute('aria-selected')).toBe('false');
    expect(buttons[1].getAttribute('data-active')).toBe('true');
    expect(buttons[1].getAttribute('aria-selected')).toBe('true');
  });
  it('clicking an item triggers onSelectResult callback', () => {
    const onSelectResult = vi.fn();
    render(
      <SettingsSearchResults
        hits={mockHits}
        searchQuery="语"
        activeIndex={0}
        onSelectResult={onSelectResult}
      />
    );

    const buttons = screen.getAllByRole('option');
    fireEvent.click(buttons[0]);

    expect(onSelectResult).toHaveBeenCalledTimes(1);
    expect(onSelectResult).toHaveBeenCalledWith(mockHits[0]);
  });

  it('hovering an item triggers onHoverResult callback', () => {
    const onHoverResult = vi.fn();
    render(
      <SettingsSearchResults
        hits={mockHits}
        searchQuery="语"
        activeIndex={0}
        onSelectResult={vi.fn()}
        onHoverResult={onHoverResult}
      />
    );

    const buttons = screen.getAllByRole('option');
    fireEvent.mouseEnter(buttons[1]);

    expect(onHoverResult).toHaveBeenCalledTimes(1);
    expect(onHoverResult).toHaveBeenCalledWith(1);
  });

  it('highlights search query matches with mark element', () => {
    const { container } = render(
      <SettingsSearchResults
        hits={mockHits}
        searchQuery="语"
        activeIndex={0}
        onSelectResult={vi.fn()}
      />
    );

    const marks = container.querySelectorAll('mark.settings-search-highlight');
    expect(marks.length).toBeGreaterThan(0);
    expect(marks[0].textContent).toBe('语');
  });
});
