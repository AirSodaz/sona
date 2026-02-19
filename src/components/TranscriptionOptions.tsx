import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsIcon } from './Icons';
import { ParameterSettingsModal } from './ParameterSettingsModal';

/** Props for TranscriptionOptions component. */
interface TranscriptionOptionsProps {
    enableTimeline: boolean;
    setEnableTimeline: (value: boolean) => void;
    language: string;
    setLanguage: (value: string) => void;
    className?: string;
    disabled?: boolean;
    lockWindow?: boolean;
    setLockWindow?: (value: boolean) => void;
    alwaysOnTop?: boolean;
    setAlwaysOnTop?: (value: boolean) => void;
}

/**
 * Component that renders a button to open parameter settings modal.
 * Replaces the inline controls for Subtitle Mode and Language.
 *
 * @param props Component props.
 * @return The rendered component.
 */
export const TranscriptionOptions = React.memo(function TranscriptionOptions({
    enableTimeline,
    setEnableTimeline,
    language,
    setLanguage,
    className = '',
    disabled = false,
    lockWindow,
    setLockWindow,
    alwaysOnTop,
    setAlwaysOnTop
}: TranscriptionOptionsProps): React.JSX.Element {
    const { t } = useTranslation();
    const [isModalOpen, setIsModalOpen] = useState(false);

    return (
        <div
            className={`options-container ${className}`}
            style={{
                marginTop: 'auto', // Push to bottom of flex container
                width: '100%',
                padding: '16px 0 0 0', // Top padding only
                display: 'flex',
                justifyContent: 'center'
            }}
        >
            <button
                className="btn btn-parameter-settings"
                onClick={() => setIsModalOpen(true)}
                disabled={disabled}
                aria-label={t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}
            >
                <SettingsIcon width={16} height={16} />
                <span>{t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}</span>
            </button>

            <ParameterSettingsModal
                isOpen={isModalOpen}
                onClose={() => setIsModalOpen(false)}
                enableTimeline={enableTimeline}
                setEnableTimeline={setEnableTimeline}
                language={language}
                setLanguage={setLanguage}
                disabled={disabled}
                lockWindow={lockWindow}
                setLockWindow={setLockWindow}
                alwaysOnTop={alwaysOnTop}
                setAlwaysOnTop={setAlwaysOnTop}
            />
        </div>
    );
});
