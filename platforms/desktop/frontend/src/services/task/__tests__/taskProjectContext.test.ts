import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { useBatchQueueStore } from '../../../stores/batchQueueStore';
import { useHistoryStore } from '../../../stores/historyStore';
import { useProjectStore } from '../../../stores/projectStore';
import { useTaskLedgerStore } from '../../../stores/taskLedgerStore';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from '../../../stores/transcriptStore';
import type { EffectivePipelineSnapshot } from '../../../types/project';
import { useDocumentMutationLockStore } from '../documentMutationCoordinator';
import {
  getProjectsWithActiveTasks,
  resolveTaskProjectContext,
  useProjectsWithActiveTasks,
  useTaskProjectContext,
  useTaskProjectId,
} from '../taskProjectContext';

function makeSnapshot(overrides?: Partial<EffectivePipelineSnapshot>): EffectivePipelineSnapshot {
  return {
    enabled: true,
    autoPolish: true,
    autoTranslate: false,
    autoSummary: false,
    autoExport: false,
    customTerms: [],
    isProjectPipeline: true,
    ...overrides,
  };
}

describe('taskProjectContext', () => {
  beforeEach(() => {
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null });
    useHistoryStore.setState({ items: [] });
    useProjectStore.setState({ projects: [], activeProjectId: null });
    useTaskLedgerStore.setState({ tasks: [] });
    useDocumentMutationLockStore.setState({ locks: {} });
    useTranscriptStore.setState({
      activeSessionId: 'default',
      recordingSessionId: null,
      isRecording: false,
      isAgentRecording: false,
      agentRecordingHistoryId: null,
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
      },
    });
  });

  describe('resolveTaskProjectContext', () => {
    it('resolves batchQueue > history > session > activeProject precedence', () => {
      const batchSnapshot = makeSnapshot({ targetLanguage: 'ja' });
      const sessionSnapshot = makeSnapshot({ targetLanguage: 'fr' });

      useProjectStore.setState({ activeProjectId: 'proj-active' });
      useHistoryStore.setState({
        items: [
          {
            id: 'hist-1',
            timestamp: 1,
            duration: 10,
            audioPath: '',
            transcriptPath: '',
            title: 'Hist 1',
            previewText: '',
            projectId: 'proj-hist',
          },
        ],
      });
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'batch-1',
            filename: 'a.mp3',
            filePath: '/a.mp3',
            status: 'processing',
            progress: 10,
            segments: [],
            projectId: 'proj-batch',
            pipelineSnapshot: batchSnapshot,
          },
        ],
      });
      useTranscriptStore.setState({
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'batch-1': {
            ...DEFAULT_SESSION_DATA,
            sourceHistoryId: 'hist-1',
            projectId: 'proj-session',
            pipelineSnapshot: sessionSnapshot,
          },
          'hist-sess': {
            ...DEFAULT_SESSION_DATA,
            sourceHistoryId: 'hist-1',
            projectId: 'proj-session',
            pipelineSnapshot: sessionSnapshot,
          },
          'inbox-sess': {
            ...DEFAULT_SESSION_DATA,
            projectId: null,
            pipelineSnapshot: sessionSnapshot,
          },
          'unbound-sess': {
            ...DEFAULT_SESSION_DATA,
          },
        },
      });

      expect(resolveTaskProjectContext({ sessionId: 'batch-1' })).toEqual({
        projectId: 'proj-batch',
        pipelineSnapshot: batchSnapshot,
        source: 'batchQueue',
      });

      expect(resolveTaskProjectContext({ sessionId: 'hist-sess' })).toEqual({
        projectId: 'proj-hist',
        pipelineSnapshot: sessionSnapshot,
        source: 'history',
      });

      expect(resolveTaskProjectContext({ sessionId: 'inbox-sess' })).toEqual({
        projectId: null,
        pipelineSnapshot: sessionSnapshot,
        source: 'session',
      });

      expect(resolveTaskProjectContext({ sessionId: 'unbound-sess' })).toEqual({
        projectId: 'proj-active',
        source: 'activeProject',
      });

      expect(
        resolveTaskProjectContext({ sessionId: 'unbound-sess', fallbackToActive: false })
      ).toEqual({
        projectId: null,
        source: 'none',
      });
    });

    it('normalizes "current" historyId and preserves explicit null on batch queue item', () => {
      useProjectStore.setState({ activeProjectId: 'proj-active' });
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'batch-inbox',
            filename: 'inbox.mp3',
            filePath: '/inbox.mp3',
            status: 'pending',
            progress: 0,
            segments: [],
            projectId: null,
          },
        ],
      });

      expect(resolveTaskProjectContext({ sessionId: 'batch-inbox' })).toEqual({
        projectId: null,
        pipelineSnapshot: undefined,
        source: 'batchQueue',
      });

      expect(resolveTaskProjectContext({ historyId: 'current', fallbackToActive: false })).toEqual({
        projectId: null,
        source: 'none',
      });
    });
  });

  describe('reactive hooks', () => {
    it('reactively updates useTaskProjectContext and useTaskProjectId when stores change', () => {
      useProjectStore.setState({ activeProjectId: 'proj-a' });
      useTranscriptStore.setState({
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'sess-1': { ...DEFAULT_SESSION_DATA },
        },
      });

      const { result } = renderHook(() => ({
        ctx: useTaskProjectContext('sess-1', null),
        id: useTaskProjectId('sess-1', null),
      }));

      expect(result.current.ctx).toEqual({ projectId: 'proj-a', source: 'activeProject' });
      expect(result.current.id).toBe('proj-a');

      act(() => {
        useTranscriptStore.getState().setSessionProjectContext('sess-1', 'proj-b');
      });

      expect(result.current.ctx).toEqual({
        projectId: 'proj-b',
        pipelineSnapshot: undefined,
        source: 'session',
      });
      expect(result.current.id).toBe('proj-b');
    });
  });

  describe('getProjectsWithActiveTasks and useProjectsWithActiveTasks', () => {
    it('collects active projects across recording, batch queue, document locks, and task ledger', () => {
      useTranscriptStore.setState({
        isRecording: true,
        recordingSessionId: 'rec-sess',
        isAgentRecording: true,
        agentRecordingHistoryId: 'agent-hist',
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'rec-sess': { ...DEFAULT_SESSION_DATA, projectId: 'proj-rec' },
          'lock-sess': { ...DEFAULT_SESSION_DATA, projectId: 'proj-lock-sess' },
        },
      });
      useHistoryStore.setState({
        items: [
          {
            id: 'agent-hist',
            timestamp: 1,
            duration: 5,
            audioPath: '',
            transcriptPath: '',
            title: 'Agent',
            previewText: '',
            projectId: 'proj-agent',
          },
          {
            id: 'lock-hist',
            timestamp: 2,
            duration: 5,
            audioPath: '',
            transcriptPath: '',
            title: 'Lock Hist',
            previewText: '',
            projectId: 'proj-lock-hist',
          },
          {
            id: 'ledger-hist',
            timestamp: 3,
            duration: 5,
            audioPath: '',
            transcriptPath: '',
            title: 'Ledger Hist',
            previewText: '',
            projectId: 'proj-ledger-hist',
          },
        ],
      });
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'b-pending',
            filename: '1.mp3',
            filePath: '/1.mp3',
            status: 'pending',
            progress: 0,
            segments: [],
            projectId: 'proj-batch-pending',
          },
          {
            id: 'b-processing',
            filename: '2.mp3',
            filePath: '/2.mp3',
            status: 'processing',
            progress: 50,
            segments: [],
            projectId: 'proj-batch-processing',
          },
          {
            id: 'b-complete',
            filename: '3.mp3',
            filePath: '/3.mp3',
            status: 'complete',
            progress: 100,
            segments: [],
            projectId: 'proj-batch-complete',
          },
          {
            id: 'b-ledger-lookup',
            filename: '4.mp3',
            filePath: '/4.mp3',
            status: 'error',
            progress: 0,
            segments: [],
            projectId: 'proj-ledger-batch',
          },
        ],
      });
      useDocumentMutationLockStore.setState({
        locks: {
          'session:lock-sess': { depth: 1, editorLockDepth: 1, taskTypes: ['polish'] },
          'lock-hist': { depth: 2, editorLockDepth: 1, taskTypes: ['translate'] },
          'zero-hist': { depth: 0, editorLockDepth: 0, taskTypes: [] },
        },
      });
      useTaskLedgerStore.setState({
        tasks: [
          {
            id: 'llm-1',
            kind: 'llmPolish',
            status: 'running',
            title: 'Polish',
            progress: 10,
            createdAt: 1,
            updatedAt: 1,
            retryable: true,
            cancelable: true,
            recoverable: false,
            tagIds: ['proj-ledger-tag'],
          },
          {
            id: 'llm-2',
            kind: 'llmSummary',
            status: 'pending',
            title: 'Summary',
            progress: 0,
            createdAt: 1,
            updatedAt: 1,
            retryable: true,
            cancelable: true,
            recoverable: false,
            historyId: 'ledger-hist',
          },
          {
            id: 'batch-b-ledger-lookup',
            kind: 'batchImport',
            status: 'cancelRequested',
            title: 'Batch lookup',
            progress: 30,
            createdAt: 1,
            updatedAt: 1,
            retryable: true,
            cancelable: true,
            recoverable: false,
          },
          {
            id: 'llm-done',
            kind: 'llmPolish',
            status: 'succeeded',
            title: 'Done',
            progress: 100,
            createdAt: 1,
            updatedAt: 1,
            retryable: false,
            cancelable: false,
            recoverable: false,
            tagIds: ['proj-ledger-done'],
          },
        ],
      });

      const active = getProjectsWithActiveTasks();
      expect(Array.from(active).sort()).toEqual([
        'proj-agent',
        'proj-batch-pending',
        'proj-batch-processing',
        'proj-ledger-batch',
        'proj-ledger-hist',
        'proj-ledger-tag',
        'proj-lock-hist',
        'proj-lock-sess',
        'proj-rec',
      ]);
    });

    it('ignores stale recordingSessionId when isRecording is false and updates hook when task resolves', () => {
      useTranscriptStore.setState({
        isRecording: false,
        recordingSessionId: 'stale-rec',
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'stale-rec': { ...DEFAULT_SESSION_DATA, projectId: 'proj-stale' },
        },
      });
      useTaskLedgerStore.setState({
        tasks: [
          {
            id: 'llm-c',
            kind: 'llmPolish',
            status: 'running',
            title: 'Polish C',
            progress: 10,
            createdAt: 1,
            updatedAt: 1,
            retryable: true,
            cancelable: true,
            recoverable: false,
            tagIds: ['proj-c'],
          },
        ],
      });

      const { result } = renderHook(() => useProjectsWithActiveTasks());
      expect(result.current.has('proj-stale')).toBe(false);
      expect(result.current.has('proj-c')).toBe(true);

      act(() => {
        useTaskLedgerStore.setState({
          tasks: [
            {
              id: 'llm-c',
              kind: 'llmPolish',
              status: 'succeeded',
              title: 'Polish C',
              progress: 100,
              createdAt: 1,
              updatedAt: 2,
              retryable: false,
              cancelable: false,
              recoverable: false,
              tagIds: ['proj-c'],
            },
          ],
        });
      });

      expect(result.current.has('proj-c')).toBe(false);
    });
  });
});
