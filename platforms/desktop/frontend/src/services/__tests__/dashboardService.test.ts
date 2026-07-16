import { beforeEach, describe, expect, it, vi } from 'vitest';
import { dashboardService } from '../dashboardService';
import { getDashboardSnapshot } from '../tauri/dashboard';
import type { DashboardSnapshot } from '../../types/dashboard';

vi.mock('../tauri/dashboard', () => ({
  getDashboardSnapshot: vi.fn(),
}));

function displayNumber(value: number): string {
  return String(Math.round(value));
}

function displayDuration(seconds: number): string {
  const totalMinutes = Math.max(0, Math.round(seconds / 60));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function createUsageBucket(value = 0) {
  return {
    callCount: value,
    callCountDisplay: displayNumber(value),
    callsWithUsage: value,
    callsWithUsageDisplay: displayNumber(value),
    callsWithoutUsage: 0,
    callsWithoutUsageDisplay: '0',
    promptTokens: 0,
    promptTokensDisplay: '0',
    completionTokens: 0,
    completionTokensDisplay: '0',
    totalTokens: 0,
    totalTokensDisplay: '0',
  };
}

function createSnapshot(isDeepLoaded: boolean): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 1,
        itemCountDisplay: '1',
        tagCount: 0,
        tagCountDisplay: '0',
        totalDurationSeconds: 42,
        totalDurationDisplay: displayDuration(42),
        transcriptCharacterCount: isDeepLoaded ? 12 : undefined,
        transcriptCharacterCountDisplay: isDeepLoaded ? '12' : undefined,
        recordingCount: 1,
        recordingCountDisplay: '1',
        batchCount: 0,
        batchCountDisplay: '0',
        untaggedCount: 1,
        untaggedCountDisplay: '1',
        taggedCount: 0,
        taggedCountDisplay: '0',
        recentDailyItems: [],
        isDeepLoaded,
      },
      speakers: isDeepLoaded
        ? {
            annotatedItemCount: 0,
            annotatedItemCountDisplay: '0',
            speakerAttributedDuration: 0,
            speakerAttributedDurationDisplay: '0m',
            identifiedSpeakerCount: 0,
            identifiedSpeakerCountDisplay: '0',
            anonymousSpeakerSlotCount: 0,
            anonymousSpeakerSlotCountDisplay: '0',
            speakerTaggedSegmentCount: 0,
            speakerTaggedSegmentCountDisplay: '0',
            totalSegmentCount: 0,
            totalSegmentCountDisplay: '0',
            totalSegmentDuration: 0,
            totalSegmentDurationDisplay: '0m',
            identifiedDuration: 0,
            identifiedDurationDisplay: '0m',
            anonymousDuration: 0,
            anonymousDurationDisplay: '0m',
            segmentCoverageRatio: 0,
            segmentCoverageLabel: '0%',
            durationCoverageRatio: 0,
            durationCoverageLabel: '0%',
            topIdentifiedSpeakers: [],
            topIdentifiedSpeakerRows: [],
            topIdentifiedSpeakerMaxValue: 0,
            isDeepLoaded: true,
          }
        : null,
    },
    llmUsage: {
      startedAt: undefined,
      lastUpdatedAt: undefined,
      totals: createUsageBucket(),
      byProvider: [],
      byProviderTopRows: [],
      byProviderMaxValue: 0,
      byCategory: [],
      byCategoryTopRows: [],
      byCategoryMaxValue: 0,
      recentDaily: [],
    },
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

describe('dashboardService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads the fast snapshot through the dashboard Tauri command', async () => {
    const snapshot = createSnapshot(false);
    vi.mocked(getDashboardSnapshot).mockResolvedValue(snapshot);

    await expect(dashboardService.getFastSnapshot()).resolves.toBe(snapshot);

    expect(getDashboardSnapshot).toHaveBeenCalledWith({ deep: false });
  });

  it('loads the deep snapshot through the dashboard Tauri command', async () => {
    const snapshot = createSnapshot(true);
    vi.mocked(getDashboardSnapshot).mockResolvedValue(snapshot);

    await expect(dashboardService.getDeepSnapshot()).resolves.toBe(snapshot);

    expect(getDashboardSnapshot).toHaveBeenCalledWith({ deep: true });
  });
});
