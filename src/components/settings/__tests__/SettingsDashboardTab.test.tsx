import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  SettingsDashboardTab,
} from '../SettingsDashboardTab';
import type {
  DashboardLlmUsageStats,
  DashboardSnapshot,
  DashboardSpeakerStats,
} from '../../../types/dashboard';
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

function displayNumber(value: number): string {
  return String(Math.round(value));
}

function displayDuration(seconds: number): string {
  const totalMinutes = Math.max(0, Math.round(seconds / 60));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function displayPercent(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function createUsageBucket({
  callCount,
  callsWithUsage,
  callsWithoutUsage,
  promptTokens,
  completionTokens,
  totalTokens,
}: {
  callCount: number;
  callsWithUsage: number;
  callsWithoutUsage: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}) {
  return {
    callCount,
    callCountDisplay: displayNumber(callCount),
    callsWithUsage,
    callsWithUsageDisplay: displayNumber(callsWithUsage),
    callsWithoutUsage,
    callsWithoutUsageDisplay: displayNumber(callsWithoutUsage),
    promptTokens,
    promptTokensDisplay: displayNumber(promptTokens),
    completionTokens,
    completionTokensDisplay: displayNumber(completionTokens),
    totalTokens,
    totalTokensDisplay: displayNumber(totalTokens),
  };
}

function createContentTrend(values: number[]) {
  return values.map((value, index) => ({
    date: createTrendDateKey(index),
    dateLabel: `Mar ${index + 1}`,
    itemCount: value,
    itemCountDisplay: displayNumber(value),
    durationSeconds: value * 60,
    durationDisplay: displayDuration(value * 60),
  }));
}

function createUsageTrend(values: number[]) {
  return values.map((value, index) => ({
    date: createTrendDateKey(index),
    dateLabel: `Mar ${index + 1}`,
    ...createUsageBucket({
      callCount: value > 0 ? 1 : 0,
      callsWithUsage: value > 0 ? 1 : 0,
      callsWithoutUsage: 0,
      promptTokens: value,
      completionTokens: 0,
      totalTokens: value,
    }),
  }));
}

function createOverview(
  overrides: Partial<DashboardSnapshot['content']['overview']> = {},
): DashboardSnapshot['content']['overview'] {
  const itemCount = overrides.itemCount ?? 3;
  const projectCount = overrides.projectCount ?? 2;
  const totalDurationSeconds = overrides.totalDurationSeconds ?? 300;
  const transcriptCharacterCount = overrides.transcriptCharacterCount;
  const recordingCount = overrides.recordingCount ?? 2;
  const batchCount = overrides.batchCount ?? 1;
  const inboxCount = overrides.inboxCount ?? 1;
  const projectAssignedCount = overrides.projectAssignedCount ?? 2;

  return {
    itemCount,
    itemCountDisplay: overrides.itemCountDisplay ?? displayNumber(itemCount),
    projectCount,
    projectCountDisplay: overrides.projectCountDisplay ?? displayNumber(projectCount),
    totalDurationSeconds,
    totalDurationDisplay: overrides.totalDurationDisplay ?? displayDuration(totalDurationSeconds),
    transcriptCharacterCount,
    transcriptCharacterCountDisplay: overrides.transcriptCharacterCountDisplay
      ?? (typeof transcriptCharacterCount === 'number' ? displayNumber(transcriptCharacterCount) : undefined),
    recordingCount,
    recordingCountDisplay: overrides.recordingCountDisplay ?? displayNumber(recordingCount),
    batchCount,
    batchCountDisplay: overrides.batchCountDisplay ?? displayNumber(batchCount),
    inboxCount,
    inboxCountDisplay: overrides.inboxCountDisplay ?? displayNumber(inboxCount),
    projectAssignedCount,
    projectAssignedCountDisplay: overrides.projectAssignedCountDisplay ?? displayNumber(projectAssignedCount),
    recentDailyItems: overrides.recentDailyItems ?? [],
    isDeepLoaded: overrides.isDeepLoaded ?? false,
  };
}

function createSpeakerStats(
  overrides: Partial<DashboardSpeakerStats> = {},
): DashboardSpeakerStats {
  const speakerTaggedSegmentCount = overrides.speakerTaggedSegmentCount ?? 5;
  const totalSegmentCount = overrides.totalSegmentCount ?? 6;
  const speakerAttributedDuration = overrides.speakerAttributedDuration ?? 180;
  const totalSegmentDuration = overrides.totalSegmentDuration ?? 240;
  const identifiedDuration = overrides.identifiedDuration ?? 120;
  const anonymousDuration = overrides.anonymousDuration ?? 60;
  const topIdentifiedSpeakers = overrides.topIdentifiedSpeakers ?? [
    {
      speakerId: 'speaker-alice',
      label: 'Alice',
      durationSeconds: 120,
      durationDisplay: displayDuration(120),
      segmentCount: 3,
      segmentCountDisplay: '3',
      itemCount: 2,
      itemCountDisplay: '2',
    },
  ];
  const topIdentifiedSpeakerRows = overrides.topIdentifiedSpeakerRows
    ?? topIdentifiedSpeakers.slice(0, 5);

  return {
    annotatedItemCount: overrides.annotatedItemCount ?? 2,
    annotatedItemCountDisplay: overrides.annotatedItemCountDisplay
      ?? displayNumber(overrides.annotatedItemCount ?? 2),
    speakerAttributedDuration,
    speakerAttributedDurationDisplay: overrides.speakerAttributedDurationDisplay
      ?? displayDuration(speakerAttributedDuration),
    identifiedSpeakerCount: overrides.identifiedSpeakerCount ?? 1,
    identifiedSpeakerCountDisplay: overrides.identifiedSpeakerCountDisplay
      ?? displayNumber(overrides.identifiedSpeakerCount ?? 1),
    anonymousSpeakerSlotCount: overrides.anonymousSpeakerSlotCount ?? 2,
    anonymousSpeakerSlotCountDisplay: overrides.anonymousSpeakerSlotCountDisplay
      ?? displayNumber(overrides.anonymousSpeakerSlotCount ?? 2),
    speakerTaggedSegmentCount,
    speakerTaggedSegmentCountDisplay: overrides.speakerTaggedSegmentCountDisplay
      ?? displayNumber(speakerTaggedSegmentCount),
    totalSegmentCount,
    totalSegmentCountDisplay: overrides.totalSegmentCountDisplay ?? displayNumber(totalSegmentCount),
    totalSegmentDuration,
    totalSegmentDurationDisplay: overrides.totalSegmentDurationDisplay
      ?? displayDuration(totalSegmentDuration),
    identifiedDuration,
    identifiedDurationDisplay: overrides.identifiedDurationDisplay
      ?? displayDuration(identifiedDuration),
    anonymousDuration,
    anonymousDurationDisplay: overrides.anonymousDurationDisplay
      ?? displayDuration(anonymousDuration),
    segmentCoverageRatio: overrides.segmentCoverageRatio
      ?? (totalSegmentCount > 0 ? speakerTaggedSegmentCount / totalSegmentCount : 0),
    segmentCoverageLabel: overrides.segmentCoverageLabel
      ?? displayPercent(totalSegmentCount > 0 ? speakerTaggedSegmentCount / totalSegmentCount : 0),
    durationCoverageRatio: overrides.durationCoverageRatio
      ?? (totalSegmentDuration > 0 ? speakerAttributedDuration / totalSegmentDuration : 0),
    durationCoverageLabel: overrides.durationCoverageLabel
      ?? displayPercent(totalSegmentDuration > 0 ? speakerAttributedDuration / totalSegmentDuration : 0),
    topIdentifiedSpeakers,
    topIdentifiedSpeakerRows,
    topIdentifiedSpeakerMaxValue: overrides.topIdentifiedSpeakerMaxValue
      ?? Math.max(0, ...topIdentifiedSpeakerRows.map((speaker) => speaker.durationSeconds)),
    isDeepLoaded: overrides.isDeepLoaded ?? true,
  };
}

function createBreakdown<TValue extends string>(
  key: TValue,
  bucket: Parameters<typeof createUsageBucket>[0],
) {
  const stats = createUsageBucket(bucket);
  const value = Math.max(stats.totalTokens, stats.callCount);
  return {
    key,
    label: key,
    stats,
    value,
    valueDisplay: displayNumber(value),
  };
}

function createUsage(callCount = 4, recentValues: number[] = []) {
  return {
    startedAt: '2026-04-01T00:00:00.000Z',
    lastUpdatedAt: '2026-04-28T00:00:00.000Z',
    trackingSinceDisplay: 'Apr 1, 2026',
    lastUpdatedDisplay: 'Apr 28, 2026',
    totals: createUsageBucket({
      callCount,
      callsWithUsage: callCount,
      callsWithoutUsage: 0,
      promptTokens: 1200,
      completionTokens: 600,
      totalTokens: 1800,
    }),
    byProvider: [],
    byProviderTopRows: [],
    byProviderMaxValue: 0,
    byCategory: [],
    byCategoryTopRows: [],
    byCategoryMaxValue: 0,
    recentDaily: createUsageTrend(recentValues),
  };
}

function createUsageWithBreakdowns(recentValues: number[] = []): DashboardLlmUsageStats {
  const byProvider = [
    createBreakdown('open_ai', {
      callCount: 4,
      callsWithUsage: 4,
      callsWithoutUsage: 0,
      promptTokens: 900,
      completionTokens: 300,
      totalTokens: 1200,
    }),
    createBreakdown('ollama', {
      callCount: 2,
      callsWithUsage: 2,
      callsWithoutUsage: 0,
      promptTokens: 300,
      completionTokens: 100,
      totalTokens: 400,
    }),
  ];
  const byCategory = [
    createBreakdown('summary', {
      callCount: 3,
      callsWithUsage: 3,
      callsWithoutUsage: 0,
      promptTokens: 500,
      completionTokens: 200,
      totalTokens: 700,
    }),
    createBreakdown('translation', {
      callCount: 3,
      callsWithUsage: 3,
      callsWithoutUsage: 0,
      promptTokens: 700,
      completionTokens: 200,
      totalTokens: 900,
    }),
  ];

  return {
    ...createUsage(6, recentValues),
    byProvider,
    byProviderTopRows: byProvider,
    byProviderMaxValue: 1200,
    byCategory,
    byCategoryTopRows: byCategory,
    byCategoryMaxValue: 900,
  };
}

function createFastSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        ...createOverview({ isDeepLoaded: false, transcriptCharacterCount: undefined }),
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
        ...createOverview({ isDeepLoaded: true, transcriptCharacterCount: 1234 }),
      },
      speakers: createSpeakerStats(),
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

function createDeepSnapshotWithRechartsData(values: number[]): DashboardSnapshot {
  const snapshot = createDeepSnapshotWithTrends(values);

  return {
    ...snapshot,
    llmUsage: createUsageWithBreakdowns(values.map((value) => value * 100)),
  };
}

function createAnonymousSpeakerSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        ...createOverview({ isDeepLoaded: true, transcriptCharacterCount: 1234 }),
      },
      speakers: createSpeakerStats({
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
        topIdentifiedSpeakerRows: [],
        topIdentifiedSpeakerMaxValue: 0,
      }),
    },
    llmUsage: createUsage(),
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

function createNoSpeakerTagsSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        ...createOverview({ isDeepLoaded: true, transcriptCharacterCount: 1234 }),
      },
      speakers: createSpeakerStats({
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
        topIdentifiedSpeakerRows: [],
        topIdentifiedSpeakerMaxValue: 0,
      }),
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

  it('does not load dashboard data while mounted inactive for tab prewarm', async () => {
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshot());

    render(<SettingsDashboardTab isActive={false} />);

    await Promise.resolve();

    expect(dashboardService.getFastSnapshot).not.toHaveBeenCalled();
    expect(dashboardService.getDeepSnapshot).not.toHaveBeenCalled();
  });

  it('shows a loading state before fast data resolves', () => {
    const fast = deferred<DashboardSnapshot>();
    vi.mocked(dashboardService.getFastSnapshot).mockReturnValue(fast.promise);
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshot());

    render(<SettingsDashboardTab />);

    expect(screen.getByTestId('dashboard-loading')).toBeDefined();
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

  it('renders Recharts trend surfaces and KPI sparklines without trend summary copy', async () => {
    const trendValues = [
      ...Array.from({ length: 16 }, () => 0),
      ...Array.from({ length: 7 }, () => 1),
      ...Array.from({ length: 7 }, () => 4),
    ];

    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshotWithRechartsData(trendValues));

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeDefined();
    });

    expect(screen.getAllByTestId('dashboard-trend-anchor-start')).toHaveLength(3);
    expect(screen.getAllByTestId('dashboard-trend-anchor-end')).toHaveLength(3);
    expect(screen.getAllByTestId('dashboard-recharts-trend')).toHaveLength(3);
    expect(screen.getAllByTestId('dashboard-kpi-sparkline')).toHaveLength(4);
    expect(screen.queryByText('settings.dashboard.trend_recent_7_days')).toBeNull();
    expect(screen.queryByText('settings.dashboard.trend_change_up')).toBeNull();
  });

  it('renders the token trend as a Recharts surface with accessible chart labeling', async () => {
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
    expect(tokenTrendCard?.querySelector('[data-testid="dashboard-recharts-trend"]')).not.toBeNull();
    expect(tokenTrendCard?.querySelector('[aria-label="settings.dashboard.recent_token_trend"]')).not.toBeNull();
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
    expect(screen.getAllByTestId('dashboard-recharts-trend')).toHaveLength(3);
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
    expect(screen.getAllByTestId('dashboard-recharts-coverage')).toHaveLength(2);
    expect(screen.getByTestId('dashboard-recharts-speaker-split')).toBeDefined();
    expect(screen.getByTestId('dashboard-recharts-ranking')).toBeDefined();
  });

  it('renders Recharts provider and category breakdown charts for tracked LLM usage', async () => {
    const trendValues = [
      ...Array.from({ length: 20 }, () => 0),
      ...Array.from({ length: 10 }, () => 10),
    ];

    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createFastSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createDeepSnapshotWithRechartsData(trendValues));

    render(<SettingsDashboardTab />);

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeDefined();
    });

    expect(screen.getAllByTestId('dashboard-recharts-breakdown')).toHaveLength(2);
    expect(screen.getByText('open_ai')).toBeDefined();
    expect(screen.getByText('summary')).toBeDefined();
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
          ...createOverview({
            itemCount: 0,
            projectCount: 0,
            totalDurationSeconds: 0,
            transcriptCharacterCount: undefined,
            recordingCount: 0,
            batchCount: 0,
            inboxCount: 0,
            projectAssignedCount: 0,
            isDeepLoaded: false,
          }),
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
