import { Check, Download } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import type { EmbySubtitleMode } from '../../../../types/batchQueue';

export interface EmbySubmitBarProps {
  selectedCount: number;
  subtitleMode: EmbySubtitleMode;
  onSubtitleModeChange: (mode: EmbySubtitleMode) => void;
  overwriteExistingExternal: boolean;
  onOverwriteExistingExternalChange: (overwrite: boolean) => void;
  onSubmitBatch: () => void;
  submittedNotice?: string | null;
}

export function EmbySubmitBar({
  selectedCount,
  subtitleMode,
  onSubtitleModeChange,
  overwriteExistingExternal,
  onOverwriteExistingExternalChange,
  onSubmitBatch,
  submittedNotice,
}: EmbySubmitBarProps): React.JSX.Element {
  const { t } = useTranslation();

  return (
    <div className="emby-submit-bar-wrapper">
      {submittedNotice && (
        <div className="emby-submitted-notice" role="status">
          <Check size={14} />
          <span>{submittedNotice}</span>
        </div>
      )}

      <div className="emby-submit-bar">
        <div className="emby-submit-controls">
          <div className="emby-submit-control-item">
            <span className="emby-submit-label">
              {t('emby.subtitle_mode', { defaultValue: 'Subtitle Mode:' })}
            </span>
            <select
              className="emby-select"
              value={subtitleMode}
              onChange={(e) => onSubtitleModeChange(e.target.value as EmbySubtitleMode)}
            >
              <option value="bilingual">
                {t('emby.mode_bilingual', { defaultValue: 'Bilingual Subtitles' })}
              </option>
              <option value="pure_translation">
                {t('emby.mode_pure', { defaultValue: 'Translation Only' })}
              </option>
              <option value="separate_tracks">
                {t('emby.mode_separate', {
                  defaultValue: 'Separate Original & Translation Tracks',
                })}
              </option>
            </select>
          </div>

          <label className="emby-submit-checkbox">
            <input
              type="checkbox"
              checked={overwriteExistingExternal}
              onChange={(e) => onOverwriteExistingExternalChange(e.target.checked)}
            />
            <span>
              {t('emby.overwrite_external', {
                defaultValue: 'Overwrite existing external subtitle of same language',
              })}
            </span>
          </label>
        </div>

        <div className="emby-submit-action">
          <button
            type="button"
            className="btn btn-primary emby-submit-btn"
            disabled={selectedCount === 0}
            onClick={onSubmitBatch}
          >
            <Download size={15} style={{ marginRight: 6 }} />
            <span>
              {t('emby.submit_btn', {
                count: selectedCount,
                defaultValue: `Transcribe and Sync (${selectedCount} selected)`,
              })}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
