import React from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useTranslation } from 'react-i18next';
import { AppMode } from '../types/transcript';

import { MicIcon, FolderIcon } from './Icons';


/** Props for TabNavigation. */
interface TabNavigationProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Tab navigation component for switching between application modes (Live Record, Batch Import).
 *
 * @param props - Component props.
 * @return The navigation tabs.
 */
export const TabNavigation: React.FC<TabNavigationProps> = ({ className = '' }) => {
    const { t } = useTranslation();
    const mode = useTranscriptStore((state) => state.mode);
    const setMode = useTranscriptStore((state) => state.setMode);

    const handleTabChange = (newMode: AppMode) => {
        setMode(newMode);
    };

    return (
        <div
            className={`tab-navigation ${className}`}
            role="tablist"
            aria-label={t('panel.mode_selection')}
        >
            <button
                className={`tab-button ${mode === 'live' ? 'active' : ''}`}
                onClick={() => handleTabChange('live')}
                aria-selected={mode === 'live'}
                role="tab"
            >
                <MicIcon />
                <span>{t('panel.live_record')}</span>
            </button>
            <button
                className={`tab-button ${mode === 'batch' ? 'active' : ''}`}
                onClick={() => handleTabChange('batch')}
                aria-selected={mode === 'batch'}
                role="tab"
            >
                <FolderIcon />
                <span>{t('panel.batch_import')}</span>
            </button>
        </div>
    );
};

export default TabNavigation;
