import { Search, X } from 'lucide-react';
import React, { useCallback, useEffect, useImperativeHandle, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { isAnyDialogOpen } from '../../../stores/dialogStore';
import { SettingsSearchResults } from './SettingsSearchResults';
import { type SettingsSearchHit, useSettingsSearch } from './useSettingsSearch';

export interface SettingsSearchSectionHandle {
  focusAndSelect: () => void;
  clearIfSearching: () => boolean;
}

interface SettingsSearchSectionProps {
  inputRef: React.RefObject<HTMLInputElement | null>;
  isOpen: boolean;
  isMac: boolean;
  isSidebarScrolled: boolean;
  onSelectResult: (hit: SettingsSearchHit) => void;
  onQueryEmptied: () => void;
  children: React.ReactNode;
}

export const SettingsSearchSection = React.forwardRef<
  SettingsSearchSectionHandle,
  SettingsSearchSectionProps
>(function SettingsSearchSection(
  { inputRef, isOpen, isMac, isSidebarScrolled, onSelectResult, onQueryEmptied, children },
  ref
) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const { hits } = useSettingsSearch(searchQuery);
  const hasQuery = searchQuery.trim().length > 0;

  const announcement = !hasQuery
    ? ''
    : hits.length > 0
      ? t('settings.search_matches_count', {
          count: hits.length,
          defaultValue: `Matches (${hits.length})`,
        })
      : t('settings.search_no_results', { defaultValue: 'No settings found' });

  useEffect(() => {
    if (!isOpen) {
      setSearchQuery('');
      setActiveIndex(0);
    }
  }, [isOpen]);

  const clearQuery = useCallback(() => {
    if (searchQuery.length > 0) {
      onQueryEmptied();
    }
    setSearchQuery('');
    setActiveIndex(0);
  }, [onQueryEmptied, searchQuery]);

  useImperativeHandle(
    ref,
    () => ({
      focusAndSelect: () => {
        inputRef.current?.focus();
        inputRef.current?.select();
      },
      clearIfSearching: () => {
        if (searchQuery.length === 0) {
          return false;
        }
        clearQuery();
        inputRef.current?.focus();
        return true;
      },
    }),
    [clearQuery, inputRef, searchQuery]
  );

  const handleSearchInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) {
      return;
    }

    if (e.key === 'ArrowDown') {
      if (hits.length > 0) {
        e.preventDefault();
        setActiveIndex((prev) => (prev + 1) % hits.length);
      }
    } else if (e.key === 'ArrowUp') {
      if (hits.length > 0) {
        e.preventDefault();
        setActiveIndex((prev) => (prev - 1 + hits.length) % hits.length);
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (hits.length > 0 && hits[activeIndex]) {
        onSelectResult(hits[activeIndex]);
      }
    } else if (e.key === 'Escape') {
      if (isAnyDialogOpen()) {
        return;
      }
      if (searchQuery.length > 0) {
        e.preventDefault();
        e.stopPropagation();
        clearQuery();
      }
    }
  };

  return (
    <>
      <div className={`settings-sidebar-header ${isSidebarScrolled ? 'is-scrolled' : ''}`}>
        <h2 id="settings-title">{t('settings.title')}</h2>
        <div className="settings-search-bar">
          <Search size={14} className="settings-search-icon" aria-hidden="true" />
          <input
            ref={inputRef}
            type="search"
            role="combobox"
            className="settings-search-input"
            aria-label={t('settings.search_placeholder', { defaultValue: 'Search settings...' })}
            placeholder={t('settings.search_placeholder', { defaultValue: 'Search settings...' })}
            aria-expanded={hasQuery}
            aria-haspopup="listbox"
            aria-autocomplete="list"
            aria-controls={hasQuery ? 'settings-search-listbox' : undefined}
            aria-activedescendant={
              hasQuery && hits[activeIndex]
                ? `settings-search-result-${hits[activeIndex].id}`
                : undefined
            }
            value={searchQuery}
            onChange={(e) => {
              const nextQuery = e.target.value;
              if (searchQuery.length > 0 && nextQuery.length === 0) {
                onQueryEmptied();
              }
              setSearchQuery(nextQuery);
              setActiveIndex(0);
            }}
            onKeyDown={handleSearchInputKeyDown}
          />
          {searchQuery ? (
            <button
              type="button"
              className="settings-search-clear"
              onClick={() => {
                clearQuery();
                inputRef.current?.focus();
              }}
              aria-label={t('common.clear_search', { defaultValue: 'Clear search' })}
            >
              <X size={12} />
            </button>
          ) : (
            <kbd className="settings-search-shortcut">{isMac ? '⌘F' : 'Ctrl F'}</kbd>
          )}
        </div>
      </div>
      <div role="status" aria-live="polite" className="settings-search-sr-only">
        {announcement}
      </div>
      {hasQuery ? (
        <SettingsSearchResults
          hits={hits}
          searchQuery={searchQuery}
          activeIndex={activeIndex}
          onSelectResult={onSelectResult}
          onHoverResult={setActiveIndex}
        />
      ) : (
        children
      )}
    </>
  );
});

SettingsSearchSection.displayName = 'SettingsSearchSection';
