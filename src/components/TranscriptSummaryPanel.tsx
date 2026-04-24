import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useProjectStore } from '../stores/projectStore';
import { useDialogStore } from '../stores/dialogStore';
import { isSummaryLlmConfigComplete } from '../services/llmConfig';
import { isSummaryRecordStale, summaryService } from '../services/summaryService';
import { SummaryTemplate } from '../types/transcript';
import { ProcessingIcon, XIcon } from './Icons';

const SUMMARY_TEMPLATES: SummaryTemplate[] = ['general', 'meeting', 'lecture'];

interface TranscriptSummaryPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * Modal dialog for displaying and generating AI transcript summaries.
 */
export function TranscriptSummaryPanel({ isOpen, onClose }: TranscriptSummaryPanelProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const bodyId = useId();
  const modalRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  const segments = useTranscriptStore((state) => state.segments);
  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const config = useTranscriptStore((state) => state.config);
  const summaryState = useTranscriptStore((state) => state.summaryStates[state.sourceHistoryId || 'current']);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => state.projects.find((item) => item.id === state.activeProjectId) || null);
  const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);
  const showError = useDialogStore((state) => state.showError);
  
  const [copied, setCopied] = useState(false);
  const copyResetTimerRef = useRef<number | null>(null);

  const summaryConfigComplete = isSummaryLlmConfigComplete(config);
  const activeTemplate = summaryState?.activeTemplate || activeProject?.defaults.summaryTemplate || 'general';
  const record = summaryState?.records?.[activeTemplate];
  const isGenerating = summaryState?.isGenerating || false;
  const generationProgress = summaryState?.generationProgress || 0;
  const isStale = useMemo(() => isSummaryRecordStale(record, segments), [record, segments]);

  // Load summary on open
  useEffect(() => {
    if (isOpen && sourceHistoryId) {
      void summaryService.loadSummary(sourceHistoryId);
    }
  }, [isOpen, sourceHistoryId]);

  // Focus management
  useEffect(() => {
    if (isOpen) {
      requestAnimationFrame(() => {
        closeButtonRef.current?.focus();
      });
    }
  }, [isOpen]);

  // Keyboard support
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) return;
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  useEffect(() => {
    return () => {
      if (copyResetTimerRef.current !== null) {
        window.clearTimeout(copyResetTimerRef.current);
      }
    };
  }, []);

  if (!isOpen) {
    return null;
  }

  const handleTemplateChange = async (template: SummaryTemplate) => {
    await summaryService.setActiveTemplate(template);
    if (activeProjectId) {
      await updateProjectDefaults(activeProjectId, { summaryTemplate: template });
    }
  };

  const handleGenerate = async () => {
    if (!summaryConfigComplete) {
      await showError({
        code: 'config.summary_model_missing',
        messageKey: 'errors.config.summary_model_missing',
        showCause: false,
      });
      return;
    }

    try {
      await summaryService.generateSummary(activeTemplate);
    } catch (error) {
      await showError({
        code: 'summary.failed',
        messageKey: 'errors.summary.failed',
        cause: error,
      });
    }
  };

  const handleCopy = async () => {
    if (!record?.content || !navigator.clipboard?.writeText) {
      return;
    }

    await navigator.clipboard.writeText(record.content);
    setCopied(true);

    if (copyResetTimerRef.current !== null) {
      window.clearTimeout(copyResetTimerRef.current);
    }

    copyResetTimerRef.current = window.setTimeout(() => {
      setCopied(false);
      copyResetTimerRef.current = null;
    }, 1600);
  };

  const statusLabel = isGenerating
    ? generationProgress > 0
      ? t('summary.generating_progress', { progress: generationProgress })
      : t('summary.generating_short')
    : record && isStale
      ? t('summary.stale')
      : null;

  return (
    <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
      <div
        ref={modalRef}
        className="dialog-modal transcript-summary-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="summary-modal-title"
        style={{
          background: 'var(--color-bg-elevated)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: 'var(--shadow-xl)',
          width: '700px',
          maxWidth: '92vw',
          maxHeight: '85vh',
          display: 'flex',
          flexDirection: 'column',
          border: '1px solid var(--color-border)',
          overflow: 'hidden'
        }}
      >
        {/* Header */}
        <div style={{ 
          display: 'flex', 
          alignItems: 'center', 
          justifyContent: 'space-between',
          padding: 'var(--spacing-lg) var(--spacing-lg) var(--spacing-md)',
          borderBottom: '1px solid var(--color-border)'
        }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 'var(--spacing-md)' }}>
            <h3
              id="summary-modal-title"
              style={{
                fontSize: '1.125rem',
                fontWeight: 600,
                color: 'var(--color-text-primary)',
                margin: 0
              }}
            >
              {t('summary.title')}
            </h3>
            {statusLabel && (
              <span style={{ 
                fontSize: '0.75rem', 
                color: isGenerating ? 'var(--color-primary)' : 'var(--color-warning)',
                fontWeight: 500
              }}>
                {statusLabel}
              </span>
            )}
          </div>
          <button
            ref={closeButtonRef}
            className="btn btn-icon"
            onClick={onClose}
            aria-label={t('common.close')}
          >
            <XIcon />
          </button>
        </div>

        {/* Toolbar */}
        <div className="transcript-summary-panel-controls" style={{
          padding: 'var(--spacing-md) var(--spacing-lg)',
          background: 'var(--color-bg-secondary)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 'var(--spacing-md)',
          flexWrap: 'wrap'
        }}>
          <div className="transcript-summary-template-row" role="tablist" aria-label={t('summary.templates_label')} style={{ flex: 1 }}>
            {SUMMARY_TEMPLATES.map((template, index) => (
              <React.Fragment key={template}>
                {index > 0 && (
                  <span className="transcript-summary-template-divider" aria-hidden="true" style={{ margin: '0 4px' }}>
                    /
                  </span>
                )}
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeTemplate === template}
                  className={`transcript-summary-template-tab ${activeTemplate === template ? 'active' : ''}`}
                  onClick={() => void handleTemplateChange(template)}
                  disabled={isGenerating}
                >
                  {t(`summary.templates.${template}`)}
                </button>
              </React.Fragment>
            ))}
          </div>

          <div className="transcript-summary-panel-actions" style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
            <button
              type="button"
              className="btn transcript-summary-generate-button"
              onClick={handleGenerate}
              disabled={isGenerating}
            >
              {isGenerating ? (
                <>
                  <ProcessingIcon />
                  <span>{t('summary.generating_short')}</span>
                </>
              ) : (
                <span>{record ? t('summary.regenerate') : t('summary.generate')}</span>
              )}
            </button>

            <button
              type="button"
              className="btn btn-secondary transcript-summary-copy-button"
              onClick={handleCopy}
              disabled={!record?.content}
              style={{ padding: '5px 12px', fontSize: '0.78rem' }}
            >
              {copied ? t('summary.copied') : t('summary.copy')}
            </button>
          </div>
        </div>

        {/* Content */}
        <div
          id={bodyId}
          className="transcript-summary-panel-body"
          data-summary-template={activeTemplate}
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--spacing-lg)',
            border: 'none',
            background: 'var(--color-bg-primary)'
          }}
        >
          {record?.content ? (
            <pre className="transcript-summary-content-text" style={{ margin: 0 }}>{record.content}</pre>
          ) : (
            <div className="transcript-summary-empty-state">
              {t('summary.empty_state', { template: t(`summary.templates.${activeTemplate}`) })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default TranscriptSummaryPanel;
