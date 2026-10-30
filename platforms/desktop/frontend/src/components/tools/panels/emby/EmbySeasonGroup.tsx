import { ChevronDown, ChevronRight } from 'lucide-react';
import type React from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { EmbyMediaItem } from '../../../../types/emby';
import { EmbyMediaRow } from './EmbyMediaRow';
import { hasTargetSubtitle } from './embyLanguageUtils';

export interface EmbySeasonGroupProps {
  seasonKey: string;
  seasonItems: EmbyMediaItem[];
  isExpanded: boolean;
  onToggleExpand: (key: string) => void;
  targetLanguage: string;
  selectedItemIds: Set<string>;
  queuedItemIds: Set<string>;
  audioTrackMap: Record<string, number>;
  onToggleSelect: (id: string) => void;
  onAudioTrackChange: (itemId: string, streamIndex: number) => void;
  onSelectAllMissingInSeason: (items: EmbyMediaItem[]) => void;
}

export function EmbySeasonGroup({
  seasonKey,
  seasonItems,
  isExpanded,
  onToggleExpand,
  targetLanguage,
  selectedItemIds,
  queuedItemIds,
  audioTrackMap,
  onToggleSelect,
  onAudioTrackChange,
  onSelectAllMissingInSeason,
}: EmbySeasonGroupProps): React.JSX.Element {
  const { t } = useTranslation();

  const missingCount = useMemo(
    () =>
      seasonItems.filter((item) => !hasTargetSubtitle(item.mediaStreams, targetLanguage)).length,
    [seasonItems, targetLanguage]
  );

  const selectableMissingCount = useMemo(
    () =>
      seasonItems.filter(
        (item) =>
          !hasTargetSubtitle(item.mediaStreams, targetLanguage) && !queuedItemIds.has(item.id)
      ).length,
    [seasonItems, targetLanguage, queuedItemIds]
  );

  return (
    <div className="emby-season-group">
      <div className="emby-season-header">
        <div
          className="emby-season-title"
          onClick={() => onToggleExpand(seasonKey)}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onToggleExpand(seasonKey);
            }
          }}
          aria-expanded={isExpanded}
        >
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <span className="emby-season-name">{seasonKey}</span>
          <span className="emby-season-count">
            {t('emby.episodes_count', {
              count: seasonItems.length,
              defaultValue: `(${seasonItems.length} episodes)`,
            })}
          </span>
          {missingCount > 0 ? (
            <span className="emby-badge emby-badge-missing">
              {t('emby.missing_target_count', {
                count: missingCount,
                defaultValue: `${missingCount} missing`,
              })}
            </span>
          ) : (
            <span className="emby-badge emby-badge-complete">
              {t('emby.all_have_target', { defaultValue: 'All have subtitles' })}
            </span>
          )}
        </div>

        <div className="emby-season-actions">
          <button
            type="button"
            className="btn btn-xs btn-secondary"
            onClick={() => onSelectAllMissingInSeason(seasonItems)}
            disabled={selectableMissingCount === 0}
          >
            {t('emby.select_season_missing', {
              defaultValue: 'Select Missing Episodes in Season',
            })}
          </button>
        </div>
      </div>

      {isExpanded && (
        <div className="emby-season-episodes">
          {seasonItems.map((item) => (
            <EmbyMediaRow
              key={item.id}
              item={item}
              targetLanguage={targetLanguage}
              isSelected={selectedItemIds.has(item.id)}
              isQueued={queuedItemIds.has(item.id)}
              selectedAudioIndex={audioTrackMap[item.id]}
              onToggleSelect={onToggleSelect}
              onAudioTrackChange={onAudioTrackChange}
            />
          ))}
        </div>
      )}
    </div>
  );
}
