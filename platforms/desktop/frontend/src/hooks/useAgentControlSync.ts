import { useEffect } from 'react';
import {
  type AgentRecordingStatusPayload,
  TauriEvent,
  type TranscriptUpdatedPayload,
} from '../services/tauri/events';
import { listen, type UnlistenFn } from '../services/tauri/platform/events';
import { useHistoryStore } from '../stores/historyStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { logger } from '../utils/logger';

/**
 * Hook to synchronize MCP / Agent Control background activities with the desktop UI:
 * 1. Agent live recording status & history draft updates
 * 2. Background transcript mutations (auto-refreshing historyStore)
 * 3. Batch transcription progress for MCP tasks in the Task Ledger
 */
export function useAgentControlSync(): void {
  useEffect(() => {
    let isMounted = true;
    const unlisteners: UnlistenFn[] = [];

    const setupListeners = async () => {
      try {
        // 1. Listen for Agent Recording status changes
        const unlistenRec = await listen<AgentRecordingStatusPayload>(
          TauriEvent.agent.recordingStatus,
          (event) => {
            if (!isMounted) return;
            const { active, isPaused = false, historyId } = event.payload;
            logger.info(
              `[AgentControlSync] Agent recording status changed: active=${active}, isPaused=${isPaused}, historyId=${historyId ?? 'none'}`
            );
            useTranscriptRuntimeStore.getState().setIsRecording(active);
            useTranscriptRuntimeStore.getState().setIsPaused(active ? isPaused : false);
            useTranscriptStore.getState().setIsRecording(active);
            useTranscriptStore.getState().setIsPaused(active ? isPaused : false);
            useTranscriptStore.getState().setIsAgentRecording(active, historyId ?? null);
            void useHistoryStore.getState().loadItems();
          }
        );
        if (isMounted) unlisteners.push(unlistenRec);
        else unlistenRec();

        // 2. Listen for background transcript updates (transcribe_file, translation, summary mutations)
        const unlistenTrans = await listen<TranscriptUpdatedPayload>(
          TauriEvent.agent.transcriptUpdated,
          (event) => {
            if (!isMounted) return;
            logger.info(
              `[AgentControlSync] Transcript updated for historyId=${event.payload?.historyId}, refreshing history`
            );
            void useHistoryStore.getState().loadItems();
          }
        );
        if (isMounted) unlisteners.push(unlistenTrans);
        else unlistenTrans();

        // 3. Listen for batch file transcription progress to update Task Ledger
        const unlistenBatch = await listen<unknown>(TauriEvent.app.batchProgress, (event) => {
          if (!isMounted) return;
          if (!Array.isArray(event.payload) || event.payload.length < 2) return;
          const [, progress, instanceId] = event.payload;
          if (typeof instanceId !== 'string' || !instanceId) return;

          const numericProgress = typeof progress === 'number' ? progress : Number(progress);
          if (!Number.isFinite(numericProgress)) return;

          const tasks = useTaskLedgerStore.getState().tasks;
          const targetTask = tasks.find((t) => t.id === instanceId);
          if (targetTask) {
            void useTaskLedgerStore
              .getState()
              .patchTask(instanceId, { progress: numericProgress }, { transient: true });
          }
        });
        if (isMounted) unlisteners.push(unlistenBatch);
        else unlistenBatch();
      } catch (error) {
        logger.error('[AgentControlSync] Failed to setup listeners:', error);
      }
    };

    void setupListeners();

    return () => {
      isMounted = false;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, []);
}
