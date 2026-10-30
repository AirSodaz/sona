import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EmbyLibrary } from '../../../../../types/emby';
import { EmbyLibraryToolbar } from '../EmbyLibraryToolbar';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: { defaultValue?: string; selected?: number; total?: number }) => {
        if (key === 'emby.selected_counter') {
          return `Selected ${opts?.selected ?? 0} / ${opts?.total ?? 0} items`;
        }
        if (key === 'common.select_all') {
          return 'Select All';
        }
        if (key === 'emby.only_missing_target') {
          return 'Only show items missing subtitles';
        }
        return opts?.defaultValue ?? key;
      },
      i18n: { language: 'en' },
    }),
  };
});

const mockLibraries: EmbyLibrary[] = [
  { id: 'lib-1', name: 'Movies', collectionType: 'movies' },
  { id: 'lib-2', name: 'TV Shows', collectionType: 'tvshows' },
];

describe('EmbyLibraryToolbar', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders library tabs and handles selection', () => {
    const onSelectLib = vi.fn();
    render(
      <EmbyLibraryToolbar
        libraries={mockLibraries}
        selectedLibraryId="lib-1"
        onSelectLibrary={onSelectLib}
        searchTerm=""
        onSearchChange={vi.fn()}
        targetLanguage="zh-CN"
        onTargetLanguageChange={vi.fn()}
        onlyMissingTargetSubtitles={false}
        onOnlyMissingTargetSubtitlesChange={vi.fn()}
        isAllSelected={false}
        isIndeterminate={false}
        selectedCount={0}
        totalSelectableCount={10}
        onToggleSelectAll={vi.fn()}
      />
    );

    const moviesTab = screen.getByRole('tab', { name: /Movies/i });
    const tvTab = screen.getByRole('tab', { name: /TV Shows/i });

    expect(moviesTab.classList.contains('active')).toBe(true);
    expect(tvTab.classList.contains('active')).toBe(false);

    fireEvent.click(tvTab);
    expect(onSelectLib).toHaveBeenCalledWith('lib-2');
  });

  it('handles search input change', () => {
    const onSearch = vi.fn();
    render(
      <EmbyLibraryToolbar
        libraries={mockLibraries}
        selectedLibraryId="lib-1"
        onSelectLibrary={vi.fn()}
        searchTerm="Matrix"
        onSearchChange={onSearch}
        targetLanguage="zh-CN"
        onTargetLanguageChange={vi.fn()}
        onlyMissingTargetSubtitles={false}
        onOnlyMissingTargetSubtitlesChange={vi.fn()}
        isAllSelected={false}
        isIndeterminate={false}
        selectedCount={0}
        totalSelectableCount={5}
        onToggleSelectAll={vi.fn()}
      />
    );

    const searchInput = screen.getByRole('textbox') as HTMLInputElement;
    expect(searchInput.value).toBe('Matrix');

    fireEvent.change(searchInput, { target: { value: 'Inception' } });
    expect(onSearch).toHaveBeenCalledWith('Inception');
  });

  it('handles missing subtitles filter toggle', () => {
    const onFilterChange = vi.fn();
    render(
      <EmbyLibraryToolbar
        libraries={mockLibraries}
        selectedLibraryId="lib-1"
        onSelectLibrary={vi.fn()}
        searchTerm=""
        onSearchChange={vi.fn()}
        targetLanguage="zh-CN"
        onTargetLanguageChange={vi.fn()}
        onlyMissingTargetSubtitles={false}
        onOnlyMissingTargetSubtitlesChange={onFilterChange}
        isAllSelected={false}
        isIndeterminate={false}
        selectedCount={0}
        totalSelectableCount={5}
        onToggleSelectAll={vi.fn()}
      />
    );

    const filterCheckbox = screen.getByLabelText(/Only show items missing subtitles/i);
    expect((filterCheckbox as HTMLInputElement).checked).toBe(false);

    fireEvent.click(filterCheckbox);
    expect(onFilterChange).toHaveBeenCalledWith(true);
  });

  it('renders master checkbox in unchecked state and disables when totalSelectableCount === 0', () => {
    render(
      <EmbyLibraryToolbar
        libraries={mockLibraries}
        selectedLibraryId="lib-1"
        onSelectLibrary={vi.fn()}
        searchTerm=""
        onSearchChange={vi.fn()}
        targetLanguage="zh-CN"
        onTargetLanguageChange={vi.fn()}
        onlyMissingTargetSubtitles={false}
        onOnlyMissingTargetSubtitlesChange={vi.fn()}
        isAllSelected={false}
        isIndeterminate={false}
        selectedCount={0}
        totalSelectableCount={0}
        onToggleSelectAll={vi.fn()}
      />
    );

    const masterCheckbox = screen.getByRole('checkbox', {
      name: /Select All/i,
    }) as HTMLInputElement;
    expect(masterCheckbox.checked).toBe(false);
    expect(masterCheckbox.indeterminate).toBe(false);
    expect(masterCheckbox.disabled).toBe(true);
    expect(screen.getByText('Selected 0 / 0 items')).toBeDefined();
  });

  it('sets indeterminate state on master checkbox and displays counter badge', () => {
    const onToggle = vi.fn();
    render(
      <EmbyLibraryToolbar
        libraries={mockLibraries}
        selectedLibraryId="lib-1"
        onSelectLibrary={vi.fn()}
        searchTerm=""
        onSearchChange={vi.fn()}
        targetLanguage="zh-CN"
        onTargetLanguageChange={vi.fn()}
        onlyMissingTargetSubtitles={false}
        onOnlyMissingTargetSubtitlesChange={vi.fn()}
        isAllSelected={false}
        isIndeterminate={true}
        selectedCount={2}
        totalSelectableCount={5}
        onToggleSelectAll={onToggle}
      />
    );

    const masterCheckbox = screen.getByRole('checkbox', {
      name: /Select All/i,
    }) as HTMLInputElement;
    expect(masterCheckbox.checked).toBe(false);
    expect(masterCheckbox.indeterminate).toBe(true);
    expect(masterCheckbox.disabled).toBe(false);
    expect(screen.getByText('Selected 2 / 5 items')).toBeDefined();

    fireEvent.click(masterCheckbox);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('sets checked state on master checkbox when all items are selected', () => {
    const onToggle = vi.fn();
    render(
      <EmbyLibraryToolbar
        libraries={mockLibraries}
        selectedLibraryId="lib-1"
        onSelectLibrary={vi.fn()}
        searchTerm=""
        onSearchChange={vi.fn()}
        targetLanguage="zh-CN"
        onTargetLanguageChange={vi.fn()}
        onlyMissingTargetSubtitles={false}
        onOnlyMissingTargetSubtitlesChange={vi.fn()}
        isAllSelected={true}
        isIndeterminate={false}
        selectedCount={5}
        totalSelectableCount={5}
        onToggleSelectAll={onToggle}
      />
    );

    const masterCheckbox = screen.getByRole('checkbox', {
      name: /Select All/i,
    }) as HTMLInputElement;
    expect(masterCheckbox.checked).toBe(true);
    expect(masterCheckbox.indeterminate).toBe(false);
    expect(masterCheckbox.disabled).toBe(false);
    expect(screen.getByText('Selected 5 / 5 items')).toBeDefined();

    fireEvent.click(masterCheckbox);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });
});
