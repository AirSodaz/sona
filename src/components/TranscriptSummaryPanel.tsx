import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../services/llmConfig';
import { isSummaryRecordStale, summaryService } from '../services/summaryService';
import { DEFAULT_SUMMARY_TEMPLATE, SummaryTemplate } from '../types/transcript';
import { ChevronDownIcon, ProcessingIcon, SparklesIcon } from './Icons';

const SUMMARY_TEMPLATES: SummaryTemplate[] = ['general', 'meeting', 'lecture'];

export function TranscriptSummaryPanel(): React.JSX.Element | null {
  const { t } = useTranslation();
  const segments = useTranscriptStore((state) => state.segments);
  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const config = useTranscriptStore((state) => state.config);
  const summaryState = useTranscriptStore((state) => state.summaryStates[state.sourceHistoryId || 'current']);
  const showError = useDialogStore((state) => state.showError);
  const [copied, setCopied] = useState(false);
  const [isCollapsed, setIsCollapsed] = useState(true);
  const copyResetTimerRef = useRef<number | null>(null);
  const summaryEnabled = config.summaryEnabled ?? true;
  const isSummaryVisible = summaryEnabled && segments.length > 0;

  const activeTemplate = summaryState?.activeTemplate || DEFAULT_SUMMARY_TEMPLATE;
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
  };

  const handleGenerate = async () => {
    const llm = getFeatureLlmConfig(config, 'summary');
    if (!isLlmConfigComplete(llm)) {
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
      >
        <div className="transcript-summary-panel-title-group">
          <span className="transcript-summary-panel-icon">
            <SparklesIcon />
          </span>
          <div className="transcript-summary-panel-heading">
            <div className="transcript-summary-panel-title">{t('summary.title')}</div>
            {!isCollapsed && (
              <div className="transcript-summary-panel-subtitle">{t('summary.subtitle')}</div>
            )}
          </div>
        </div>

        <div className="transcript-summary-panel-toggle-meta">
          {isGenerating && (
            <span className="transcript-summary-panel-badge is-generating">
              {generationProgress > 0
                ? t('summary.generating_short_progress', { progress: generationProgress })
                : t('summary.generating_short')}
            </span>
          )}
          {!isGenerating && record && isStale && (
            <span className="transcript-summary-panel-badge is-stale">
              {t('summary.stale_short')}
            </span>
          )}
          <span className={`transcript-summary-panel-chevron ${isCollapsed ? '' : 'open'}`} aria-hidden="true">
            <ChevronDownIcon />
          </span>
        </div>
      </button>

      {!isCollapsed && (
        <>
          <div className="transcript-summary-panel-actions">
            <button
              type="button"
              className="btn btn-secondary transcript-summary-action-button"
              onClick={handleCopy}
              disabled={!record?.content}
            >
              {copied ? t('summary.copied') : t('summary.copy')}
            </button>
            <button
              type="button"
              className="btn btn-primary transcript-summary-action-button"
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
          </div>

          <div className="transcript-summary-template-row" role="tablist" aria-label={t('summary.templates_label')}>
            {SUMMARY_TEMPLATES.map((template) => (
              <button
                key={template}
                type="button"
                role="tab"
                aria-selected={activeTemplate === template}
                className={`transcript-summary-template-tab ${activeTemplate === template ? 'active' : ''}`}
                onClick={() => void handleTemplateChange(template)}
                disabled={isGenerating}
              >
                {t(`summary.templates.${template}`)}
              </button>
            ))}
          </div>

          {record && isStale && (
            <div className="transcript-summary-stale-banner">
              {t('summary.stale')}
            </div>
          )}

          <div className="transcript-summary-content" data-summary-template={activeTemplate}>
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
