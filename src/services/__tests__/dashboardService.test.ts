import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { dashboardService } from '../dashboardService';
import { llmUsageService } from '../llmUsageService';

vi.mock('@tauri-apps/plugin-fs', () => ({
  readTextFile: vi.fn(),
  BaseDirectory: { AppLocalData: 3 },
}));

vi.mock('../llmUsageService', () => ({
  llmUsageService: {
    getStats: vi.fn(),
  },
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    warn: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

function createEmptyUsageStats() {
  return {
    startedAt: undefined,
    lastUpdatedAt: undefined,
    totals: {
      callCount: 0,
      callsWithUsage: 0,
      callsWithoutUsage: 0,
      promptTokens: 0,
      completionTokens: 0,
      totalTokens: 0,
    },
    byProvider: [],
    byCategory: [],
    recentDaily: [],
  };
}

function mockFiles(files: Record<string, unknown>) {
  vi.mocked(readTextFile).mockImplementation(async (path) => {
    const normalizedPath = String(path);
    if (!(normalizedPath in files)) {
      throw new Error(`missing mock for ${normalizedPath}`);
    }

    const value = files[normalizedPath];
    return typeof value === 'string' ? value : JSON.stringify(value);
  });
}

describe('dashboardService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(llmUsageService.getStats).mockResolvedValue(createEmptyUsageStats());
  });

  it('aggregates transcript characters and speaker insights from saved transcripts', async () => {
    mockFiles({
      'history/index.json': [
        {
          id: 'hist-1',
          timestamp: new Date('2026-04-20T10:00:00Z').getTime(),
          duration: 120,
          audioPath: 'hist-1.wav',
          transcriptPath: 'hist-1.json',
          title: 'Recording 1',
          previewText: '',
          type: 'recording',
          projectId: null,
        },
        {
          id: 'hist-2',
          timestamp: new Date('2026-04-22T10:00:00Z').getTime(),
          duration: 60,
          audioPath: 'hist-2.wav',
          transcriptPath: 'hist-2.json',
          title: 'Recording 2',
          previewText: '',
          type: 'batch',
          projectId: 'project-1',
        },
      ],
      'projects/index.json': [{ id: 'project-1' }, { id: 'project-2' }],
      'history/hist-1.json': [
        {
          id: 'seg-1',
          start: 0,
          end: 30,
          text: 'hello world',
          speaker: { id: 'speaker-alice', label: 'Alice', kind: 'identified' },
        },
        {
          id: 'seg-2',
          start: 30,
          end: 60,
          text: 'anonymous part',
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
        {
          id: 'seg-3',
          start: 60,
          end: 120,
          text: 'plain text',
        },
      ],
      'history/hist-2.json': [
        {
          id: 'seg-4',
          start: 0,
          end: 20,
          text: 'second anonymous',
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
        {
          id: 'seg-5',
          start: 20,
          end: 60,
          text: 'alice again',
          speaker: { id: 'speaker-alice', label: 'Alice', kind: 'identified' },
        },
      ],
    });

    const snapshot = await dashboardService.getDeepSnapshot();

    expect(snapshot.content.overview.itemCount).toBe(2);
    expect(snapshot.content.overview.recordingCount).toBe(1);
    expect(snapshot.content.overview.batchCount).toBe(1);
    expect(snapshot.content.overview.inboxCount).toBe(1);
    expect(snapshot.content.overview.projectAssignedCount).toBe(1);
    expect(snapshot.content.overview.projectCount).toBe(2);
    expect(snapshot.content.overview.transcriptCharacterCount).toBe(
      'hello world'.length
        + 'anonymous part'.length
        + 'plain text'.length
        + 'second anonymous'.length
        + 'alice again'.length,
    );

    expect(snapshot.content.speakers).not.toBeNull();
    expect(snapshot.content.speakers?.annotatedItemCount).toBe(2);
    expect(snapshot.content.speakers?.speakerAttributedDuration).toBe(120);
    expect(snapshot.content.speakers?.identifiedSpeakerCount).toBe(1);
    expect(snapshot.content.speakers?.anonymousSpeakerSlotCount).toBe(2);
    expect(snapshot.content.speakers?.speakerTaggedSegmentCount).toBe(4);
    expect(snapshot.content.speakers?.totalSegmentCount).toBe(5);
    expect(snapshot.content.speakers?.identifiedDuration).toBe(70);
    expect(snapshot.content.speakers?.anonymousDuration).toBe(50);
    expect(snapshot.content.speakers?.topIdentifiedSpeakers).toEqual([
      {
        speakerId: 'speaker-alice',
        label: 'Alice',
        durationSeconds: 70,
        segmentCount: 2,
        itemCount: 2,
      },
    ]);
  });

  it('does not deduplicate anonymous speakers across different history items', async () => {
    mockFiles({
      'history/index.json': [
        {
          id: 'hist-a',
          timestamp: new Date('2026-04-20T10:00:00Z').getTime(),
          duration: 40,
          audioPath: 'hist-a.wav',
          transcriptPath: 'hist-a.json',
          title: 'A',
          previewText: '',
          projectId: null,
        },
        {
          id: 'hist-b',
          timestamp: new Date('2026-04-21T10:00:00Z').getTime(),
          duration: 40,
          audioPath: 'hist-b.wav',
          transcriptPath: 'hist-b.json',
          title: 'B',
          previewText: '',
          projectId: null,
        },
      ],
      'projects/index.json': [],
      'history/hist-a.json': [
        {
          id: 'seg-a1',
          start: 0,
          end: 20,
          text: 'a',
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
      ],
      'history/hist-b.json': [
        {
          id: 'seg-b1',
          start: 0,
          end: 20,
          text: 'b',
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
      ],
    });

    const snapshot = await dashboardService.getDeepSnapshot();

    expect(snapshot.content.speakers?.anonymousSpeakerSlotCount).toBe(2);
    expect(snapshot.content.speakers?.identifiedSpeakerCount).toBe(0);
    expect(snapshot.content.speakers?.topIdentifiedSpeakers).toEqual([]);
  });

  it('keeps speaker insights empty when transcripts have no speaker tags', async () => {
    mockFiles({
      'history/index.json': [
        {
          id: 'hist-plain',
          timestamp: new Date('2026-04-20T10:00:00Z').getTime(),
          duration: 50,
          audioPath: 'hist-plain.wav',
          transcriptPath: 'hist-plain.json',
          title: 'Plain',
          previewText: '',
          projectId: null,
        },
      ],
      'projects/index.json': [],
      'history/hist-plain.json': [
        { id: 'seg-1', start: 0, end: 20, text: 'first line' },
        { id: 'seg-2', start: 20, end: 50, text: 'second line' },
      ],
    });

    const snapshot = await dashboardService.getDeepSnapshot();

    expect(snapshot.content.speakers).toEqual({
      annotatedItemCount: 0,
      speakerAttributedDuration: 0,
      identifiedSpeakerCount: 0,
      anonymousSpeakerSlotCount: 0,
      speakerTaggedSegmentCount: 0,
      totalSegmentCount: 2,
      totalSegmentDuration: 50,
      identifiedDuration: 0,
      anonymousDuration: 0,
      topIdentifiedSpeakers: [],
      isDeepLoaded: true,
    });
  });
});
