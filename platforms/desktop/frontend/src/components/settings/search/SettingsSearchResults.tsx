import { SearchX } from 'lucide-react';
import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { SETTINGS_TAB_META } from '../settingsTabMeta';
import { highlightMatches, type SettingsSearchHit } from './useSettingsSearch';

interface SettingsSearchResultItemProps {
  hit: SettingsSearchHit;
  isActive: boolean;
  searchQuery: string;
  index: number;
  onSelectResult: (hit: SettingsSearchHit) => void;
  onHoverResult?: (index: number) => void;
  isKeyboardNavRef: React.MutableRefObject<boolean>;
}

const SettingsSearchResultItem = React.memo(function SettingsSearchResultItem({
  hit,
  isActive,
  searchQuery,
  index,
  onSelectResult,
  onHoverResult,
  isKeyboardNavRef,
}: SettingsSearchResultItemProps) {
  const meta = SETTINGS_TAB_META[hit.tab];
  const Icon = meta?.Icon;

  const breadcrumb = [hit.resolvedGroup, hit.resolvedTab, hit.resolvedSection]
    .filter(Boolean)
    .join(' > ');

  const handleMouseEnter = () => {
    // If user is actively using keyboard navigation, ignore stationary pointer enter events
    if (isKeyboardNavRef.current) {
      return;
    }
    onHoverResult?.(index);
  };

  const handleMouseMove = () => {
    // If user is actively using keyboard navigation or item is already active, ignore movement
    if (isKeyboardNavRef.current || isActive) {
      return;
    }
    onHoverResult?.(index);
  };
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onSelectResult(hit);
    }
  };

  return (
    <div
      id={`settings-search-result-${hit.id}`}
      className="settings-search-result-item"
      data-active={isActive}
      role="option"
      aria-selected={isActive}
      tabIndex={-1}
      onClick={() => onSelectResult(hit)}
      onKeyDown={handleKeyDown}
      onMouseEnter={handleMouseEnter}
      onMouseMove={handleMouseMove}
    >
      <div className="settings-search-result-icon" aria-hidden="true">
        {Icon ? <Icon /> : null}
      </div>
      <div className="settings-search-result-content">
        <span className="settings-search-result-title">
          {highlightMatches(hit.resolvedTitle, searchQuery)}
        </span>
        <span className="settings-search-result-breadcrumb">{breadcrumb}</span>
      </div>
    </div>
  );
});

export interface SettingsSearchResultsProps {
  hits: readonly SettingsSearchHit[];
  searchQuery: string;
  activeIndex: number;
  onSelectResult: (hit: SettingsSearchHit) => void;
  onHoverResult?: (index: number) => void;
}

export function SettingsSearchResults({
  hits,
  searchQuery,
  activeIndex,
  onSelectResult,
  onHoverResult,
}: SettingsSearchResultsProps): React.JSX.Element {
  const { t } = useTranslation();
  const isKeyboardNavRef = useRef(false);
  const resultsContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!searchQuery) {
      return;
    }
    if (resultsContainerRef.current) {
      resultsContainerRef.current.scrollTop = 0;
    }
  }, [searchQuery]);
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        isKeyboardNavRef.current = true;
      }
    };
    const handleMouseMove = (e: MouseEvent) => {
      // Check actual movement to ignore synthetic mousemove events
      if ((e.movementX ?? 0) !== 0 || (e.movementY ?? 0) !== 0) {
        isKeyboardNavRef.current = false;
      }
    };

    window.addEventListener('keydown', handleKeyDown, { capture: true });
    window.addEventListener('mousemove', handleMouseMove, { capture: true });
    return () => {
      window.removeEventListener('keydown', handleKeyDown, { capture: true });
      window.removeEventListener('mousemove', handleMouseMove, { capture: true });
    };
  }, []);

  useEffect(() => {
    // Only scroll into view when navigating via keyboard, avoiding hover jitter
    if (!isKeyboardNavRef.current) {
      return;
    }
    const activeHit = hits[activeIndex];
    if (activeHit) {
      document
        .getElementById(`settings-search-result-${activeHit.id}`)
        ?.scrollIntoView?.({ block: 'nearest' });
    }
  }, [activeIndex, hits]);

  if (hits.length === 0) {
    return (
      <div ref={resultsContainerRef} className="settings-search-results">
        <div className="settings-search-empty">
          <SearchX size={32} className="settings-search-empty-icon" aria-hidden="true" />
          <div className="settings-search-empty-title">
            {t('settings.search_no_results', { defaultValue: 'No settings found' })}
          </div>
          <div className="settings-search-empty-hint">
            {t('settings.search_no_results_hint', {
              defaultValue: 'Try different keywords, such as shortcuts, models, or API',
            })}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div ref={resultsContainerRef} className="settings-search-results">
      <div className="settings-search-header">
        <span id="settings-search-count" className="settings-search-count">
          {t('settings.search_matches_count', {
            count: hits.length,
            defaultValue: `Matches (${hits.length})`,
          })}
        </span>
      </div>

      <div
        id="settings-search-listbox"
        className="settings-search-listbox"
        role="listbox"
        aria-labelledby="settings-search-count"
      >
        {hits.map((hit, index) => (
          <SettingsSearchResultItem
            key={hit.id}
            hit={hit}
            index={index}
            isActive={index === activeIndex}
            searchQuery={searchQuery}
            onSelectResult={onSelectResult}
            onHoverResult={onHoverResult}
            isKeyboardNavRef={isKeyboardNavRef}
          />
        ))}
      </div>
    </div>
  );
}

export default SettingsSearchResults;
