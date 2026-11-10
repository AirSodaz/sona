import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { AppConfig } from '../types/config';
import type { TaskLedgerKind, TaskLedgerRecord } from '../types/taskLedger';
import type { TranscriptSegment } from '../types/transcript';
import { logger } from '../utils/logger';
import { historyService } from './historyService';
import {
  getFeatureLlmConfig,
  isLlmConfigComplete,
  isSummaryLlmConfigComplete,
} from './llm/configUtils';
import { polishService } from './polishService';
import { summaryService } from './summaryService';
import { handleTaskRetryPreflightFailure } from './taskRetryFailure';
import { translationService } from './translationService';

interface RetryLlmTaskOptions {
  config?: AppConfig;
}

interface RetryTranscriptContext {
  segments: TranscriptSegment[];
  historyId: string | null;
  sessionId: string | null;
}

type LlmLedgerKind = Extract<TaskLedgerKind, 'llmPolish' | 'llmTranslate' | 'llmSummary'>;

function isLlmLedgerKind(kind: TaskLedgerKind): kind is LlmLedgerKind {
  return kind === 'llmPolish' || kind === 'llmTranslate' || kind === 'llmSummary';
}

function hasSegments(
  segments: TranscriptSegment[] | null | undefined
): segments is TranscriptSegment[] {
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
  const transcriptState = useTranscriptStore.getState();
  const sessionStore = useTranscriptSessionStore.getState();

  if (task.historyId) {
    const matchingEntry = Object.entries(transcriptState.sessions).find(
      ([, session]) => session.sourceHistoryId === task.historyId && hasSegments(session.segments)
    );
    if (matchingEntry) {
      return {
        segments: matchingEntry[1].segments,
        historyId: task.historyId,
        sessionId: matchingEntry[0],
      };
    }

    if (sessionStore.sourceHistoryId === task.historyId && hasSegments(sessionStore.segments)) {
      return {
        segments: sessionStore.segments,
        historyId: task.historyId,
        sessionId: transcriptState.activeSessionId,
      };
    }

    const segments = await historyService.loadTranscript(task.historyId);
    if (!hasSegments(segments)) {
      throw new Error('Transcript is no longer available for retry.');
    }

    return {
      segments,
      historyId: task.historyId,
      sessionId: null,
    };
  }

  const activeSession = transcriptState.sessions[transcriptState.activeSessionId];
  if (activeSession && !activeSession.sourceHistoryId && hasSegments(activeSession.segments)) {
    return {
      segments: activeSession.segments,
      historyId: null,
      sessionId: transcriptState.activeSessionId,
    };
  }

  const draftEntry = Object.entries(transcriptState.sessions).find(
    ([, session]) => !session.sourceHistoryId && hasSegments(session.segments)
  );
  if (draftEntry) {
    return {
      segments: draftEntry[1].segments,
      historyId: null,
      sessionId: draftEntry[0],
    };
  }

  if (!sessionStore.sourceHistoryId && hasSegments(sessionStore.segments)) {
    return {
      segments: sessionStore.segments,
      historyId: null,
      sessionId: transcriptState.activeSessionId,
    };
  }

  throw new Error('Transcript is no longer available for retry.');
}

function runRetryTask(task: TaskLedgerRecord, context: RetryTranscriptContext): Promise<void> {
  if (task.kind === 'llmPolish') {
    return polishService.retryPolishTranscriptJob({
      segments: context.segments,
      historyId: context.historyId,
      sessionId: context.sessionId,
      priority: 'normal',
    });
  }

  if (task.kind === 'llmTranslate') {
    return translationService.retryTranslateTranscriptJob({
      segments: context.segments,
      historyId: context.historyId,
      sessionId: context.sessionId,
      targetLanguage: task.targetLanguage,
      priority: 'normal',
    });
  }

  if (task.kind === 'llmSummary') {
    return summaryService.retrySummaryTranscriptJob({
      segments: context.segments,
      historyId: context.historyId,
      sessionId: context.sessionId,
      templateId: task.templateId,
      priority: 'normal',
    });
  }

  throw new Error('Unsupported LLM task type.');
}

export async function retryLlmTaskFromLedger(
  task: TaskLedgerRecord,
  options?: RetryLlmTaskOptions
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
