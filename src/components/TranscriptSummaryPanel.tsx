import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useDialogStore } from '../stores/dialogStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { isSummaryLlmConfigComplete } from '../services/llmConfig';
import { isSummaryRecordStale, summaryService } from '../services/summaryService';
import {
  getSummaryTemplateOptions,
  resolveSummaryTemplate,
} from '../utils/summaryTemplates';
import { Dropdown } from './Dropdown';
import { ProcessingIcon, SummaryIcon, XIcon } from './Icons';

interface TranscriptSummaryPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * Modal dialog for displaying and generating transcript summaries.
 */
export function TranscriptSummaryPanel({ isOpen, onClose }: TranscriptSummaryPanelProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const bodyId = useId();
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const segments = useTranscriptStore((state) => state.segments);
  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const config = useTranscriptStore((state) => state.config);
  const summaryState = useTranscriptStore((state) => state.summaryStates[state.sourceHistoryId || 'current']);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);
  const showError = useDialogStore((state) => state.showError);

  const [copied, setCopied] = useState(false);
  const [editContent, setEditContent] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const copyResetTimerRef = useRef<number | null>(null);
  const editContentRef = useRef('');
  const lastSavedContentRef = useRef('');
  const saveInFlightRef = useRef<Promise<void> | null>(null);

  const summaryConfigComplete = isSummaryLlmConfigComplete(config);
  const activeTemplate = useMemo(
    () => resolveSummaryTemplate(
      summaryState?.activeTemplateId || config.summaryTemplateId,
      config.summaryCustomTemplates,
      t,
    ),
    [config.summaryCustomTemplates, config.summaryTemplateId, summaryState?.activeTemplateId, t],
  );
  const templateOptions = useMemo(
    () => getSummaryTemplateOptions(config.summaryCustomTemplates, t),
    [config.summaryCustomTemplates, t],
  );
  const record = summaryState?.record;
  const streamingContent = summaryState?.streamingContent || '';
  const isGenerating = summaryState?.isGenerating || false;
  const generationProgress = summaryState?.generationProgress || 0;
  const isStale = useMemo(() => isSummaryRecordStale(record, segments), [record, segments]);
  const displayContent = isGenerating
    ? streamingContent || record?.content || ''
    : record?.content || streamingContent;

  const persistDraftIfNeeded = useCallback(async () => {
    if (saveInFlightRef.current) {
      return saveInFlightRef.current;
    }

    const nextContent = editContentRef.current;
    const hasStoredRecord = !!useTranscriptStore.getState().getSummaryState(sourceHistoryId || 'current').record;
    if (nextContent === lastSavedContentRef.current || (!hasStoredRecord && !nextContent.trim())) {
      return;
    }

    setIsSaving(true);
    const savePromise = summaryService.updateSummaryRecord(nextContent)
      .then(() => {
        lastSavedContentRef.current = nextContent;
      })
      .finally(() => {
        setIsSaving(false);
        saveInFlightRef.current = null;
      });

    saveInFlightRef.current = savePromise;
    return savePromise;
  }, [sourceHistoryId]);

  const handleCloseRequest = useCallback(async () => {
    await persistDraftIfNeeded();
    onClose();
  }, [onClose, persistDraftIfNeeded]);

  useEffect(() => {
    setEditContent(displayContent);
    editContentRef.current = displayContent;
  }, [displayContent]);

  useEffect(() => {
    lastSavedContentRef.current = record?.content || '';
  }, [record?.content]);

  useEffect(() => {
    editContentRef.current = editContent;
  }, [editContent]);

  useEffect(() => {
    if (isOpen && sourceHistoryId) {
      void summaryService.loadSummary(sourceHistoryId);
    }
  }, [isOpen, sourceHistoryId]);

  useEffect(() => {
    if (isOpen) {
      requestAnimationFrame(() => {
        textareaRef.current?.focus();
      });
    }
  }, [isOpen]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) {
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        void handleCloseRequest();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleCloseRequest, isOpen]);

  useEffect(() => (
    () => {
      if (copyResetTimerRef.current !== null) {
        window.clearTimeout(copyResetTimerRef.current);
      }
    }
  ), []);

  if (!isOpen) {
    return null;
  }

  const handleTemplateChange = async (templateId: string) => {
    await persistDraftIfNeeded();
    await summaryService.setActiveTemplate(templateId);
    if (activeProjectId) {
      await updateProjectDefaults(activeProjectId, { summaryTemplateId: templateId });
    }
  };

  const handleGenerate = async () => {
    await persistDraftIfNeeded();
    if (!summaryConfigComplete) {
      await showError({
        code: 'config.summary_model_missing',
        messageKey: 'errors.config.summary_model_missing',
        showCause: false,
      });
      return;
    }

    try {
      await summaryService.generateSummary(activeTemplate.id);
    } catch (error) {
      await showError({
        code: 'summary.failed',
        messageKey: 'errors.summary.failed',
        cause: error,
      });
    }
  };

  const handleCopy = async () => {
    if (!editContent.trim() || !navigator.clipboard?.writeText) {
      return;
    }

    await navigator.clipboard.writeText(editContent);
    setCopied(true);

    if (copyResetTimerRef.current !== null) {
      window.clearTimeout(copyResetTimerRef.current);
    }

    copyResetTimerRef.current = window.setTimeout(() => {
      setCopied(false);
      copyResetTimerRef.current = null;
    }, 1600);
  };

  const handleContentChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setEditContent(e.target.value);
  };

  const handleBlur = async () => {
    await persistDraftIfNeeded();
  };

  const statusLabel = isGenerating
    ? generationProgress > 0
      ? t('summary.generating_progress', { progress: generationProgress })
      : t('summary.generating_short')
    : isSaving
      ? t('summary.saving')
      : record && isStale
        ? t('summary.stale')
        : null;

  return (
    <div className="settings-overlay" onClick={() => { void handleCloseRequest(); }} style={{ zIndex: 2000 }}>
      <div
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
          overflow: 'hidden',
        }}
      >
        <div style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: 'var(--spacing-lg) var(--spacing-lg) var(--spacing-md)',
          borderBottom: '1px solid var(--color-border)',
        }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 'var(--spacing-md)' }}>
            <h3
              id="summary-modal-title"
              style={{
                fontSize: '1.125rem',
                fontWeight: 600,
                color: 'var(--color-text-primary)',
                margin: 0,
              }}
            >
              {t('summary.title')}
            </h3>
            {statusLabel && (
              <span
                style={{
                  fontSize: '0.75rem',
                  color: isGenerating ? 'var(--color-primary)' : 'var(--color-warning)',
                  fontWeight: 500,
                }}
              >
                {statusLabel}
              </span>
            )}
          </div>
          <button
            ref={closeButtonRef}
            className="btn btn-icon"
            onClick={() => { void handleCloseRequest(); }}
            aria-label={t('common.close')}
          >
            <XIcon />
          </button>
        </div>

        <div className="transcript-summary-panel-controls" style={{
          padding: 'var(--spacing-md) var(--spacing-lg)',
          background: 'var(--color-bg-secondary)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 'var(--spacing-md)',
          flexWrap: 'wrap',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flex: 1, minWidth: '260px' }}>
            <span
              style={{
                fontSize: '0.8125rem',
                fontWeight: 600,
                color: 'var(--color-text-secondary)',
                whiteSpace: 'nowrap',
              }}
            >
              {t('summary.templates_label')}
            </span>
            <div style={{ flex: 1, minWidth: '220px' }}>
              <Dropdown
                value={activeTemplate.id}
                onChange={(value: string) => {
                  void handleTemplateChange(value);
                }}
                options={templateOptions}
                style={{ width: '100%' }}
              />
            </div>
          </div>

          <div className="transcript-summary-panel-actions" style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
            <button
              type="button"
              className="btn transcript-summary-generate-button"
              onClick={handleGenerate}
              disabled={isGenerating || !summaryConfigComplete}
            >
              {isGenerating ? (
                <>
                  <ProcessingIcon />
                  <span>{t('summary.generating_short')}</span>
                </>
              ) : (
                <>
                  <SummaryIcon />
                  <span>{record ? t('summary.regenerate') : t('summary.generate')}</span>
                </>
              )}
            </button>

            <button
              type="button"
              className="btn btn-secondary transcript-summary-copy-button"
              onClick={handleCopy}
              disabled={!editContent.trim()}
              style={{ padding: '5px 12px', fontSize: '0.78rem' }}
            >
              {copied ? t('summary.copied') : t('summary.copy')}
            </button>
          </div>
        </div>

        {!summaryConfigComplete && (
          <div
            style={{
              margin: '0 var(--spacing-lg)',
              marginTop: 'var(--spacing-md)',
              padding: '10px 12px',
              borderRadius: 'var(--radius-sm)',
              border: '1px dashed var(--color-border)',
              background: 'var(--color-bg-secondary)',
              color: 'var(--color-text-secondary)',
              fontSize: '0.8125rem',
              lineHeight: 1.5,
            }}
          >
            {t('summary.manual_only_hint', {
              defaultValue: 'Configure an LLM service to generate summaries. You can still write and edit this summary manually.',
            })}
          </div>
        )}

        <div
          id={bodyId}
          className="transcript-summary-panel-body"
          data-summary-template={activeTemplate.id}
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--spacing-lg)',
            border: 'none',
            background: 'var(--color-bg-primary)',
            display: 'flex',
            flexDirection: 'column',
          }}
        >
          <textarea
            ref={textareaRef}
            className="transcript-summary-content-text"
            value={editContent}
            onChange={handleContentChange}
            onBlur={() => { void handleBlur(); }}
            placeholder={t('summary.placeholder')}
            style={{
              flex: 1,
              width: '100%',
              border: 'none',
              outline: 'none',
              resize: 'none',
              background: 'transparent',
              padding: 0,
              margin: 0,
              display: 'block',
              minHeight: '400px',
            }}
          />
        </div>
      </div>
    </div>
  );
}

export default TranscriptSummaryPanel;
