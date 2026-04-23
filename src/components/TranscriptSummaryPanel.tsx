import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useProjectStore } from '../stores/projectStore';
import { useDialogStore } from '../stores/dialogStore';
import { isSummaryLlmConfigComplete } from '../services/llmConfig';
import { isSummaryRecordStale, summaryService } from '../services/summaryService';
import { DEFAULT_SUMMARY_TEMPLATE, SummaryTemplate } from '../types/transcript';
import { ChevronDownIcon, ProcessingIcon } from './Icons';

const SUMMARY_TEMPLATES: SummaryTemplate[] = ['general', 'meeting', 'lecture'];

export function TranscriptSummaryPanel(): React.JSX.Element | null {
  const { t } = useTranslation();
  const bodyId = useId();
  const segments = useTranscriptStore((state) => state.segments);
  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const config = useTranscriptStore((state) => state.config);
  const summaryState = useTranscriptStore((state) => state.summaryStates[state.sourceHistoryId || 'current']);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => state.projects.find((item) => item.id === state.activeProjectId) || null);
  const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);
  const showError = useDialogStore((state) => state.showError);
  const [copied, setCopied] = useState(false);
  const [isCollapsed, setIsCollapsed] = useState(true);
  const copyResetTimerRef = useRef<number | null>(null);
  const summaryEnabled = config.summaryEnabled ?? true;
  const summaryConfigComplete = isSummaryLlmConfigComplete(config);
  const isSummaryVisible = summaryEnabled && summaryConfigComplete && segments.length > 0;

  const activeTemplate = summaryState?.activeTemplate || activeProject?.defaults.summaryTemplate || DEFAULT_SUMMARY_TEMPLATE;
  const record = summaryState?.records?.[activeTemplate];
  const isGenerating = summaryState?.isGenerating || false;
  const generationProgress = summaryState?.generationProgress || 0;
  const isStale = useMemo(() => isSummaryRecordStale(record, segments), [record, segments]);

  useEffect(() => {
    if (isSummaryVisible && sourceHistoryId) {
      void summaryService.loadSummary(sourceHistoryId);
    }
  }, [isSummaryVisible, sourceHistoryId]);

  useEffect(() => {
    setIsCollapsed(true);
  }, [sourceHistoryId]);

  useEffect(() => {
    return () => {
      if (copyResetTimerRef.current !== null) {
        window.clearTimeout(copyResetTimerRef.current);
      }
    };
  }, []);

  if (!isSummaryVisible) {
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

  const toggleLabel = t(isCollapsed ? 'summary.expand' : 'summary.collapse');
  const collapsedStatusLabel = isGenerating
    ? generationProgress > 0
      ? t('summary.generating_short_progress', { progress: generationProgress })
      : t('summary.generating_short')
    : record && isStale
      ? t('summary.stale_short')
      : null;
  const expandedStatusLabel = isGenerating
    ? generationProgress > 0
      ? t('summary.generating_progress', { progress: generationProgress })
      : t('summary.generating_short')
    : record && isStale
      ? t('summary.stale')
      : null;

  return (
    <section
      className={`transcript-summary-panel ${isCollapsed ? 'is-collapsed' : 'is-expanded'}`}
      aria-label={t('summary.title')}
    >
      <button
        type="button"
        className="transcript-summary-panel-toggle"
        onClick={() => setIsCollapsed((value) => !value)}
        aria-expanded={!isCollapsed}
        aria-label={toggleLabel}
        aria-controls={bodyId}
      >
        <span
          className={`transcript-summary-panel-chevron ${isCollapsed ? '' : 'open'}`}
          aria-hidden="true"
        >
          <ChevronDownIcon />
        </span>

        <div className="transcript-summary-panel-heading">
          <div className="transcript-summary-panel-title-line">
            <span className="transcript-summary-panel-title">{t('summary.title')}</span>
            {isCollapsed && collapsedStatusLabel && (
              <span className={`transcript-summary-panel-inline-status ${isGenerating ? 'is-generating' : 'is-stale'}`}>
                {collapsedStatusLabel}
              </span>
            )}
          </div>
          {!isCollapsed && expandedStatusLabel && (
            <div className={`transcript-summary-panel-meta ${isGenerating ? 'is-generating' : 'is-stale'}`}>
              {expandedStatusLabel}
            </div>
          )}
        </div>

      </button>

      {!isCollapsed && (
        <>
        <div className="transcript-summary-panel-controls">
          <div className="transcript-summary-template-row" role="tablist" aria-label={t('summary.templates_label')}>
            {SUMMARY_TEMPLATES.map((template, index) => (
              <React.Fragment key={template}>
                {index > 0 && (
                  <span className="transcript-summary-template-divider" aria-hidden="true">
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

          <div className="transcript-summary-panel-actions">
            <button
              type="button"
              className="btn transcript-summary-action-button transcript-summary-generate-button"
              onClick={handleGenerate}
              disabled={isGenerating}
            >
              {isGenerating ? (
                <>
                  <ProcessingIcon />
                  <span>{t('summary.generating_progress', { progress: generationProgress })}</span>
                </>
              ) : (
                <span>{record ? t('summary.regenerate') : t('summary.generate')}</span>
              )}
            </button>

            <button
              type="button"
              className="btn transcript-summary-copy-button"
              onClick={handleCopy}
              disabled={!record?.content}
            >
              {copied ? t('summary.copied') : t('summary.copy')}
            </button>
          </div>
        </div>

        <div
          id={bodyId}
          className="transcript-summary-panel-body"
          data-summary-template={activeTemplate}
        >
          {record?.content ? (
            <pre className="transcript-summary-content-text">{record.content}</pre>
          ) : (
            <div className="transcript-summary-empty-state">
              {t('summary.empty_state', { template: t(`summary.templates.${activeTemplate}`) })}
            </div>
          )}
        </div>
        </>
      )}
    </section>
  );
}

export default TranscriptSummaryPanel;
