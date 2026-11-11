import { useMemo, useSyncExternalStore } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useHistoryStore } from '../../stores/historyStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTaskLedgerStore } from '../../stores/taskLedgerStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { type SessionData, useTranscriptStore } from '../../stores/transcriptStore';
import type { BatchQueueItem } from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';
import type { EffectivePipelineSnapshot } from '../../types/project';
import { isTaskLedgerActiveStatus, type TaskLedgerRecord } from '../../types/taskLedger';
import { useDocumentMutationLockStore } from './documentMutationCoordinator';
import { normalizeHistoryId } from './taskTarget';

interface BatchQueueReader {
  getItems: () => BatchQueueItem[];
  subscribe: (listener: () => void) => () => void;
}

const EMPTY_BATCH_QUEUE_ITEMS: BatchQueueItem[] = [];
const batchQueueListeners = new Set<() => void>();
let batchQueueReader: BatchQueueReader | null = null;
let batchQueueUnsubscribe: (() => void) | null = null;

export function registerBatchQueueReader(reader: BatchQueueReader): void {
  batchQueueUnsubscribe?.();
  batchQueueReader = reader;
  batchQueueUnsubscribe = reader.subscribe(() => {
    for (const listener of batchQueueListeners) {
      listener();
    }
  });
  for (const listener of batchQueueListeners) {
    listener();
  }
}

function getBatchQueueItems(): BatchQueueItem[] {
  return batchQueueReader?.getItems() ?? EMPTY_BATCH_QUEUE_ITEMS;
}

function subscribeBatchQueueItems(listener: () => void): () => void {
  batchQueueListeners.add(listener);
  return () => {
    batchQueueListeners.delete(listener);
  };
}

export interface TaskProjectContextOptions {
  sessionId?: string | null;
  historyId?: string | null;
  fallbackToActive?: boolean;
}

export interface TaskProjectContext {
  projectId: string | null;
  pipelineSnapshot?: EffectivePipelineSnapshot;
  source: 'batchQueue' | 'history' | 'session' | 'activeProject' | 'none';
}

interface ComputeTaskProjectContextParams {
  sessionId?: string | null;
  historyId?: string | null;
  fallbackToActive?: boolean;
  queueItems: BatchQueueItem[];
  historyItems: HistoryItem[];
  session: SessionData | undefined;
  activeProjectId: string | null;
}

function computeTaskProjectContext({
  sessionId,
  historyId,
  fallbackToActive = true,
  queueItems,
  historyItems,
  session,
  activeProjectId,
}: ComputeTaskProjectContextParams): TaskProjectContext {
  if (sessionId) {
    const queueItem = queueItems.find((item) => item.id === sessionId);
    if (queueItem) {
      return {
        projectId: queueItem.projectId ?? null,
        pipelineSnapshot: queueItem.pipelineSnapshot,
        source: 'batchQueue',
      };
    }
  }

  const resolvedHistoryId =
    normalizeHistoryId(historyId) ?? normalizeHistoryId(session?.sourceHistoryId);
  if (resolvedHistoryId) {
    const historyItem = historyItems.find((item) => item.id === resolvedHistoryId);
    if (historyItem) {
      return {
        projectId: historyItem.projectId ?? null,
        pipelineSnapshot: session?.pipelineSnapshot ?? undefined,
        source: 'history',
      };
    }
  }

  if (session && session.projectId !== undefined) {
    return {
      projectId: session.projectId,
      pipelineSnapshot: session.pipelineSnapshot ?? undefined,
      source: 'session',
    };
  }

  if (fallbackToActive) {
    return {
      projectId: activeProjectId ?? null,
      source: 'activeProject',
    };
  }

  return {
    projectId: null,
    source: 'none',
  };
}

export function resolveTaskProjectContext(options?: TaskProjectContextOptions): TaskProjectContext {
  const sessionId = options?.sessionId;
  const session = sessionId ? useTranscriptStore.getState().sessions[sessionId] : undefined;
  return computeTaskProjectContext({
    sessionId,
    historyId: options?.historyId,
    fallbackToActive: options?.fallbackToActive,
    queueItems: getBatchQueueItems(),
    historyItems: useHistoryStore.getState().items,
    session,
    activeProjectId: useProjectStore.getState().activeProjectId,
  });
}

export function useTaskProjectContext(
  sessionId?: string | null,
  historyId?: string | null
): TaskProjectContext {
  const queueItems = useSyncExternalStore(
    subscribeBatchQueueItems,
    getBatchQueueItems,
    getBatchQueueItems
  );
  const historyItems = useHistoryStore((s) => s.items);
  const session = useTranscriptStore((s) => (sessionId ? s.sessions[sessionId] : undefined));
  const activeProjectId = useProjectStore((s) => s.activeProjectId);

  return useMemo(
    () =>
      computeTaskProjectContext({
        sessionId,
        historyId,
        fallbackToActive: true,
        queueItems,
        historyItems,
        session,
        activeProjectId,
      }),
    [sessionId, historyId, queueItems, historyItems, session, activeProjectId]
  );
}

export function useTaskProjectId(
  sessionId?: string | null,
  historyId?: string | null
): string | null {
  return useTaskProjectContext(sessionId, historyId).projectId;
}

interface ComputeProjectsWithActiveTasksParams {
  isRecording: boolean;
  recordingSessionId: string | null;
  isAgentRecording: boolean;
  agentRecordingHistoryId: string | null;
  sessions: Record<string, SessionData>;
  queueItems: BatchQueueItem[];
  historyItems: HistoryItem[];
  locks: Record<string, { depth: number }>;
  tasks: TaskLedgerRecord[];
}

function computeProjectsWithActiveTasks({
  isRecording,
  recordingSessionId,
  isAgentRecording,
  agentRecordingHistoryId,
  sessions,
  queueItems,
  historyItems,
  locks,
  tasks,
}: ComputeProjectsWithActiveTasksParams): Set<string> {
  const result = new Set<string>();

  if (isRecording) {
    const recProjectId = computeTaskProjectContext({
      sessionId: recordingSessionId,
      historyId: recordingSessionId,
      fallbackToActive: false,
      queueItems,
      historyItems,
      session: recordingSessionId ? sessions[recordingSessionId] : undefined,
      activeProjectId: null,
    }).projectId;
    if (recProjectId) {
      result.add(recProjectId);
    }
  }

  if (isAgentRecording && agentRecordingHistoryId) {
    const agentProjectId = computeTaskProjectContext({
      historyId: agentRecordingHistoryId,
      fallbackToActive: false,
      queueItems,
      historyItems,
      session: undefined,
      activeProjectId: null,
    }).projectId;
    if (agentProjectId) {
      result.add(agentProjectId);
    }
  }

  for (const item of queueItems) {
    if ((item.status === 'pending' || item.status === 'processing') && item.projectId) {
      result.add(item.projectId);
    }
  }

  for (const [lockKey, entry] of Object.entries(locks)) {
    if (entry.depth <= 0) {
      continue;
    }
    if (lockKey.startsWith('session:')) {
      const lockSessionId = lockKey.slice('session:'.length);
      const lockProjectId = computeTaskProjectContext({
        sessionId: lockSessionId,
        fallbackToActive: false,
        queueItems,
        historyItems,
        session: sessions[lockSessionId],
        activeProjectId: null,
      }).projectId;
      if (lockProjectId) {
        result.add(lockProjectId);
      }
    } else {
      const lockProjectId = computeTaskProjectContext({
        historyId: lockKey,
        fallbackToActive: false,
        queueItems,
        historyItems,
        session: undefined,
        activeProjectId: null,
      }).projectId;
      if (lockProjectId) {
        result.add(lockProjectId);
      }
    }
  }

  for (const task of tasks) {
    if (!isTaskLedgerActiveStatus(task.status)) {
      continue;
    }
    const explicitProjectId = task.projectId ?? task.tagIds?.[0];
    if (explicitProjectId) {
      result.add(explicitProjectId);
      continue;
    }
    if (task.historyId) {
      const taskProjectId = computeTaskProjectContext({
        historyId: task.historyId,
        fallbackToActive: false,
        queueItems,
        historyItems,
        session: undefined,
        activeProjectId: null,
      }).projectId;
      if (taskProjectId) {
        result.add(taskProjectId);
      }
      continue;
    }
    if (task.id.startsWith('batch-')) {
      const queueId = task.id.slice('batch-'.length);
      const matched = queueItems.find((item) => item.id === queueId);
      if (matched?.projectId) {
        result.add(matched.projectId);
      }
    }
  }

  return result;
}

export function getProjectsWithActiveTasks(): Set<string> {
  const runtimeState = useTranscriptRuntimeStore.getState();
  const transcriptState = useTranscriptStore.getState();
  return computeProjectsWithActiveTasks({
    isRecording: runtimeState.isRecording,
    recordingSessionId: transcriptState.recordingSessionId,
    isAgentRecording: transcriptState.isAgentRecording,
    agentRecordingHistoryId: transcriptState.agentRecordingHistoryId,
    sessions: transcriptState.sessions,
    queueItems: getBatchQueueItems(),
    historyItems: useHistoryStore.getState().items,
    locks: useDocumentMutationLockStore.getState().locks,
    tasks: useTaskLedgerStore.getState().tasks,
  });
}

export function useProjectsWithActiveTasks(): Set<string> {
  const isRecording = useTranscriptRuntimeStore((s) => s.isRecording);
  const [recordingSessionId, isAgentRecording, agentRecordingHistoryId, sessions] =
    useTranscriptStore(
      useShallow(
        (s) =>
          [s.recordingSessionId, s.isAgentRecording, s.agentRecordingHistoryId, s.sessions] as const
      )
    );
  const queueItems = useSyncExternalStore(
    subscribeBatchQueueItems,
    getBatchQueueItems,
    getBatchQueueItems
  );
  const historyItems = useHistoryStore((s) => s.items);
  const locks = useDocumentMutationLockStore((s) => s.locks);
  const tasks = useTaskLedgerStore((s) => s.tasks);

  return useMemo(
    () =>
      computeProjectsWithActiveTasks({
        isRecording,
        recordingSessionId,
        isAgentRecording,
        agentRecordingHistoryId,
        sessions,
        queueItems,
        historyItems,
        locks,
        tasks,
      }),
    [
      isRecording,
      recordingSessionId,
      isAgentRecording,
      agentRecordingHistoryId,
      sessions,
      queueItems,
      historyItems,
      locks,
      tasks,
    ]
  );
}
