import { describe, expect, it } from 'vitest';
import { MockAssetPort, MockTransport } from '../../platform/drivers/mock';
import { createHistoryService } from '../historyService';

describe('HistoryService with MockTransport (zero native mocks)', () => {
  it('loads history items via transport without any @tauri-apps mocks', async () => {
    const transport = new MockTransport();
    const assets = new MockAssetPort();

    transport.setCommandHandler('history_list_items', async () => [
      {
        id: 'mock-1',
        timestamp: 1700000000,
        duration: 120,
        audioPath: 'audio.wav',
        transcriptPath: 'transcript.json',
        title: 'Meeting 1',
        previewText: 'Hello world',
        icon: 'system:mic',
        type: 'recording',
        searchContent: 'hello world',
        projectId: null,
        deletedAt: null,
        status: 'completed',
        draftSource: null,
      },
    ]);

    const service = createHistoryService({ transport, assets });
    const items = await service.getAll();

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: 'mock-1',
      title: 'Meeting 1',
      previewText: 'Hello world',
    });

    const invocations = transport.getInvocationsFor('history_list_items');
    expect(invocations).toHaveLength(1);
  });

  it('creates live recording draft via transport', async () => {
    const transport = new MockTransport();
    const assets = new MockAssetPort();

    transport.setCommandHandler('history_create_live_draft', async () => ({
      item: {
        id: 'draft-123',
        timestamp: 1700000001,
        duration: 0,
        audioPath: 'draft.wav',
        transcriptPath: 'draft.json',
        title: 'Draft Recording',
        previewText: '',
        icon: 'system:mic',
        type: 'recording',
        searchContent: '',
        projectId: 'project-abc',
        deletedAt: null,
        status: 'draft',
        draftSource: 'live_record',
      },
      audioAbsolutePath: '/tmp/draft.wav',
    }));

    const service = createHistoryService({ transport, assets });
    const result = await service.createLiveRecordingDraft('wav', 'project-abc');

    expect(result.audioAbsolutePath).toBe('/tmp/draft.wav');
    expect(result.item.id).toBe('draft-123');
    expect(result.item.status).toBe('draft');

    const invocations = transport.getInvocationsFor('history_create_live_draft');
    expect(invocations).toHaveLength(1);
    expect(invocations[0].args).toEqual({
      id: null,
      audioExtension: 'wav',
      projectId: 'project-abc',
      icon: 'system:mic',
    });
  });

  it('resolves audio URL through assetPort without Tauri convertFileSrc', async () => {
    const transport = new MockTransport();
    const assets = new MockAssetPort('custom-media://');

    transport.setCommandHandler(
      'history_resolve_audio_path',
      async () => '/data/recordings/audio.wav'
    );

    const service = createHistoryService({ transport, assets });
    const url = await service.getAudioUrl('item-123');

    expect(url).toBe('custom-media:///data/recordings/audio.wav');
  });

  it('completes live recording draft and normalizes response', async () => {
    const transport = new MockTransport();
    const assets = new MockAssetPort();

    transport.setCommandHandler('history_complete_live_draft', async () => ({
      id: 'draft-123',
      timestamp: 1700000001,
      duration: 35,
      audioPath: 'draft.wav',
      transcriptPath: 'draft.json',
      title: 'Finished Draft',
      previewText: 'Recognized text',
      icon: 'system:mic',
      type: 'recording',
      searchContent: 'Recognized text',
      projectId: null,
      deletedAt: null,
      status: 'completed',
      draftSource: 'live_record',
    }));

    const service = createHistoryService({ transport, assets });
    const completed = await service.completeLiveRecordingDraft(
      'draft-123',
      [{ id: 's1', text: 'Recognized text', start: 0, end: 35, isFinal: true }],
      35
    );

    expect(completed.id).toBe('draft-123');
    expect(completed.status).toBe('complete');
    expect(completed.duration).toBe(35);
  });
});
