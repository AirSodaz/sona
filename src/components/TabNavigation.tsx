import React, { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { AppMode } from '../types/transcript';
import { useDialogStore } from '../stores/dialogStore';
import { useErrorDialogStore } from '../stores/errorDialogStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';

import { MicIcon, FolderIcon, BookIcon } from './Icons';


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
export function TabNavigation({ className = '' }: TabNavigationProps): React.JSX.Element {
    const { t } = useTranslation();
    const mode = useTranscriptRuntimeStore((state) => state.mode);
    const setMode = useTranscriptRuntimeStore((state) => state.setMode);

    const handleTabChange = useCallback((newMode: AppMode) => {
        setMode(newMode);
    }, [setMode]);

    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.key === 'Tab') {
                // Check if any modal/dialog is open
                const isSettingsOpen = !!document.querySelector('.settings-overlay');
                const isDialogOpen = useDialogStore.getState().isOpen;
                const isErrorDialogOpen = useErrorDialogStore.getState().isOpen;

                if (isSettingsOpen || isDialogOpen || isErrorDialogOpen) return;

                e.preventDefault();
                const modes: AppMode[] = ['live', 'batch', 'projects'];
                const currentIndex = modes.indexOf(mode);
                const nextIndex = e.shiftKey
                    ? (currentIndex - 1 + modes.length) % modes.length
                    : (currentIndex + 1) % modes.length;

                handleTabChange(modes[nextIndex]);
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [handleTabChange, mode]);

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
            <button
                className={`tab-button ${mode === 'projects' ? 'active' : ''}`}
                onClick={() => handleTabChange('projects')}
                aria-selected={mode === 'projects'}
                role="tab"
            >
                <BookIcon />
                <span>{t('panel.projects')}</span>
            </button>
        </div>
    );
}
