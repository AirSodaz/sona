import { vi } from 'vitest';
import type { Mock } from 'vitest';
import type { LiveRecordingDraftHandle } from '../../services/historyService';
import type { HistoryItem } from '../../types/history';

type SegmentLike = {
  text?: string;
};

type SavedRecordingResult = {
  id: string;
  title: string;
  projectId: string | null;
};

type LiveRecordHistoryMockOptions = {
  saveRecordingResult?: SavedRecordingResult;
  saveNativeRecordingResult?: SavedRecordingResult;
  saveImportedFileResult?: unknown;
};

type LiveRecordHistoryMockFns = {
  mockSaveRecording: Mock<(...args: any[]) => any>;
  mockSaveNativeRecording: Mock<(...args: any[]) => any>;
  mockCreateLiveRecordingDraft: Mock<(...args: any[]) => any>;
  mockCompleteLiveRecordingDraft: Mock<(...args: any[]) => any>;
  mockDeleteRecording: Mock<(...args: any[]) => any>;
};

export function createLiveRecordingDraftHandle(
  id: string,
  extension = 'wav',
  overrides: Partial<HistoryItem> = {},
): LiveRecordingDraftHandle {
  return {
    item: {
      id,
      timestamp: 1,
      duration: 0,
      audioPath: `${id}.${extension}`,
      transcriptPath: `${id}.json`,
      title: `Recording ${id}`,
      previewText: '',
      projectId: null,
      icon: 'system:mic',
      type: 'recording',
      searchContent: '',
      status: 'draft',
      draftSource: 'live_record',
      ...overrides,
    },
    audioAbsolutePath: `C:/mock/history/${id}.${extension}`,
  };
}

export function createCompletedHistoryItem(
  id: string,
  extension = 'wav',
  overrides: Partial<HistoryItem> = {},
): HistoryItem {
  return {
    id,
    timestamp: 1,
    duration: 1,
    audioPath: `${id}.${extension}`,
    transcriptPath: `${id}.json`,
    title: `Recording ${id}`,
    previewText: '',
    projectId: null,
    icon: 'system:mic',
    type: 'recording',
    searchContent: '',
    status: 'complete',
    ...overrides,
  };
}

function getAudioExtension(draft: LiveRecordingDraftHandle) {
  return draft.item.audioPath.split('.').pop() || 'wav';
}

export function createLiveDraftRegistry() {
  let counter = 0;
  const handles = new Map<string, LiveRecordingDraftHandle>();

  const reset = () => {
    counter = 0;
    handles.clear();
  };

  const createNext = (
    audioExtension: string,
    projectId?: string | null,
    icon?: string | null,
  ) => {
    counter += 1;
    const draft = createLiveRecordingDraftHandle(`draft-${counter}`, audioExtension, {
      projectId: projectId ?? null,
      icon: icon ?? 'system:mic',
    });
    handles.set(draft.item.id, draft);
    return draft;
  };

  const discard = (historyId: string) => {
    handles.delete(historyId);
  };

  const complete = (
    historyId: string,
    segments: SegmentLike[],
    duration: number,
  ) => {
    const draft = handles.get(historyId) ?? createLiveRecordingDraftHandle(historyId);
    return createCompletedHistoryItem(historyId, getAudioExtension(draft), {
      title: draft.item.title,
      icon: draft.item.icon,
      projectId: draft.item.projectId,
      duration,
      previewText: segments[0]?.text || '',
      searchContent: segments.map((segment) => segment.text || '').join(' ').trim(),
    });
  };

  return {
    handles,
    reset,
    createNext,
    discard,
    complete,
  };
}

const liveDraftRegistries = new WeakMap<LiveRecordHistoryMockFns, ReturnType<typeof createLiveDraftRegistry>>();

export function getLiveDraftRegistry(mocks: LiveRecordHistoryMockFns) {
  let registry = liveDraftRegistries.get(mocks);

  if (!registry) {
    registry = createLiveDraftRegistry();
    liveDraftRegistries.set(mocks, registry);
  }

  return registry;
}

export function resetLiveRecordHistoryMocks(
  mocks: LiveRecordHistoryMockFns,
  options: LiveRecordHistoryMockOptions = {},
) {
  const drafts = getLiveDraftRegistry(mocks);
  const saveRecordingResult = options.saveRecordingResult ?? {
    id: 'test-id',
    title: 'Recording test',
    projectId: null,
  };
  const saveNativeRecordingResult = options.saveNativeRecordingResult ?? saveRecordingResult;

  drafts.reset();
  mocks.mockSaveRecording.mockReset();
  mocks.mockSaveNativeRecording.mockReset();
  mocks.mockCreateLiveRecordingDraft.mockReset();
  mocks.mockCompleteLiveRecordingDraft.mockReset();
  mocks.mockDeleteRecording.mockReset();

  mocks.mockSaveRecording.mockResolvedValue(saveRecordingResult);
  mocks.mockSaveNativeRecording.mockResolvedValue(saveNativeRecordingResult);
  mocks.mockDeleteRecording.mockImplementation(async (historyId: string) => {
    drafts.discard(historyId);
  });
  mocks.mockCreateLiveRecordingDraft.mockImplementation(
    async (
      audioExtension: string,
      projectId?: string | null,
      icon?: string | null,
    ) => drafts.createNext(audioExtension, projectId, icon),
  );
  mocks.mockCompleteLiveRecordingDraft.mockImplementation(
    async (historyId: string, segments: SegmentLike[], duration: number) =>
      drafts.complete(historyId, segments, duration),
  );
}

export function createLiveRecordHistoryServiceMockModule(
  mocks: LiveRecordHistoryMockFns,
  options: Pick<LiveRecordHistoryMockOptions, 'saveImportedFileResult'> = {},
) {
  const saveImportedFileResult = options.saveImportedFileResult ?? { id: 'test-id' };

  return {
    historyService: {
      createLiveRecordingDraft: (
        audioExtension: string,
        projectId?: string | null,
        icon?: string | null,
      ) => mocks.mockCreateLiveRecordingDraft(audioExtension, projectId, icon),
      completeLiveRecordingDraft: (
        historyId: string,
        segments: SegmentLike[],
        duration: number,
      ) => mocks.mockCompleteLiveRecordingDraft(historyId, segments, duration),
      discardLiveRecordingDraft: (historyId: string) => mocks.mockDeleteRecording(historyId),
      deleteRecording: (historyId: string) => mocks.mockDeleteRecording(historyId),
      updateTranscript: vi.fn().mockResolvedValue(undefined),
      saveRecording: (blob: Blob, segments: unknown, duration: number) =>
        mocks.mockSaveRecording(blob, segments, duration),
      saveNativeRecording: (path: string, segments: unknown, duration: number) =>
        mocks.mockSaveNativeRecording(path, segments, duration),
      saveImportedFile: vi.fn().mockResolvedValue(saveImportedFileResult),
      saveTranscriptFile: vi.fn(),
      getAll: vi.fn().mockResolvedValue([]),
      init: vi.fn(),
    },
  };
}

export function createLiveRecordHistoryMockController(
  options: LiveRecordHistoryMockOptions = {},
) {
  const mocks: LiveRecordHistoryMockFns = {
    mockSaveRecording: vi.fn(),
    mockSaveNativeRecording: vi.fn(),
    mockCreateLiveRecordingDraft: vi.fn(),
    mockCompleteLiveRecordingDraft: vi.fn(),
    mockDeleteRecording: vi.fn(),
  };

  const reset = () => {
    resetLiveRecordHistoryMocks(mocks, options);
  };

  reset();

  return {
    drafts: getLiveDraftRegistry(mocks),
    ...mocks,
    reset,
    createMockModule: () => createLiveRecordHistoryServiceMockModule(mocks, options),
  };
}
