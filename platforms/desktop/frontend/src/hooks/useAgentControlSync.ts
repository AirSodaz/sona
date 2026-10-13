import { useEffect } from 'react';
import { historyService } from '../services/historyService';
import {
  type AgentRecordingStatusPayload,
  TauriEvent,
  type TranscriptUpdatedPayload,
  type UiRecordingStopRequestedPayload,
} from '../services/tauri/events';
import { listen, type UnlistenFn } from '../services/tauri/platform/events';
import { useHistoryStore } from '../stores/historyStore';
import { useMcpStore } from '../stores/mcpStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { stopActiveRecording } from '../stores/transcriptCoordinator';
import { sessionActions, transcriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { McpActivityLogEntry, McpMutationNotice } from '../types/mcp';
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
          async (event) => {
            if (!isMounted) return;
            const { active, isPaused = false, historyId } = event.payload;
            logger.info(
              `[AgentControlSync] Agent recording status changed: active=${active}, isPaused=${isPaused}, historyId=${historyId ?? 'none'}`
            );
            if (
              !active &&
              useTranscriptStore.getState().isRecording &&
              !useTranscriptStore.getState().isAgentRecording
            ) {
              await stopActiveRecording();
            }
            useTranscriptStore.getState().setIsRecording(active);
            useTranscriptStore.getState().setIsPaused(active ? isPaused : false);
            useTranscriptStore.getState().setIsAgentRecording(active, historyId ?? null);
            void useHistoryStore.getState().loadItems();
          }
        );
        if (isMounted) unlisteners.push(unlistenRec);
        else unlistenRec();

        // 1b. Listen for UI recording stop requested by Agent
        const unlistenStopReq = await listen<UiRecordingStopRequestedPayload>(
          TauriEvent.agent.uiRecordingStopRequested,
          async (event) => {
            if (!isMounted) return;
            const discard = event.payload?.discard ?? false;
            logger.info(
              `[AgentControlSync] UI recording stop requested by agent (discard=${discard})`
            );
            await stopActiveRecording({ discard });
          }
        );
        if (isMounted) unlisteners.push(unlistenStopReq);
        else unlistenStopReq();

        // 2. Listen for background transcript updates (transcribe_file, translation, summary mutations)
        const unlistenTrans = await listen<TranscriptUpdatedPayload>(
          TauriEvent.agent.transcriptUpdated,
          async (event) => {
            if (!isMounted) return;
            const historyId = event.payload?.historyId;
            logger.info(
              `[AgentControlSync] Transcript updated for historyId=${historyId}, refreshing history`
            );
            void useHistoryStore.getState().loadItems();
            if (historyId && transcriptSessionStore.getState().sourceHistoryId === historyId) {
              try {
                const updatedSegments = await historyService.loadTranscript(historyId);
                if (
                  isMounted &&
                  updatedSegments &&
                  transcriptSessionStore.getState().sourceHistoryId === historyId
                ) {
                  sessionActions.setSegments(updatedSegments);
                }
              } catch (error) {
                logger.error(
                  '[AgentControlSync] Failed to reload active transcript segments:',
                  error
                );
              }
            }
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

        // 4. Listen for MCP status changes
        const unlistenMcpStatus = await listen<void>(TauriEvent.mcp.statusChanged, () => {
          if (!isMounted) return;
          void useMcpStore.getState().fetchStatus();
        });
        if (isMounted) unlisteners.push(unlistenMcpStatus);
        else unlistenMcpStatus();

        // 5. Listen for MCP activity logs
        const unlistenMcpActivity = await listen<McpActivityLogEntry>(
          TauriEvent.mcp.activity,
          (event) => {
            if (!isMounted) return;
            if (event.payload) {
              useMcpStore.getState().addLogEntry(event.payload);
            }
          }
        );
        if (isMounted) unlisteners.push(unlistenMcpActivity);
        else unlistenMcpActivity();

        // 6. Listen for MCP mutation notice
        const unlistenMcpMutation = await listen<McpMutationNotice>(
          TauriEvent.mcp.mutationNotice,
          (event) => {
            if (!isMounted) return;
            logger.info('[AgentControlSync] MCP mutation notice:', event.payload);
          }
        );
        if (isMounted) unlisteners.push(unlistenMcpMutation);
        else unlistenMcpMutation();
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
