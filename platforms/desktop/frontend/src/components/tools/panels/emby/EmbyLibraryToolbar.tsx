import { Film, Search, Tv } from 'lucide-react';
import type React from 'react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown, type DropdownOption } from '../../../../components/Dropdown';
import type { EmbyLibrary } from '../../../../types/emby';

const supportedLanguageOptions: DropdownOption[] = [
  { value: 'zh-CN', label: '简体中文 (zh-CN)' },
  { value: 'zh-TW', label: '繁體中文 (zh-TW)' },
  { value: 'en', label: 'English (en)' },
  { value: 'ja', label: '日本語 (ja)' },
  { value: 'ko', label: '한국어 (ko)' },
];

export interface EmbyLibraryToolbarProps {
  libraries: EmbyLibrary[];
  selectedLibraryId: string | null;
  onSelectLibrary: (id: string) => void;
  searchTerm: string;
  onSearchChange: (term: string) => void;
  targetLanguage: string;
  onTargetLanguageChange: (lang: string) => void;
  onlyMissingTargetSubtitles: boolean;
  onOnlyMissingTargetSubtitlesChange: (onlyMissing: boolean) => void;
  isAllSelected: boolean;
  isIndeterminate: boolean;
  selectedCount: number;
  totalSelectableCount: number;
  onToggleSelectAll: () => void;
}

export function EmbyLibraryToolbar({
  libraries,
  selectedLibraryId,
  onSelectLibrary,
  searchTerm,
  onSearchChange,
  targetLanguage,
  onTargetLanguageChange,
  onlyMissingTargetSubtitles,
  onOnlyMissingTargetSubtitlesChange,
  isAllSelected,
  isIndeterminate,
  selectedCount,
  totalSelectableCount,
  onToggleSelectAll,
}: EmbyLibraryToolbarProps): React.JSX.Element {
  const { t } = useTranslation();
  const indeterminateRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (indeterminateRef.current) {
      indeterminateRef.current.indeterminate = isIndeterminate;
    }
  }, [isIndeterminate]);

  return (
    <div className="emby-library-toolbar">
      {/* 1. Horizontal Library Tabs */}
      {libraries.length > 0 && (
        <div className="emby-library-tabs" role="tablist">
          {libraries.map((lib) => {
            const isSelected = lib.id === selectedLibraryId;
            const isMovie = lib.collectionType === 'movies';
            return (
              <button
                key={lib.id}
                type="button"
                role="tab"
                aria-selected={isSelected}
                className={`emby-library-tab ${isSelected ? 'active' : ''}`}
                onClick={() => onSelectLibrary(lib.id)}
              >
                {isMovie ? <Film size={14} /> : <Tv size={14} />}
                <span>{lib.name}</span>
              </button>
            );
          })}
        </div>
      )}

      {/* 2. Controls: Search (left) & Target Language (right) */}
      <div className="emby-library-controls">
        <div className="emby-controls-row">
          <div className="emby-search-box">
            <Search size={14} className="emby-search-icon" aria-hidden="true" />
            <input
              type="text"
              className="emby-search-input"
              aria-label={t('emby.search_placeholder', {
                defaultValue: 'Search movies and TV shows...',
              })}
              placeholder={t('emby.search_placeholder', {
                defaultValue: 'Search movies and TV shows...',
              })}
              value={searchTerm}
              onChange={(e) => onSearchChange(e.target.value)}
            />
          </div>

          <div className="emby-toolbar-select-group">
            <span className="emby-toolbar-label">
              {t('emby.target_lang', { defaultValue: 'Target Language:' })}
            </span>
            <Dropdown
              className="emby-dropdown"
              options={supportedLanguageOptions}
              value={targetLanguage}
              onChange={onTargetLanguageChange}
              aria-label={t('emby.target_lang', { defaultValue: 'Target Language:' })}
            />
          </div>
        </div>

        {/* 3. Subrow: Filter Zone (left) & Selection Zone (right) */}
        <div className="emby-controls-subrow">
          <div className="emby-filter-zone">
            <label className="emby-toolbar-checkbox">
              <input
                type="checkbox"
                checked={onlyMissingTargetSubtitles}
                onChange={(e) => onOnlyMissingTargetSubtitlesChange(e.target.checked)}
              />
              <span>
                {t('emby.only_missing_target', {
                  defaultValue: 'Only show items missing subtitles',
                })}
              </span>
            </label>
          </div>

          <div className="emby-selection-zone">
            <label className="emby-toolbar-checkbox">
              <input
                type="checkbox"
                ref={indeterminateRef}
                checked={isAllSelected}
                onChange={onToggleSelectAll}
                disabled={totalSelectableCount === 0}
              />
              <span>{t('common.select_all', { defaultValue: 'Select All' })}</span>
            </label>
            <span className="emby-selection-counter">
              {t('emby.selected_counter', {
                selected: selectedCount,
                total: totalSelectableCount,
                defaultValue: `Selected ${selectedCount} / ${totalSelectableCount} items`,
              })}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
