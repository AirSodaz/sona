import type { TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot } from '../../types/taskLedger';
import type {
  TaskLedgerPatch_Deserialize,
  TaskLedgerRecord_Deserialize,
  TaskLedgerRecord_Serialize,
  TaskLedgerSnapshot_Serialize,
} from '../../bindings';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

function toTaskLedgerRecordTransport(
  record: TaskLedgerRecord,
): TaskLedgerRecord_Deserialize {
  const { projectId, ...current } = record;
  return {
    ...current,
    stage: current.stage ?? null,
    historyId: current.historyId ?? null,
    tagIds: current.tagIds ?? (projectId ? [projectId] : undefined),
    filePath: current.filePath ?? null,
    automationRuleId: current.automationRuleId ?? null,
    sourceFingerprint: current.sourceFingerprint ?? null,
    errorMessage: current.errorMessage ?? null,
    templateId: current.templateId ?? null,
    targetLanguage: current.targetLanguage ?? null,
  };
}

function toTaskLedgerPatchTransport(
  patch: TaskLedgerPatch,
): TaskLedgerPatch_Deserialize {
  const { projectId, ...current } = patch;
  const compatibilityTagIds = projectId === null
    ? []
    : projectId
      ? [projectId]
      : undefined;
  return {
    ...current,
    ...(current.tagIds !== undefined
      ? { tagIds: current.tagIds }
      : compatibilityTagIds !== undefined
        ? { tagIds: compatibilityTagIds }
        : {}),
  };
}

function normalizeTaskLedgerRecord(
  record: TaskLedgerRecord_Serialize,
): TaskLedgerRecord {
  return {
    id: record.id,
    kind: record.kind,
    status: record.status,
    title: record.title,
    progress: record.progress,
    createdAt: record.createdAt,
    updatedAt: record.updatedAt,
    retryable: record.retryable,
    cancelable: record.cancelable,
    recoverable: record.recoverable,
    ...(record.stage != null ? { stage: record.stage } : {}),
    ...(record.historyId != null ? { historyId: record.historyId } : {}),
    ...(record.tagIds !== undefined ? { tagIds: record.tagIds } : {}),
    ...(record.filePath != null ? { filePath: record.filePath } : {}),
    ...(record.automationRuleId != null
      ? { automationRuleId: record.automationRuleId }
      : {}),
    ...(record.sourceFingerprint != null
      ? { sourceFingerprint: record.sourceFingerprint }
      : {}),
    ...(record.errorMessage != null ? { errorMessage: record.errorMessage } : {}),
    ...(record.templateId != null ? { templateId: record.templateId } : {}),
    ...(record.targetLanguage != null
      ? { targetLanguage: record.targetLanguage }
      : {}),
  };
}

function normalizeTaskLedgerSnapshot(
  snapshot: TaskLedgerSnapshot_Serialize,
): TaskLedgerSnapshot {
  return {
    ...snapshot,
    tasks: snapshot.tasks.map(normalizeTaskLedgerRecord),
  };
}

export async function taskLedgerLoadSnapshot(): Promise<TaskLedgerSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.taskLedger.loadSnapshot);
  return normalizeTaskLedgerSnapshot(snapshot);
}

export async function taskLedgerUpsertTask(record: TaskLedgerRecord): Promise<TaskLedgerSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.taskLedger.upsertTask, {
    record: toTaskLedgerRecordTransport(record),
  });
  return normalizeTaskLedgerSnapshot(snapshot);
}

export async function taskLedgerPatchTask(
  id: string,
  patch: TaskLedgerPatch,
): Promise<TaskLedgerSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.taskLedger.patchTask, {
    id,
    patch: toTaskLedgerPatchTransport(patch),
  });
  return normalizeTaskLedgerSnapshot(snapshot);
}

export async function taskLedgerRemoveTask(id: string): Promise<TaskLedgerSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.taskLedger.removeTask, { id });
  return normalizeTaskLedgerSnapshot(snapshot);
}

export async function taskLedgerClearResolved(): Promise<TaskLedgerSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.taskLedger.clearResolved);
  return normalizeTaskLedgerSnapshot(snapshot);
}
