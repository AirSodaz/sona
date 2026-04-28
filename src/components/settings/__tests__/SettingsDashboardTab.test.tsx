import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  buildTrendGeometry,
  normalizeTrendPoints,
  SettingsDashboardTab,
} from '../SettingsDashboardTab';
import type { DashboardSnapshot } from '../../../types/dashboard';
import { dashboardService } from '../../../services/dashboardService';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../../../services/dashboardService', () => ({
  dashboardService: {
    getFastSnapshot: vi.fn(),
    getDeepSnapshot: vi.fn(),
  },
}));

function createTrendDateKey(index: number): string {
  const date = new Date(Date.UTC(2026, 2, index + 1));
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, '0');
  const day = String(date.getUTCDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function createContentTrend(values: number[]) {
  return values.map((value, index) => ({
    date: createTrendDateKey(index),
    itemCount: value,
    durationSeconds: value * 60,
  }));
}

function createUsageTrend(values: number[]) {
  return values.map((value, index) => ({
    date: createTrendDateKey(index),
    callCount: value > 0 ? 1 : 0,
    callsWithUsage: value > 0 ? 1 : 0,
    callsWithoutUsage: 0,
    promptTokens: value,
    completionTokens: 0,
    totalTokens: value,
  }));
}

function createUsage(callCount = 4, recentValues: number[] = []) {
  return {
    startedAt: '2026-04-01T00:00:00.000Z',
    lastUpdatedAt: '2026-04-28T00:00:00.000Z',
    totals: {
      callCount,
      callsWithUsage: callCount,
      callsWithoutUsage: 0,
      promptTokens: 1200,
      completionTokens: 600,
      totalTokens: 1800,
    },
    byProvider: [],
    byCategory: [],
    recentDaily: createUsageTrend(recentValues),
  };
}

function createFastSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 3,
        projectCount: 2,
        totalDurationSeconds: 300,
        transcriptCharacterCount: undefined,
        recordingCount: 2,
        batchCount: 1,
        inboxCount: 1,
        projectAssignedCount: 2,
        recentDailyItems: [],
        isDeepLoaded: false,
      },
      speakers: null,
    },
    llmUsage: createUsage(),
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

function createDeepSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 3,
        projectCount: 2,
        totalDurationSeconds: 300,
        transcriptCharacterCount: 1234,
        recordingCount: 2,
        batchCount: 1,
        inboxCount: 1,
        projectAssignedCount: 2,
        recentDailyItems: [],
        isDeepLoaded: true,
      },
      speakers: {
        annotatedItemCount: 2,
        speakerAttributedDuration: 180,
        identifiedSpeakerCount: 1,
        anonymousSpeakerSlotCount: 2,
        speakerTaggedSegmentCount: 5,
        totalSegmentCount: 6,
        totalSegmentDuration: 240,
        identifiedDuration: 120,
        anonymousDuration: 60,
        topIdentifiedSpeakers: [
          {
            speakerId: 'speaker-alice',
            label: 'Alice',
            durationSeconds: 120,
            segmentCount: 3,
            itemCount: 2,
          },
        ],
        isDeepLoaded: true,
      },
    },
    llmUsage: createUsage(),
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

function createDeepSnapshotWithTrends(values: number[]): DashboardSnapshot {
  const snapshot = createDeepSnapshot();

  return {
    ...snapshot,
    content: {
      ...snapshot.content,
      overview: {
        ...snapshot.content.overview,
        recentDailyItems: createContentTrend(values),
      },
    },
    llmUsage: createUsage(snapshot.llmUsage.totals.callCount, values.map((value) => value * 100)),
  };
}

function createAnonymousSpeakerSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 3,
        projectCount: 2,
        totalDurationSeconds: 300,
        transcriptCharacterCount: 1234,
        recordingCount: 2,
        batchCount: 1,
        inboxCount: 1,
        projectAssignedCount: 2,
        recentDailyItems: [],
        isDeepLoaded: true,
      },
      speakers: {
        annotatedItemCount: 1,
        speakerAttributedDuration: 90,
        identifiedSpeakerCount: 0,
        anonymousSpeakerSlotCount: 2,
        speakerTaggedSegmentCount: 3,
        totalSegmentCount: 6,
        totalSegmentDuration: 240,
        identifiedDuration: 0,
        anonymousDuration: 90,
        topIdentifiedSpeakers: [],
        isDeepLoaded: true,
      },
    },
    llmUsage: createUsage(),
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

function createNoSpeakerTagsSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 3,
        projectCount: 2,
        totalDurationSeconds: 300,
        transcriptCharacterCount: 1234,
        recordingCount: 2,
        batchCount: 1,
        inboxCount: 1,
        projectAssignedCount: 2,
        recentDailyItems: [],
        isDeepLoaded: true,
      },
      speakers: {
        annotatedItemCount: 0,
        speakerAttributedDuration: 0,
        identifiedSpeakerCount: 0,
        anonymousSpeakerSlotCount: 0,
        speakerTaggedSegmentCount: 0,
        totalSegmentCount: 6,
        totalSegmentDuration: 240,
        identifiedDuration: 0,
        anonymousDuration: 0,
        topIdentifiedSpeakers: [],
        isDeepLoaded: true,
      },
    },
    llmUsage: createUsage(),
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe('SettingsDashboardTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows a loading state before fast data resolves', () => {
    const fast = deferred<DashboardSnapshot>();
    vi.mocked(dashboardService.getFastSnapshot).mockReturnValue(fast.promise);
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshot());

    render(<SettingsDashboardTab />);

    expect(screen.getByTestId('dashboard-loading')).toBeDefined();
  });

  it('builds stable geometry for no-activity and flat non-zero trends', () => {
    const zeros = normalizeTrendPoints(Array.from({ length: 30 }, (_, index) => ({
      label: `d-${index}`,
      value: 0,
    })));
    const constant = normalizeTrendPoints(Array.from({ length: 30 }, (_, index) => ({
      label: `d-${index}`,
      value: 5,
    })));

    const zeroGeometry = buildTrendGeometry(zeros);
    const constantGeometry = buildTrendGeometry(constant);
    const zeroYValues = new Set(zeroGeometry.coordinates.map((point) => point.y));
    const constantYValues = new Set(constantGeometry.coordinates.map((point) => point.y));

    expect(zeroYValues.size).toBe(1);
    expect([...zeroYValues][0]).toBe(zeroGeometry.baseline);
    expect(constantYValues.size).toBe(1);
    expect([...constantYValues][0]).toBeLessThan(constantGeometry.baseline);
  });

  it('renders fast data first and keeps speaker stats in partial loading while deep scan runs', async () => {
    const deep = deferred<DashboardSnapshot>();
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockReturnValue(deep.promise);

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByTestId('dashboard-partial')).toBeDefined();
      expect(screen.getByTestId('dashboard-speaker-loading')).toBeDefined();
    });

    expect(screen.getByText('settings.dashboard.global_content')).toBeDefined();
    expect(screen.getByText('settings.dashboard.llm_usage')).toBeDefined();
  });

  it('renders plain line-chart anchors without trend summary copy', async () => {
    const trendValues = [
      ...Array.from({ length: 16 }, () => 0),
      ...Array.from({ length: 7 }, () => 1),
      ...Array.from({ length: 7 }, () => 4),
    ];

    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshotWithTrends(trendValues));

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeDefined();
    });

    expect(screen.getAllByTestId('dashboard-trend-anchor-start')).toHaveLength(3);
    expect(screen.getAllByTestId('dashboard-trend-anchor-end')).toHaveLength(3);
    expect(screen.queryByText('settings.dashboard.trend_recent_7_days')).toBeNull();
    expect(screen.queryByText('settings.dashboard.trend_change_up')).toBeNull();
  });

  it('does not render visible point markers for the token trend chart', async () => {
    const trendValues = [
      ...Array.from({ length: 20 }, () => 0),
      ...Array.from({ length: 10 }, () => 50),
    ];

    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshotWithTrends(trendValues));

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeDefined();
    });

    const tokenTrendTitle = screen.getAllByText('settings.dashboard.recent_token_trend')
      .find((element) => element.classList.contains('settings-dashboard-subtitle'));

    expect(tokenTrendTitle).toBeDefined();
    if (!tokenTrendTitle) {
      throw new Error('Token trend title not found');
    }

    const tokenTrendCard = tokenTrendTitle.closest('.settings-dashboard-chart-card');
    expect(tokenTrendCard).not.toBeNull();
    expect(tokenTrendCard?.querySelectorAll('circle').length).toBe(0);
  });

  it('renders plain line charts for all-zero recent trend values', async () => {
    const trendValues = Array.from({ length: 30 }, () => 0);

    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshotWithTrends(trendValues));

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeDefined();
    });

    expect(screen.getAllByTestId('dashboard-trend-anchor-start')).toHaveLength(3);
    expect(screen.queryByText('settings.dashboard.trend_no_activity')).toBeNull();
  });

  it('renders deep-loaded speaker details after the transcript scan finishes', async () => {
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshot());

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeDefined();
    });

    expect(screen.queryByTestId('dashboard-speaker-loading')).toBeNull();
    expect(screen.getByText('settings.dashboard.top_identified_speakers')).toBeDefined();
  });

  it('keeps top identified speakers in an empty state when only anonymous speakers exist', async () => {
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createAnonymousSpeakerSnapshot());

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByTestId('dashboard-top-speakers-empty')).toBeDefined();
    });

    expect(screen.getByText('settings.dashboard.identified_vs_anonymous')).toBeDefined();
  });

  it('renders a stable speaker overview when deep-loaded transcripts have no speaker tags', async () => {
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createNoSpeakerTagsSnapshot());

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByTestId('dashboard-speaker-overview-card')).toBeDefined();
    });

    expect(screen.getByText('settings.dashboard.coverage_and_attribution')).toBeDefined();
    expect(screen.getByTestId('dashboard-top-speakers-empty')).toBeDefined();
  });

  it('renders the dashboard empty state when there is no saved content or usage yet', async () => {
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue({
      content: {
        overview: {
          itemCount: 0,
          projectCount: 0,
          totalDurationSeconds: 0,
          transcriptCharacterCount: undefined,
          recordingCount: 0,
          batchCount: 0,
          inboxCount: 0,
          projectAssignedCount: 0,
          recentDailyItems: [],
          isDeepLoaded: false,
        },
        speakers: null,
      },
      llmUsage: createUsage(0),
      generatedAt: '2026-04-28T00:00:00.000Z',
    });

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByTestId('dashboard-empty')).toBeDefined();
    });
  });

  it('renders an error state when the fast load fails', async () => {
    vi.mocked(dashboardService.getFastSnapshot).mockRejectedValue(new Error('broken index'));
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshot());

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByTestId('dashboard-error')).toBeDefined();
    });

    expect(screen.getByText('broken index')).toBeDefined();
  });
});
