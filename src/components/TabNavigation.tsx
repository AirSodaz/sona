import { useTranscriptStore } from '../stores/transcriptStore';
import { useTranslation } from 'react-i18next';
import { AppMode } from '../types/transcript';

// Icons as inline SVG components for simplicity
const MicIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
        <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
        <line x1="12" x2="12" y1="19" y2="22" />
    </svg>
);

const FolderIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
    </svg>
);

interface TabNavigationProps {
    className?: string;
}

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
