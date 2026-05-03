import { beforeEach, describe, expect, it, vi } from 'vitest';
import { dashboardService } from '../dashboardService';
import { getDashboardSnapshot } from '../tauri/dashboard';
import type { DashboardSnapshot } from '../../types/dashboard';

vi.mock('../tauri/dashboard', () => ({
  getDashboardSnapshot: vi.fn(),
}));

function createSnapshot(isDeepLoaded: boolean): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 1,
        projectCount: 0,
        totalDurationSeconds: 42,
        transcriptCharacterCount: isDeepLoaded ? 12 : undefined,
        recordingCount: 1,
        batchCount: 0,
        inboxCount: 1,
        projectAssignedCount: 0,
        recentDailyItems: [],
        isDeepLoaded,
      },
      speakers: isDeepLoaded
        ? {
            annotatedItemCount: 0,
            speakerAttributedDuration: 0,
            identifiedSpeakerCount: 0,
            anonymousSpeakerSlotCount: 0,
            speakerTaggedSegmentCount: 0,
            totalSegmentCount: 0,
            totalSegmentDuration: 0,
            identifiedDuration: 0,
            anonymousDuration: 0,
            topIdentifiedSpeakers: [],
            isDeepLoaded: true,
          }
        : null,
    },
    llmUsage: {
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
