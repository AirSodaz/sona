import { SearchX } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { SETTINGS_TAB_META } from '../settingsTabMeta';
import { highlightMatches, type SettingsSearchHit } from './useSettingsSearch';

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

  if (hits.length === 0) {
    return (
      <div className="settings-search-results">
        <div className="settings-search-empty" role="status" aria-live="polite">
          <SearchX size={32} className="settings-search-empty-icon" aria-hidden="true" />
          <div className="settings-search-empty-title">
            {t('settings.search_no_results', { defaultValue: '未找到相关设置' })}
          </div>
          <div className="settings-search-empty-hint">
            {t('settings.search_no_results_hint', {
              defaultValue: '请尝试其他关键词，如快捷键、模型或 API',
            })}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className="settings-search-results"
      role="listbox"
      aria-label={t('settings.search_matches_count', {
        count: hits.length,
        defaultValue: `匹配项 (${hits.length})`,
      })}
    >
      <div className="settings-search-header">
        <span className="settings-search-count">
          {t('settings.search_matches_count', {
            count: hits.length,
            defaultValue: `匹配项 (${hits.length})`,
          })}
        </span>
      </div>

      {hits.map((hit, index) => {
        const meta = SETTINGS_TAB_META[hit.tab];
        const Icon = meta?.Icon;
        const isActive = index === activeIndex;

        const breadcrumbParts = [hit.resolvedGroup, hit.resolvedTab];
        if (hit.resolvedSection) {
          breadcrumbParts.push(hit.resolvedSection);
        }
        const breadcrumb = breadcrumbParts.join(' > ');

        return (
          <button
            key={hit.id}
            type="button"
            className={`settings-search-result-item ${isActive ? 'is-active' : ''}`}
            data-active={isActive}
            role="option"
            aria-selected={isActive}
            tabIndex={-1}
            onClick={() => onSelectResult(hit)}
            onMouseEnter={() => onHoverResult?.(index)}
          >
            <div className="settings-search-result-icon" aria-hidden="true">
              {Icon ? <Icon /> : null}
            </div>
            <div className="settings-search-result-content">
              <span className="settings-search-result-title">
                {highlightMatches(hit.resolvedTitle, searchQuery)}
              </span>
              <span className="settings-search-result-breadcrumb" title={breadcrumb}>
                {breadcrumb}
              </span>
            </div>
          </button>
        );
      })}
    </div>
  );
}

export default SettingsSearchResults;
