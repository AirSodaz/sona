import type { AppConfig } from '../types/config';
import type { TaskLedgerKind, TaskLedgerRecord } from '../types/taskLedger';
import type { TranscriptSegment } from '../types/transcript';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { logger } from '../utils/logger';
import { getFeatureLlmConfig, isLlmConfigComplete, isSummaryLlmConfigComplete } from './llm/configUtils';
import { historyService } from './historyService';
import { polishService } from './polishService';
import { summaryService } from './summaryService';
import { translationService } from './translationService';
import { handleTaskRetryPreflightFailure } from './taskRetryFailure';

interface RetryLlmTaskOptions {
  config?: AppConfig;
}

interface RetryTranscriptContext {
  segments: TranscriptSegment[];
  historyId: string | null;
}

type LlmLedgerKind = Extract<TaskLedgerKind, 'llmPolish' | 'llmTranslate' | 'llmSummary'>;

function isLlmLedgerKind(kind: TaskLedgerKind): kind is LlmLedgerKind {
  return kind === 'llmPolish' || kind === 'llmTranslate' || kind === 'llmSummary';
}

function hasSegments(segments: TranscriptSegment[] | null | undefined): segments is TranscriptSegment[] {
  return Array.isArray(segments) && segments.length > 0;
}

function getRetryConfig(options?: RetryLlmTaskOptions): AppConfig {
  return options?.config ?? getEffectiveConfigSnapshot();
}

function assertRetryConfig(task: TaskLedgerRecord, config: AppConfig): void {
  if (task.kind === 'llmSummary') {
    if (config.summaryEnabled === false) {
      throw new Error('Summary is disabled.');
    }

    if (!isSummaryLlmConfigComplete(config)) {
      throw new Error('LLM Service not fully configured.');
    }
    return;
  }

  const feature = task.kind === 'llmPolish' ? 'polish' : 'translation';
  if (!isLlmConfigComplete(getFeatureLlmConfig(config, feature))) {
    throw new Error('LLM Service not fully configured.');
  }
}

async function resolveRetryTranscript(task: TaskLedgerRecord): Promise<RetryTranscriptContext> {
  const sessionStore = useTranscriptSessionStore.getState();

  if (task.historyId) {
    if (sessionStore.sourceHistoryId === task.historyId) {
      if (!hasSegments(sessionStore.segments)) {
        throw new Error('Transcript is no longer available for retry.');
      }

      return {
        segments: sessionStore.segments,
        historyId: task.historyId,
      };
    }

    const segments = await historyService.loadTranscript(task.historyId);
    if (!hasSegments(segments)) {
      throw new Error('Transcript is no longer available for retry.');
    }

    return {
      segments,
      historyId: task.historyId,
    };
  }

  if (sessionStore.sourceHistoryId || !hasSegments(sessionStore.segments)) {
    throw new Error('Transcript is no longer available for retry.');
  }

  return {
    segments: sessionStore.segments,
    historyId: null,
  };
}

function runRetryTask(task: TaskLedgerRecord, context: RetryTranscriptContext): Promise<void> {
  if (task.kind === 'llmPolish') {
    return polishService.retryPolishTranscriptJob({
      segments: context.segments,
      historyId: context.historyId,
    });
  }

  if (task.kind === 'llmTranslate') {
    return translationService.retryTranslateTranscriptJob({
      segments: context.segments,
      historyId: context.historyId,
      targetLanguage: task.targetLanguage,
    });
  }

  if (task.kind === 'llmSummary') {
    return summaryService.retrySummaryTranscriptJob({
      segments: context.segments,
      historyId: context.historyId,
      templateId: task.templateId,
    });
  }

  throw new Error('Unsupported LLM task type.');
}

export async function retryLlmTaskFromLedger(
  task: TaskLedgerRecord,
  options?: RetryLlmTaskOptions,
): Promise<void> {
  try {
    if (!isLlmLedgerKind(task.kind)) {
      throw new Error('Unsupported LLM task type.');
    }

    const config = getRetryConfig(options);
    assertRetryConfig(task, config);
    const context = await resolveRetryTranscript(task);
    void runRetryTask(task, context).catch((error) => {
      logger.error('[TaskLedger] Retried LLM task failed:', error);
    });
  } catch (error) {
    handleTaskRetryPreflightFailure(task, error);
  }
}
