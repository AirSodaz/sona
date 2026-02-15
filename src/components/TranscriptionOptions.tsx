import React from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';

/** Props for TranscriptionOptions component. */
interface TranscriptionOptionsProps {
    enableTimeline: boolean;
    setEnableTimeline: (value: boolean) => void;
    language: string;
    setLanguage: (value: string) => void;
    className?: string;
    disabled?: boolean;
}

/**
 * Component for configuring transcription options (Subtitle Mode, Language).
 * Shared between Batch Import and Live Recording.
 *
 * @param props Component props.
 * @return The rendered options component.
 */
export const TranscriptionOptions = React.memo(function TranscriptionOptions({
    enableTimeline,
    setEnableTimeline,
    language,
    setLanguage,
    className = '',
    disabled = false
}: TranscriptionOptionsProps): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <div className={`options-container ${className}`}>
            <div className="options-row">
                <div className="options-label">
                    <span>{t('batch.timeline_mode')}</span>
                    <span className="options-hint">{t('batch.timeline_hint')}</span>
                </div>
                <button
                    className={`toggle-switch ${disabled ? 'disabled' : ''}`}
                    onClick={() => !disabled && setEnableTimeline(!enableTimeline)}
                    role="switch"
                    aria-checked={enableTimeline}
                    aria-label={t('batch.timeline_mode')}
                    data-tooltip={t('batch.timeline_mode_tooltip')}
                    data-tooltip-pos="left"
                    disabled={disabled}
                >
                    <div className="toggle-switch-handle" />
                </button>
            </div>

            <div className="options-row">
                <div className="options-label">
                    <span>{t('batch.language')}</span>
                    <span className="options-hint">{t('batch.language_hint')}</span>
                </div>
                <Dropdown
                    value={language}
                    onChange={(val) => !disabled && setLanguage(val)}
                    options={[
                        { value: 'auto', label: 'Auto' },
                        { value: 'zh', label: 'Chinese' },
                        { value: 'en', label: 'English' },
                        { value: 'ja', label: 'Japanese' },
                        { value: 'ko', label: 'Korean' },
                        { value: 'yue', label: 'Cantonese' }
                    ]}
                    style={{ width: '180px', opacity: disabled ? 0.6 : 1, pointerEvents: disabled ? 'none' : 'auto' }}
                />
            </div>
        </div>
    );
});
