import { SquarePen, Wrench } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
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
 * Tab navigation component for switching between application modes (Create, Projects, Tools).
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
            document
              .querySelector('.queue-grouped-list, .create-pane-batch, .queue-list')
              ?.scrollTo({ top: 0, behavior: 'smooth' });
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
    },
    [mode, setMode]
  );

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'Tab') {
        // Check if any modal/dialog is open
        const isModalOpen = !!document.querySelector(
          '.settings-overlay, .panel-modal-overlay, .shared-modal-overlay'
        );
        if (isModalOpen || isAnyDialogOpen()) return;

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

  const handleNavKeyDown = (e: React.KeyboardEvent) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();
    const currentIndex = TAB_MODES.indexOf(mode);
    const effectiveIndex = Math.max(currentIndex, 0);
    let nextIndex = effectiveIndex;
    if (e.key === 'ArrowRight') {
      nextIndex = (effectiveIndex + 1) % TAB_MODES.length;
    } else if (e.key === 'ArrowLeft') {
      nextIndex = (effectiveIndex - 1 + TAB_MODES.length) % TAB_MODES.length;
    } else if (e.key === 'Home') {
      nextIndex = 0;
    } else if (e.key === 'End') {
      nextIndex = TAB_MODES.length - 1;
    }
    const nextMode = TAB_MODES[nextIndex];
    if (nextMode) {
      handleTabChange(nextMode);
      document.getElementById(`tab-button-${nextMode}`)?.focus();
    }
  };
  return (
    <div
      className={`tab-navigation ${className}`}
      role="tablist"
      aria-label={t('panel.mode_selection', { defaultValue: 'Mode Selection' })}
      onKeyDown={handleNavKeyDown}
    >
      <div
        className="tab-slider"
        style={{
          transform: `translateX(calc(${activeIndex} * (100% + 4px)))`,
        }}
        aria-hidden="true"
      />
      <button
        id="tab-button-create"
        type="button"
        className={`tab-button ${mode === 'create' ? 'active' : ''}`}
        onClick={() => handleTabChange('create')}
        aria-selected={mode === 'create'}
        tabIndex={mode === 'create' ? 0 : -1}
        role="tab"
      >
        <SquarePen size={16} aria-hidden="true" />
        <span>{t('panel.create', { defaultValue: 'Create' })}</span>
      </button>
      <button
        id="tab-button-projects"
        type="button"
        className={`tab-button ${mode === 'projects' ? 'active' : ''}`}
        onClick={() => handleTabChange('projects')}
        aria-selected={mode === 'projects'}
        tabIndex={mode === 'projects' ? 0 : -1}
        role="tab"
      >
        <BookIcon />
        <span>{t('panel.projects', { defaultValue: 'Projects' })}</span>
      </button>
      <button
        id="tab-button-tools"
        type="button"
        className={`tab-button ${mode === 'tools' ? 'active' : ''}`}
        onClick={() => handleTabChange('tools')}
        aria-selected={mode === 'tools'}
        tabIndex={mode === 'tools' ? 0 : -1}
        role="tab"
      >
        <Wrench size={16} aria-hidden="true" />
        <span>{t('panel.tools', { defaultValue: 'Tools' })}</span>
      </button>
    </div>
  );
}
