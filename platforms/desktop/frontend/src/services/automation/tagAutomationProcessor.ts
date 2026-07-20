import type { AppConfig } from '../../types/config';
import type { AutomationActions } from '../../types/automation';
import type { TranscriptSegment } from '../../types/transcript';
import { historyService } from '../historyService';
import { polishService } from '../polishService';
import { summaryService } from '../summaryService';
import { translationService } from '../translationService';
import { beginTagAutomationRun, finishTagAutomationRun } from './tagAutomationRun';

export async function processTagAutomationForHistory(args: {
  actions: AutomationActions;
  config: AppConfig;
  historyId: string;
  segments: TranscriptSegment[];
  ruleId?: string;
  inputVersion?: string;
  force?: boolean;
}): Promise<TranscriptSegment[]> {
  const hasActions = args.actions.autoPolish || args.actions.autoTranslate || args.actions.autoSummary;
  const runKey = args.ruleId && args.inputVersion
    ? { ruleId: args.ruleId, historyId: args.historyId, inputVersion: args.inputVersion }
    : null;
  let runStarted = false;

  if (hasActions && runKey) {
    runStarted = await beginTagAutomationRun({
      ...runKey,
      actions: args.actions,
      force: args.force,
    });
    if (!runStarted) {
      return args.segments;
    }
  }

  let segments = args.segments;
  try {
    if (args.actions.autoPolish) {
      await polishService.polishSegmentsWithConfig(args.config, segments, async (chunk) => {
        segments = polishService.applyPolishedSegmentsInMemory(segments, chunk);
      });
      await historyService.updateTranscript(args.historyId, segments);
    }

    if (args.actions.autoTranslate) {
      await translationService.translateSegmentsWithConfig(args.config, segments, async (chunk) => {
        segments = translationService.applyTranslationsInMemory(segments, chunk);
      });
      await historyService.updateTranscript(args.historyId, segments);
    }

    if (args.actions.autoSummary) {
      await summaryService.retrySummaryTranscriptJob({
        segments,
        historyId: args.historyId,
        templateId: args.config.summaryTemplateId,
        config: args.config,
      });
      await summaryService.persistSummary(args.historyId);
    }

    if (runStarted && runKey) {
      await finishTagAutomationRun({ ...runKey, status: 'complete' });
    }
    return segments;
  } catch (error) {
    if (runStarted && runKey) {
      await finishTagAutomationRun({
        ...runKey,
        status: 'error',
        errorMessage: error instanceof Error ? error.message : String(error),
      });
    }
    throw error;
  }
}
