import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_CONFIG, useConfigStore } from '../configStore';
import { getEffectiveConfigSnapshot, useEffectiveConfigStore } from '../effectiveConfigStore';
import { useProjectStore } from '../projectStore';
import { useTranscriptPlaybackStore } from '../transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../transcriptSessionStore';
import { useTranscriptSidecarStore } from '../transcriptSidecarStore';
import {
  applyTranscriptUpdate,
  clearActiveTranscriptSession,
  openTranscriptSession,
  syncSavedRecordingMeta,
} from '../transcriptCoordinator';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';

describe('Transcript Stores', () => {
  beforeEach(() => {
    resetTranscriptStores();
  });

  it('session store manages segment CRUD and clears edit state on delete', () => {
    const sessionStore = useTranscriptSessionStore.getState();
    const firstId = sessionStore.addSegment({
      text: 'First',
      start: 0,
      end: 1,
      isFinal: true,
    });
    const secondId = sessionStore.addSegment({
      text: 'Second',
      start: 1,
      end: 2,
      isFinal: false,
    });

    sessionStore.setEditingSegmentId(secondId);
    sessionStore.updateSegment(secondId, { text: 'Second updated' });
    sessionStore.deleteSegment(firstId);

    expect(useTranscriptSessionStore.getState().segments).toEqual([
      expect.objectContaining({
        id: secondId,
        text: 'Second updated',
      }),
    ]);
    expect(useTranscriptSessionStore.getState().editingSegmentId).toBe(secondId);

    sessionStore.deleteSegment(secondId);

    expect(useTranscriptSessionStore.getState().segments).toEqual([]);
    expect(useTranscriptSessionStore.getState().editingSegmentId).toBeNull();
  });

  it('opens and clears a transcript session, then syncs saved metadata back into session and sidecar state', () => {
    useTranscriptSidecarStore.getState().setSummaryState({
      activeTemplateId: 'meeting',
      record: {
        templateId: 'meeting',
        content: 'Unsaved summary',
        generatedAt: '2026-04-30T00:00:00.000Z',
        sourceFingerprint: 'fp-1',
      },
    }, 'current');

    openTranscriptSession({
      segments: [{ id: 'seg-1', text: 'Hello', start: 0, end: 1, isFinal: true }],
      sourceHistoryId: null,
      title: 'Draft',
      icon: 'system:mic',
      audioUrl: 'asset:///draft.wav',
    });

    expect(useTranscriptSessionStore.getState()).toEqual(expect.objectContaining({
      sourceHistoryId: null,
      title: 'Draft',
      icon: 'system:mic',
    }));
    expect(useTranscriptPlaybackStore.getState().audioUrl).toBe('asset:///draft.wav');

    syncSavedRecordingMeta('Saved title', 'history-1', 'system:file');

    expect(useTranscriptSessionStore.getState()).toEqual(expect.objectContaining({
      sourceHistoryId: 'history-1',
      title: 'Saved title',
      icon: 'system:file',
    }));
    expect(useTranscriptSidecarStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptSidecarStore.getState().getSummaryState('history-1').record?.content).toBe('Unsaved summary');

    clearActiveTranscriptSession({ clearAudio: true, title: 'Empty' });

    expect(useTranscriptSessionStore.getState()).toEqual(expect.objectContaining({
      segments: [],
      sourceHistoryId: null,
      title: 'Empty',
      icon: null,
    }));
    expect(useTranscriptPlaybackStore.getState()).toEqual(expect.objectContaining({
      audioFile: null,
      audioUrl: null,
      activeSegmentId: null,
      activeSegmentIndex: -1,
    }));
  });

  it('playback store tracks seek requests and active segment highlighting from the session snapshot', () => {
    openTranscriptSession({
      segments: [
        { id: 'seg-1', text: 'First', start: 0, end: 5, isFinal: true },
        { id: 'seg-2', text: 'Second', start: 5, end: 10, isFinal: true },
      ],
      sourceHistoryId: null,
    });

    const playbackStore = useTranscriptPlaybackStore.getState();
    playbackStore.requestSeek(7.5);

    expect(useTranscriptPlaybackStore.getState()).toEqual(expect.objectContaining({
      currentTime: 7.5,
      activeSegmentId: 'seg-2',
      activeSegmentIndex: 1,
      seekRequest: {
        time: 7.5,
        timestamp: expect.any(Number),
      },
    }));

    playbackStore.clearSession();

    expect(useTranscriptPlaybackStore.getState()).toEqual(expect.objectContaining({
      audioUrl: null,
      currentTime: 0,
      activeSegmentId: null,
      activeSegmentIndex: -1,
    }));
  });

  it('runtime store updates workbench and recording flags independently of transcript content', () => {
    const runtimeStore = useTranscriptRuntimeStore.getState();

    runtimeStore.setMode('batch');
    runtimeStore.setProcessingStatus('processing');
    runtimeStore.setProcessingProgress(45);
    runtimeStore.setIsRecording(true);
    runtimeStore.setIsPaused(true);
    runtimeStore.setIsCaptionMode(true);

    expect(useTranscriptRuntimeStore.getState()).toEqual(expect.objectContaining({
      mode: 'batch',
      processingStatus: 'processing',
      processingProgress: 45,
      isRecording: true,
      isPaused: true,
      isCaptionMode: true,
    }));
  });

  it('sidecar store updates llm and auto-save state and rekeys current summary records', () => {
    const sidecarStore = useTranscriptSidecarStore.getState();

    sidecarStore.updateLlmState({ isPolishing: true, polishProgress: 50 }, 'current');
    sidecarStore.setAutoSaveState('hist-1', 'saving');
    sidecarStore.setSummaryState({
      activeTemplateId: 'general',
      record: {
        templateId: 'general',
        content: 'Current summary',
        generatedAt: '2026-04-30T01:00:00.000Z',
        sourceFingerprint: 'fp-current',
      },
    }, 'current');
    sidecarStore.rekeyCurrentSummaryState('hist-2');

    expect(useTranscriptSidecarStore.getState().getLlmState('current')).toEqual(expect.objectContaining({
      isPolishing: true,
      polishProgress: 50,
    }));
    expect(useTranscriptSidecarStore.getState().autoSaveStates['hist-1']).toEqual({
      status: 'saving',
      updatedAt: expect.any(Number),
    });
    expect(useTranscriptSidecarStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptSidecarStore.getState().getSummaryState('hist-2').record?.content).toBe('Current summary');
  });

  it('applies transcript updates atomically and keeps active selection aligned', () => {
    openTranscriptSession({
      segments: [
        { id: 'seg-partial', text: 'Hello world.', start: 0, end: 2, isFinal: false },
      ],
      sourceHistoryId: null,
    });

    applyTranscriptUpdate({
      removeIds: ['seg-partial'],
      upsertSegments: [
        { id: 'seg-final-1', text: 'Hello.', start: 0, end: 1, isFinal: true },
        { id: 'seg-final-2', text: 'World.', start: 1, end: 2, isFinal: true },
      ],
    }, 'seg-final-2');

    expect(useTranscriptSessionStore.getState().segments.map((segment) => segment.id)).toEqual([
      'seg-final-1',
      'seg-final-2',
    ]);
    expect(useTranscriptPlaybackStore.getState()).toEqual(expect.objectContaining({
      activeSegmentId: 'seg-final-2',
      activeSegmentIndex: 1,
    }));
  });

  it('effective config store resolves active-project overrides and exposes a synchronous snapshot getter', () => {
    useConfigStore.setState((state) => ({
      ...state,
      config: {
        ...DEFAULT_CONFIG,
        summaryTemplateId: 'general',
        translationLanguage: 'zh',
      },
    }));
    useProjectStore.setState((state) => ({
      ...state,
      activeProjectId: 'project-1',
      projects: [
        {
          id: 'project-1',
          name: 'Project 1',
          description: '',
          icon: '',
          createdAt: 1,
          updatedAt: 1,
          defaults: {
            summaryTemplateId: 'meeting',
            translationLanguage: 'ja',
            polishPresetId: 'general',
            exportFileNamePrefix: '',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
            enabledPolishKeywordSetIds: [],
            enabledSpeakerProfileIds: [],
          },
        },
      ],
    }));

    useEffectiveConfigStore.getState().syncConfig();

    expect(useEffectiveConfigStore.getState().config).toEqual(expect.objectContaining({
      summaryTemplateId: 'meeting',
      translationLanguage: 'ja',
    }));
    expect(getEffectiveConfigSnapshot()).toEqual(expect.objectContaining({
      summaryTemplateId: 'meeting',
      translationLanguage: 'ja',
    }));
  });
});
