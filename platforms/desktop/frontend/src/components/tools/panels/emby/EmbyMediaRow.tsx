import { Clock, Film } from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../../../../components/Dropdown';
import type { EmbyMediaItem } from '../../../../types/emby';
import { findBestAudioStream, getAudioTrackLabel, hasTargetSubtitle } from './embyLanguageUtils';
export function formatDurationTicks(ticks?: number): string {
  if (!ticks) return '';
  const seconds = Math.floor(ticks / 10_000_000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m`;
  }
  return `${seconds}s`;
}
export interface EmbyMediaRowProps {
  item: EmbyMediaItem;
  targetLanguage: string;
  isSelected: boolean;
  isQueued: boolean;
  selectedAudioIndex?: number;
  onToggleSelect: (id: string) => void;
  onAudioTrackChange: (itemId: string, streamIndex: number) => void;
}

export const EmbyMediaRow = React.memo(function EmbyMediaRow({
  item,
  targetLanguage,
  isSelected,
  isQueued,
  selectedAudioIndex,
  onToggleSelect,
  onAudioTrackChange,
}: EmbyMediaRowProps): React.JSX.Element {
  const { t } = useTranslation();

  const streams = item.mediaStreams ?? [];
  const audios = streams.filter((s) => s.streamType?.toLowerCase() === 'audio');
  const bestAudio = findBestAudioStream(item.mediaStreams);
  const currentAudioIndex = selectedAudioIndex ?? bestAudio?.index ?? audios[0]?.index;
  const hasSub = hasTargetSubtitle(streams, targetLanguage);
  const duration = formatDurationTicks(item.runtimeTicks);
  return (
    <div className={`emby-media-row ${isSelected ? 'selected' : ''} ${isQueued ? 'queued' : ''}`}>
      <div className="emby-item-left">
        <label className="emby-row-checkbox-label">
          <input
            type="checkbox"
            checked={isQueued ? true : isSelected}
            disabled={isQueued}
            onChange={() => onToggleSelect(item.id)}
            aria-label={`Select ${item.name}`}
          />
        </label>

        {item.episodeIndex !== undefined ? (
          <span className="emby-episode-badge">E{item.episodeIndex}</span>
        ) : (
          <span className="emby-movie-icon-badge" aria-hidden="true">
            <Film size={14} />
          </span>
        )}
        <div className="emby-item-title-meta">
          <span className="emby-item-name" title={item.name}>
            {item.episodeIndex !== undefined ? `E${item.episodeIndex} - ${item.name}` : item.name}
          </span>
          {duration && (
            <span className="emby-item-duration">
              <Clock size={11} aria-hidden="true" />
              <span>{duration}</span>
            </span>
          )}
        </div>
      </div>

      <div className="emby-item-right">
        {/* Subtitle status badge */}
        {isQueued ? (
          <span className="emby-status-tag emby-tag-queued">
            <span className="emby-tag-dot" aria-hidden="true" />
            <span>{t('emby.status_in_queue', { defaultValue: 'In Queue' })}</span>
          </span>
        ) : hasSub ? (
          <span className="emby-status-tag emby-tag-present">
            <span className="emby-tag-dot" aria-hidden="true" />
            <span>{t('emby.status_has_sub', { defaultValue: 'Has Subtitles' })}</span>
          </span>
        ) : (
          <span className="emby-status-tag emby-tag-missing">
            <span className="emby-tag-dot" aria-hidden="true" />
            <span>{t('emby.status_missing_sub', { defaultValue: 'Missing Subtitles' })}</span>
          </span>
        )}

        {/* Audio track selector */}
        {audios.length > 0 && (
          <Dropdown
            className="emby-audio-dropdown"
            options={audios.map((stream) => ({
              value: String(stream.index),
              label: getAudioTrackLabel(stream),
            }))}
            value={currentAudioIndex !== undefined ? String(currentAudioIndex) : ''}
            onChange={(val) => onAudioTrackChange(item.id, Number(val))}
            disabled={isQueued}
            aria-label={`${t('emby.select_audio_track', { defaultValue: 'Select Audio Track' })} - ${item.name}`}
          />
        )}
      </div>
    </div>
  );
});
