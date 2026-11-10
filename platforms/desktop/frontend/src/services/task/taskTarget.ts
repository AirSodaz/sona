import type { TranscriptTextReplacementRuleSet } from '../../bindings';
import { getActiveEditor } from '../../stores/transcriptRuntimeStore';
import { resolveSidecarKey } from '../../stores/transcriptSidecarState';
import { useTranscriptStore } from '../../stores/transcriptStore';
import type { TranscriptSegment } from '../../types/transcript';
import { extractReplacementsFromTerms } from '../../utils/textProcessing';
import { historyService } from '../historyService';
import type { EffectivePipelineSnapshot } from '../projectPipeline';
import { transcriptAutoSaveRuntime } from '../transcriptAutoSaveRuntime';
import {
  type DocumentMutationLockContext,
  documentMutationCoordinator,
} from './documentMutationCoordinator';

export interface TaskTarget {
  /** Immutable session identifier captured when the task was initiated, if bound to a session. */
  readonly sessionId: string | null;
  /** Initial persisted history id captured at launch (`null` if unpersisted draft). */
  readonly initialHistoryId: string | null;
}

export function normalizeHistoryId(historyId: string | null | undefined): string | null {
  return historyId && historyId !== 'current' ? historyId : null;
}

export function createTaskTarget(options?: {
  sessionId?: string | null;
  historyId?: string | null;
}): TaskTarget {
  return {
    sessionId: options?.sessionId ?? null,
    initialHistoryId: normalizeHistoryId(options?.historyId),
  };
}

export function captureTaskTarget(options?: {
  sessionId?: string | null;
  historyId?: string | null;
  fallbackToActiveSession?: boolean;
}): TaskTarget {
  const initialHistoryId = normalizeHistoryId(options?.historyId);
  let sessionId = options?.sessionId ?? null;
  if (!sessionId && !initialHistoryId && options?.fallbackToActiveSession !== false) {
    sessionId = useTranscriptStore.getState().activeSessionId || 'default';
  }
  return {
    sessionId,
    initialHistoryId,
  };
}

/**
 * Resolves the current durable history id for `target`, accounting for a session draft that
 * persisted and gained a `sourceHistoryId` after the task was initiated.
 */
export function resolveTargetHistoryId(target: TaskTarget): string | null {
  if (target.initialHistoryId) {
    return target.initialHistoryId;
  }
  if (target.sessionId) {
    const session = useTranscriptStore.getState().sessions[target.sessionId];
    return normalizeHistoryId(session?.sourceHistoryId);
  }
  return null;
}

export function resolveTargetSidecarKey(target: TaskTarget): string {
  return resolveSidecarKey(target.sessionId || 'default', resolveTargetHistoryId(target));
}

/**
 * Blurs any active Lexical segment editor so in-progress inline text edits commit before
 * acquiring a whole-document write lock or flushing auto-save.
 */
export function flushActiveLexicalEditor(): void {
  const editor = getActiveEditor();
  const root = editor?.getRootElement?.();
  if (root && typeof root.blur === 'function') {
    root.blur();
  }
}

export function runWithDocumentLock<T>(
  target: TaskTarget,
  options: {
    taskType: string;
    lockEditor?: boolean;
    flushAutoSave?: (historyIds: string[]) => Promise<void>;
  },
  fn: (lockCtx: DocumentMutationLockContext) => Promise<T>
): Promise<T> {
  return documentMutationCoordinator.runExclusive(
    {
      sessionId: target.sessionId,
      historyId: resolveTargetHistoryId(target) ?? target.initialHistoryId,
    },
    {
      taskType: options.taskType,
      lockEditor: options.lockEditor,
      flushActiveEditor: flushActiveLexicalEditor,
      flushAutoSave:
        options.flushAutoSave ?? ((ids) => transcriptAutoSaveRuntime.flushPendingHistoryIds(ids)),
      resolveLatestHistoryId: () => resolveTargetHistoryId(target),
    },
    fn
  );
}

export async function resolveLatestSegmentsForTarget(
  target: TaskTarget,
  fallbackSegments: TranscriptSegment[],
  options?: {
    loadTranscript?: (historyId: string) => Promise<TranscriptSegment[] | null>;
  }
): Promise<TranscriptSegment[]> {
  const latestHistoryId = resolveTargetHistoryId(target);
  const sessions = useTranscriptStore.getState().sessions;

  if (target.sessionId) {
    const boundSession = sessions[target.sessionId];
    if (boundSession && boundSession.segments.length > 0) {
      const boundHistoryId = normalizeHistoryId(boundSession.sourceHistoryId);
      if (boundHistoryId === latestHistoryId || (!latestHistoryId && !boundHistoryId)) {
        return boundSession.segments;
      }
    }
  }

  if (latestHistoryId) {
    const residentMatch = Object.values(sessions).find(
      (session) =>
        normalizeHistoryId(session.sourceHistoryId) === latestHistoryId &&
        session.segments.length > 0
    );
    if (residentMatch) {
      return residentMatch.segments;
    }

    const loader = options?.loadTranscript ?? ((id: string) => historyService.loadTranscript(id));
    try {
      const loaded = await loader(latestHistoryId);
      if (loaded && loaded.length > 0) {
        return loaded;
      }
    } catch {
      // Fall back to caller snapshot if disk load fails or is unmocked in isolated tests.
    }
  }

  return fallbackSegments;
}

export type SegmentMergeField = 'text' | 'translation';

/**
 * Merges rewritten `text` or `translation` fields from `updatedSegments` onto `baseSegments`
 * by segment id so concurrent edits on sibling fields or newly appended segments are preserved.
 */
export function mergeUpdatedSegmentsById(
  baseSegments: TranscriptSegment[],
  updatedSegments: TranscriptSegment[],
  field: SegmentMergeField
): TranscriptSegment[] {
  if (baseSegments.length === 0) {
    return updatedSegments;
  }
  const updatedMap = new Map<string, TranscriptSegment>();
  for (const segment of updatedSegments) {
    updatedMap.set(segment.id, segment);
  }
  return baseSegments.map((base) => {
    const updated = updatedMap.get(base.id);
    if (!updated) {
      return base;
    }
    if (field === 'text') {
      return { ...base, text: updated.text };
    }
    return { ...base, translation: updated.translation };
  });
}

export function buildLocalNmtPostProcessReplacements(
  pipeline: Pick<EffectivePipelineSnapshot, 'customTerms'>
): TranscriptTextReplacementRuleSet[] | undefined {
  if (!pipeline.customTerms || pipeline.customTerms.length === 0) {
    return undefined;
  }
  const replacements = extractReplacementsFromTerms(pipeline.customTerms);
  if (replacements.length === 0) {
    return undefined;
  }
  return [
    {
      enabled: true,
      ignoreCase: false,
      rules: replacements.map((rule) => ({
        from: rule.from,
        to: rule.to,
      })),
    },
  ];
}
