import { Check, Download } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown, type DropdownOption } from '../../../../components/Dropdown';
import type { EmbySubtitleMode } from '../../../../types/batchQueue';

export interface EmbySubmitBarProps {
  selectedCount: number;
  subtitleMode: EmbySubtitleMode;
  onSubtitleModeChange: (mode: EmbySubtitleMode) => void;
  overwriteExistingExternal: boolean;
  onOverwriteExistingExternalChange: (overwrite: boolean) => void;
  onSubmitBatch: () => void;
  submittedNotice?: string | null;
  isSubmitting?: boolean;
}

export function EmbySubmitBar({
  selectedCount,
  subtitleMode,
  onSubtitleModeChange,
  overwriteExistingExternal,
  onOverwriteExistingExternalChange,
  onSubmitBatch,
  submittedNotice,
  isSubmitting = false,
}: EmbySubmitBarProps): React.JSX.Element {
  const { t } = useTranslation();

  const subtitleModeOptions: DropdownOption[] = [
    {
      value: 'bilingual',
      label: t('emby.mode_bilingual', { defaultValue: 'Bilingual Subtitles' }),
    },
    {
      value: 'pure_translation',
      label: t('emby.mode_pure', { defaultValue: 'Translation Only' }),
    },
    {
      value: 'separate_tracks',
      label: t('emby.mode_separate', {
        defaultValue: 'Separate Original & Translation Tracks',
      }),
    },
  ];
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
            <Dropdown
              className="emby-dropdown"
              options={subtitleModeOptions}
              value={subtitleMode}
              disabled={isSubmitting}
              onChange={(val) => onSubtitleModeChange(val as EmbySubtitleMode)}
              aria-label={t('emby.subtitle_mode', { defaultValue: 'Subtitle Mode:' })}
            />
          </div>

          <label className="emby-submit-checkbox">
            <input
              type="checkbox"
              checked={overwriteExistingExternal}
              disabled={isSubmitting}
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
            disabled={selectedCount === 0 || isSubmitting}
            onClick={onSubmitBatch}
          >
            <Download size={14} style={{ marginRight: 6 }} />
            <span>
              {t('emby.submit_btn_compact', {
                count: selectedCount,
                defaultValue: `Transcribe (${selectedCount})`,
              })}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
