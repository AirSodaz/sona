import { beforeEach, describe, expect, it, vi } from 'vitest';

const testContext = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('uuid', () => ({
  v4: () => 'history-1',
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn().mockImplementation((path) => `asset://${path}`),
  invoke: testContext.invokeMock,
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    error: vi.fn(),
    info: vi.fn(),
  },
}));

import { historyService } from '../historyService';

describe('historyService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('createLiveRecordingDraft persists a draft through the Rust bridge and normalizes the returned item', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(123456789);
    testContext.invokeMock.mockResolvedValue({
      item: {
        id: 'history-1',
        timestamp: 123456789,
        duration: 0,
        audioPath: 'history-1.webm',
        transcriptPath: 'history-1.json',
        title: 'Recording 1970-01-02 10-17-36',
        previewText: '',
        icon: 'system:mic',
        type: 'recording',
        searchContent: '',
        projectId: null,
        status: 'draft',
        draftSource: 'live_record',
      },
      audioAbsolutePath: 'C:\\AppData\\history\\history-1.webm',
    });

    const result = await historyService.createLiveRecordingDraft('webm', null, 'system:mic');

    expect(testContext.invokeMock).toHaveBeenCalledWith('history_create_live_draft', {
      item: expect.objectContaining({
        id: 'history-1',
        audioPath: 'history-1.webm',
        transcriptPath: 'history-1.json',
        status: 'draft',
        draftSource: 'live_record',
      }),
    });
    expect(result).toEqual(expect.objectContaining({
      audioAbsolutePath: 'C:\\AppData\\history\\history-1.webm',
      item: expect.objectContaining({
        id: 'history-1',
        status: 'draft',
      }),
    }));
  });

  it('deleteRecordings forwards the whole id set through one Rust command', async () => {
    testContext.invokeMock.mockResolvedValue(undefined);

    await historyService.deleteRecordings(['1', '3']);

    expect(testContext.invokeMock).toHaveBeenCalledWith('history_delete_items', {
      ids: ['1', '3'],
    });
  });

  it('loadTranscript still normalizes legacy timing payloads from the Rust bridge', async () => {
    testContext.invokeMock.mockResolvedValue([
      {
        id: 'seg-1',
        text: '你好',
        start: 0,
        end: 1,
        isFinal: true,
        tokens: ['你', '好'],
        timestamps: [0, 0.5],
      },
    ]);

    const segments = await historyService.loadTranscript('legacy.json');

    expect(testContext.invokeMock).toHaveBeenCalledWith('history_load_transcript', {
      filename: 'legacy.json',
    });
    expect(segments).toHaveLength(1);
    expect(segments?.[0].timing).toEqual(expect.objectContaining({
      level: 'token',
      source: 'model',
    }));
    expect(segments?.[0].timing?.units).toEqual([
      expect.objectContaining({ text: '你', start: 0, end: 0.5 }),
      expect.objectContaining({ text: '好', start: 0.5, end: 1 }),
    ]);
  });
});
