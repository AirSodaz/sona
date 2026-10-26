import { fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { SettingsSearchSection } from '../SettingsSearchSection';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number; defaultValue?: string }) => {
      if (key === 'settings.title') return 'Settings';
      if (key === 'settings.search_placeholder') return 'Search settings...';
      if (key === 'settings.search_matches_count') return `Matches (${options?.count ?? 0})`;
      if (key === 'settings.search_no_results') return 'No settings found';
      if (key === 'common.clear_search') return 'Clear search';
      return options?.defaultValue ?? key;
    },
    i18n: { changeLanguage: vi.fn() },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

function renderSection(searchInputRef: React.RefObject<HTMLInputElement | null>) {
  return render(
    <SettingsSearchSection
      inputRef={searchInputRef}
      isOpen={true}
      isMac={false}
      isSidebarScrolled={false}
      onSelectResult={vi.fn()}
      onQueryEmptied={vi.fn()}
    >
      <div data-testid="settings-tabs" />
    </SettingsSearchSection>
  );
}

describe('SettingsSearchSection', () => {
  it('exposes combobox semantics and a persistent live region', () => {
    const inputRef = React.createRef<HTMLInputElement>();
    renderSection(inputRef);

    const input = screen.getByRole('combobox', { name: 'Search settings...' });
    expect(input.getAttribute('type')).toBe('search');
    expect(input.getAttribute('aria-expanded')).toBe('false');
    expect(input.getAttribute('aria-controls')).toBeNull();
    expect(input.getAttribute('aria-haspopup')).toBe('listbox');

    const liveRegion = screen.getByRole('status');
    expect(liveRegion.getAttribute('aria-live')).toBe('polite');
    expect(liveRegion.textContent).toBe('');
    expect(screen.getByTestId('settings-tabs')).toBeDefined();
  });

  it('announces result counts and shows the listbox while searching', () => {
    const inputRef = React.createRef<HTMLInputElement>();
    renderSection(inputRef);

    fireEvent.change(screen.getByRole('combobox'), { target: { value: '语言' } });

    const input = screen.getByRole('combobox');
    expect(input.getAttribute('aria-expanded')).toBe('true');
    expect(input.getAttribute('aria-controls')).toBe('settings-search-listbox');
    expect(input.getAttribute('aria-activedescendant')).toMatch(/^settings-search-result-/);

    expect(screen.queryByTestId('settings-tabs')).toBeNull();
    expect(screen.getAllByRole('option').length).toBeGreaterThan(0);
    expect(screen.getByRole('status').textContent).toMatch(/^Matches \(\d+\)$/);
  });

  it('announces an empty result set', () => {
    const inputRef = React.createRef<HTMLInputElement>();
    renderSection(inputRef);

    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'zzzzzz' } });

    expect(screen.queryAllByRole('option')).toHaveLength(0);
    expect(screen.getByRole('status').textContent).toBe('No settings found');
  });

  it('notifies the parent when the query is emptied', () => {
    const inputRef = React.createRef<HTMLInputElement>();
    const onQueryEmptied = vi.fn();
    render(
      <SettingsSearchSection
        inputRef={inputRef}
        isOpen={true}
        isMac={false}
        isSidebarScrolled={false}
        onSelectResult={vi.fn()}
        onQueryEmptied={onQueryEmptied}
      >
        <div data-testid="settings-tabs" />
      </SettingsSearchSection>
    );

    const input = screen.getByRole('combobox');
    fireEvent.change(input, { target: { value: '语言' } });
    expect(onQueryEmptied).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: '' } });
    expect(onQueryEmptied).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('settings-tabs')).toBeDefined();
  });
});
