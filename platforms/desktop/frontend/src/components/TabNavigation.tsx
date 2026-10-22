import { PlusCircle, Wrench } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { isAnyDialogOpen } from '../stores/dialogStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { AppMode } from '../types/transcript';
import { BookIcon } from './Icons';

/** Props for TabNavigation. */
interface TabNavigationProps {
  /** Optional CSS class name. */
  className?: string;
}
const TAB_MODES: AppMode[] = ['create', 'projects', 'tools'];

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
  const activeIndex = TAB_MODES.indexOf(mode);

  const handleTabChange = useCallback(
    (newMode: AppMode) => {
      if (mode === newMode) {
        if (newMode === 'create') {
          const ingestionFocus = useTranscriptStore.getState().ingestionFocus;
          if (ingestionFocus === 'batch') {
            document.querySelector('.queue-list')?.scrollTo({ top: 0, behavior: 'smooth' });
          }
        } else if (newMode === 'projects') {
          document.querySelector('.projects-main-scroll')?.scrollTo({ top: 0, behavior: 'smooth' });
          document.querySelector('.projects-rail-list')?.scrollTo({ top: 0, behavior: 'smooth' });
        } else if (newMode === 'tools') {
          document.querySelector('.tools-detail-pane')?.scrollTo({ top: 0, behavior: 'smooth' });
        }
        return;
      }
      setMode(newMode);

      if (newMode === 'create') {
        const ingestionFocus = useTranscriptStore.getState().ingestionFocus;
        if (ingestionFocus === 'live') {
          const recordingSessionId = useTranscriptStore.getState().recordingSessionId;
          useTranscriptStore.setState({ activeSessionId: recordingSessionId || 'default' });
        } else if (ingestionFocus === 'batch') {
          const activeItemId = useBatchQueueStore.getState().activeItemId;
          if (activeItemId) {
            const sessions = useTranscriptStore.getState().sessions;
            if (sessions[activeItemId]) {
              useTranscriptStore.setState({ activeSessionId: activeItemId });
            } else {
              useTranscriptStore.setState({ activeSessionId: 'default' });
            }
          } else {
            useTranscriptStore.setState({ activeSessionId: 'default' });
          }
        }
      }

      if (newMode === 'projects' || newMode === 'tools') {
        // Reset to a neutral session so the batch item's transcript is not
        // shown in the projects/tools view while a batch run is in progress.
        useTranscriptStore.setState({ activeSessionId: 'default' });
      }
    },
    [mode, setMode]
  );

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'Tab') {
        // Check if any modal/dialog is open
        const isSettingsOpen = !!document.querySelector('.settings-overlay');
        if (isSettingsOpen || isAnyDialogOpen()) return;

        e.preventDefault();
        const currentIndex = TAB_MODES.indexOf(mode);
        const nextIndex = e.shiftKey
          ? (currentIndex - 1 + TAB_MODES.length) % TAB_MODES.length
          : (currentIndex + 1) % TAB_MODES.length;

        handleTabChange(TAB_MODES[nextIndex]);
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
      <div
        className="tab-slider"
        style={{
          transform: `translateX(calc(${activeIndex} * (100% + 4px)))`,
        }}
        aria-hidden="true"
      />
      <button
        className={`tab-button ${mode === 'create' ? 'active' : ''}`}
        onClick={() => handleTabChange('create')}
        aria-selected={mode === 'create'}
        role="tab"
      >
        <PlusCircle size={16} aria-hidden="true" />
        <span>{t('panel.create', { defaultValue: '新建' })}</span>
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
      <button
        className={`tab-button ${mode === 'tools' ? 'active' : ''}`}
        onClick={() => handleTabChange('tools')}
        aria-selected={mode === 'tools'}
        role="tab"
      >
        <Wrench size={16} aria-hidden="true" />
        <span>{t('panel.tools', { defaultValue: 'Tools' })}</span>
      </button>
    </div>
  );
}
