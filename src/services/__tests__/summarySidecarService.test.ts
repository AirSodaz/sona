import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_SUMMARY_TEMPLATE_ID } from '../../types/transcript';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { summarySidecarService } from '../summarySidecarService';

const historyServiceMock = vi.hoisted(() => ({
  loadSummary: vi.fn(),
  saveSummary: vi.fn(),
  deleteSummary: vi.fn(),
}));

vi.mock('../historyService', () => ({
  historyService: historyServiceMock,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((innerResolve) => {
    resolve = innerResolve;
  });
  return { promise, resolve };
}

describe('summarySidecarService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTranscriptStore.setState({
      sourceHistoryId: null,
      summaryStates: {},
    });
  });

  it('does not overwrite in-memory summary state when a sidecar load resolves late', async () => {
    const load = deferred<Awaited<ReturnType<typeof summarySidecarService.loadSummaryPayload>>>();
    historyServiceMock.loadSummary.mockReturnValue(load.promise);

    const pendingLoad = summarySidecarService.loadSummary('history-a');
    useTranscriptStore.getState().setSummaryState({
      activeTemplateId: 'meeting',
      record: {
        templateId: 'meeting',
        content: 'Manual summary',
        generatedAt: '2026-05-26T00:00:00.000Z',
        sourceFingerprint: 'manual-fingerprint',
      },
    }, 'history-a');

    load.resolve({
      activeTemplateId: 'lecture',
      record: {
        templateId: 'lecture',
        content: 'Disk summary',
        generatedAt: '2026-05-25T00:00:00.000Z',
        sourceFingerprint: 'disk-fingerprint',
      },
    });
    await pendingLoad;

    expect(historyServiceMock.loadSummary).toHaveBeenCalledWith('history-a');
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.content).toBe('Manual summary');
  });

  it('deletes persisted summary sidecars when the local state is default-only', async () => {
    useTranscriptStore.getState().setSummaryState({
      activeTemplateId: DEFAULT_SUMMARY_TEMPLATE_ID,
      record: undefined,
      isGenerating: false,
      generationProgress: 0,
    }, 'history-empty');

    await summarySidecarService.persistSummary('history-empty');

    expect(historyServiceMock.deleteSummary).toHaveBeenCalledWith('history-empty');
    expect(historyServiceMock.saveSummary).not.toHaveBeenCalled();
  });

  it('saves manual summary records using the durable sidecar payload shape', async () => {
    useTranscriptStore.getState().setSummaryState({
      activeTemplateId: 'meeting',
      record: {
        templateId: 'meeting',
        content: 'Manual summary',
        generatedAt: '2026-05-26T00:00:00.000Z',
        sourceFingerprint: 'manual-fingerprint',
      },
      streamingContent: 'transient text',
    }, 'history-manual');

    await summarySidecarService.persistSummary('history-manual');

    expect(historyServiceMock.saveSummary).toHaveBeenCalledWith('history-manual', {
      activeTemplateId: 'meeting',
      record: {
        templateId: 'meeting',
        content: 'Manual summary',
        generatedAt: '2026-05-26T00:00:00.000Z',
        sourceFingerprint: 'manual-fingerprint',
      },
    });
    expect(historyServiceMock.deleteSummary).not.toHaveBeenCalled();
  });
});
