import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertCircle,
  BarChart3,
  Bot,
  Clock3,
  FileText,
  LoaderCircle,
} from 'lucide-react';
import type {
  DashboardContentTrendPoint,
  DashboardLlmUsageBreakdown,
  DashboardLlmUsageTrendPoint,
  DashboardSnapshot,
  DashboardSpeakerLeader,
  DashboardSpeakerStats,
} from '../../types/dashboard';
import { dashboardService } from '../../services/dashboardService';
import { normalizeError } from '../../utils/errorUtils';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import './SettingsDashboardTab.css';

function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(Math.round(value));
}

function formatDuration(seconds: number, t: (key: string, options?: Record<string, unknown>) => string): string {
  const totalMinutes = Math.max(0, Math.round(seconds / 60));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours > 0) {
    return t('settings.dashboard.duration_hours', {
      hours,
      minutes,
      defaultValue: `${hours}h ${minutes}m`,
    });
  }

  return t('settings.dashboard.duration_minutes', {
    minutes: totalMinutes,
    defaultValue: `${totalMinutes}m`,
  });
}

function formatPercent(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function formatDateLabel(dateKey: string): string {
  if (!dateKey) {
    return '';
  }

  const date = new Date(`${dateKey}T00:00:00`);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

function calculateCoverage(numerator: number, denominator: number): number {
  if (denominator <= 0) {
    return 0;
  }

  return numerator / denominator;
}

type TrendCardPoint = {
  label: string;
  value: number;
};

type TrendChartCoordinate = TrendCardPoint & {
  x: number;
  y: number;
};

export function normalizeTrendPoints(points: TrendCardPoint[], minimumPoints = 30): TrendCardPoint[] {
  const safePoints = points.map((point) => ({
    label: typeof point.label === 'string' ? point.label : '',
    value: Number.isFinite(point.value) ? Math.max(0, point.value) : 0,
  }));

  if (safePoints.length >= minimumPoints) {
    return safePoints.slice(-minimumPoints);
  }

  return [
    ...Array.from({ length: minimumPoints - safePoints.length }, () => ({ label: '', value: 0 })),
    ...safePoints,
  ];
}

export function buildTrendGeometry(
  points: TrendCardPoint[],
  chartWidth = 100,
  chartHeight = 72,
  topPadding = 8,
  bottomPadding = 12,
): {
  coordinates: TrendChartCoordinate[];
  linePath: string;
  baseline: number;
} {
  const normalizedPoints = normalizeTrendPoints(points);
  const values = normalizedPoints.map((point) => point.value);
  const maxValue = values.length > 0 ? Math.max(...values) : 0;
  const minValue = values.length > 0 ? Math.min(...values) : 0;
  const baseline = chartHeight - bottomPadding;
  const drawableHeight = baseline - topPadding;
  const range = maxValue - minValue;
  const flatY = values.every((value) => value === 0)
    ? baseline
    : topPadding + (drawableHeight / 2);
  const coordinates = normalizedPoints.map((point, index) => {
    const x = normalizedPoints.length === 1
      ? chartWidth / 2
      : (index / (normalizedPoints.length - 1)) * chartWidth;
    const y = range === 0
      ? flatY
      : baseline - (((point.value - minValue) / range) * drawableHeight);

    return {
      ...point,
      x,
      y: Number.isFinite(y) ? y : baseline,
    };
  });
  const linePath = coordinates
    .map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x} ${point.y}`)
    .join(' ');

  return {
    coordinates,
    linePath,
    baseline,
  };
}

function joinClassNames(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(' ');
}

function StatPill({ children }: { children: React.ReactNode }): React.JSX.Element {
  return <span className="settings-dashboard-stat-pill">{children}</span>;
}

function StatusBadge({
  icon,
  label,
  tone = 'neutral',
  testId,
}: {
  icon: React.ReactNode;
  label: string;
  tone?: 'neutral' | 'warning';
  testId?: string;
}): React.JSX.Element {
  return (
    <div
      className={joinClassNames(
        'settings-dashboard-status-badge',
        tone === 'warning' ? 'warning' : '',
      )}
      data-testid={testId}
    >
      {icon}
      <span>{label}</span>
    </div>
  );
}

function KpiCard({
  label,
  value,
  detail,
  badge,
  muted,
  variant = 'support',
  tone = 'default',
  compact = false,
}: {
  label: string;
  value: string;
  detail?: React.ReactNode;
  badge?: React.ReactNode;
  muted?: boolean;
  variant?: 'feature' | 'support';
  tone?: 'default' | 'accent' | 'info' | 'warm';
  compact?: boolean;
}): React.JSX.Element {
  return (
    <div
      className={joinClassNames(
        'settings-dashboard-kpi-card',
        variant,
        tone,
        compact ? 'compact' : '',
        muted ? 'muted' : '',
      )}
    >
      <div className="settings-dashboard-kpi-topline">
        {badge && <span className="settings-dashboard-kpi-badge">{badge}</span>}
        <div className="settings-dashboard-kpi-label">{label}</div>
      </div>
      <div className="settings-dashboard-kpi-value">{value}</div>
      {detail && <div className="settings-dashboard-kpi-detail">{detail}</div>}
    </div>
  );
}

function CoverageMeter({
  label,
  numerator,
  denominator,
  unitFormatter,
}: {
  label: string;
  numerator: number;
  denominator: number;
  unitFormatter: (value: number) => string;
}): React.JSX.Element {
  const coverage = calculateCoverage(numerator, denominator);

  return (
    <div className="settings-dashboard-coverage-block">
      <div className="settings-dashboard-subtitle">{label}</div>
      <div className="settings-dashboard-coverage-value">{formatPercent(coverage)}</div>
      <div className="settings-dashboard-note">
        {unitFormatter(numerator)} / {unitFormatter(denominator)}
      </div>
      <div className="settings-dashboard-progress-track" aria-hidden="true">
        <div
          className="settings-dashboard-progress-fill"
          style={{ width: `${Math.max(coverage * 100, coverage > 0 ? 4 : 0)}%` }}
        />
      </div>
    </div>
  );
}

function TrendCard({
  title,
  description,
  points,
  tone = 'accent',
}: {
  title: string;
  description: string;
  points: TrendCardPoint[];
  tone?: 'accent' | 'info';
}): React.JSX.Element {
  const normalizedPoints = normalizeTrendPoints(points);
  const {
    linePath,
    baseline,
  } = buildTrendGeometry(normalizedPoints);
  const startPoint = normalizedPoints[0];
  const endPoint = normalizedPoints[normalizedPoints.length - 1];

  return (
    <div className={joinClassNames('settings-dashboard-chart-card', 'settings-dashboard-trend-card', tone)}>
      <div className="settings-dashboard-chart-header">
        <div className="settings-dashboard-subtitle">{title}</div>
        <div className="settings-dashboard-note">{description}</div>
      </div>
      <div className="settings-dashboard-trend-surface">
        <svg
          className="settings-dashboard-trend-svg"
          viewBox="0 0 100 72"
          role="img"
          aria-label={title}
          preserveAspectRatio="none"
        >
          <title>{title}</title>
          <line x1="0" y1={baseline} x2="100" y2={baseline} className="settings-dashboard-trend-axis" />
          <path d={linePath} className="settings-dashboard-trend-line" />
        </svg>
        <div className="settings-dashboard-trend-anchors">
          <div className="settings-dashboard-trend-anchor" data-testid="dashboard-trend-anchor-start">
            <div className="settings-dashboard-trend-anchor-label">{startPoint?.label || '\u00A0'}</div>
          </div>
          <div className="settings-dashboard-trend-anchor end" data-testid="dashboard-trend-anchor-end">
            <div className="settings-dashboard-trend-anchor-label">{endPoint?.label || '\u00A0'}</div>
          </div>
        </div>
      </div>
    </div>
  );
}

function IdentifiedAnonymousDistribution({
  identifiedDuration,
  anonymousDuration,
  t,
}: {
  identifiedDuration: number;
  anonymousDuration: number;
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
  const total = identifiedDuration + anonymousDuration;
  const identifiedRatio = calculateCoverage(identifiedDuration, total);
  const anonymousRatio = calculateCoverage(anonymousDuration, total);

  return (
    <div className="settings-dashboard-distribution-panel">
      <div className="settings-dashboard-chart-header">
        <div className="settings-dashboard-subtitle">
          {t('settings.dashboard.identified_vs_anonymous', { defaultValue: 'Identified vs Anonymous' })}
        </div>
        <div className="settings-dashboard-note">
          {t('settings.dashboard.identified_vs_anonymous_hint', {
            defaultValue: 'Duration split across speaker-tagged segments only.',
          })}
        </div>
      </div>
      <div className="settings-dashboard-stacked-bar" aria-hidden="true">
        <div className="identified" style={{ width: `${Math.max(identifiedRatio * 100, identifiedDuration > 0 ? 4 : 0)}%` }} />
        <div className="anonymous" style={{ width: `${Math.max(anonymousRatio * 100, anonymousDuration > 0 ? 4 : 0)}%` }} />
      </div>
      <div className="settings-dashboard-bar-list compact">
        <div className="settings-dashboard-bar-item">
          <div className="settings-dashboard-bar-label-row">
            <span>{t('settings.dashboard.identified_duration', { defaultValue: 'Identified duration' })}</span>
            <span>{formatDuration(identifiedDuration, t)}</span>
          </div>
        </div>
        <div className="settings-dashboard-bar-item">
          <div className="settings-dashboard-bar-label-row">
            <span>{t('settings.dashboard.anonymous_duration', { defaultValue: 'Anonymous duration' })}</span>
            <span>{formatDuration(anonymousDuration, t)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function SpeakerOverviewCard({
  speakers,
  segmentCoverage,
  durationCoverage,
  t,
  statusMessage,
}: {
  speakers: DashboardSpeakerStats | null;
  segmentCoverage: number;
  durationCoverage: number;
  t: (key: string, options?: Record<string, unknown>) => string;
  statusMessage?: string | null;
}): React.JSX.Element {
  if (!speakers) {
    return (
      <div
        className={joinClassNames('settings-dashboard-chart-card', 'settings-dashboard-overview-card', 'muted')}
        data-testid="dashboard-speaker-overview-card"
      >
        <div className="settings-dashboard-chart-header">
          <div className="settings-dashboard-subtitle">
            {t('settings.dashboard.coverage_and_attribution', { defaultValue: 'Coverage & Attribution' })}
          </div>
          {statusMessage && <div className="settings-dashboard-note">{statusMessage}</div>}
        </div>
        <div className="settings-dashboard-placeholder-stack" aria-hidden="true">
          <div className="settings-dashboard-placeholder-line wide" />
          <div className="settings-dashboard-placeholder-line" />
          <div className="settings-dashboard-placeholder-line short" />
        </div>
      </div>
    );
  }

  return (
    <div className="settings-dashboard-chart-card settings-dashboard-overview-card" data-testid="dashboard-speaker-overview-card">
      <div className="settings-dashboard-chart-header">
        <div className="settings-dashboard-subtitle">
          {t('settings.dashboard.coverage_and_attribution', { defaultValue: 'Coverage & Attribution' })}
        </div>
        <div className="settings-dashboard-note">
          {t('settings.dashboard.coverage_summary', {
            defaultValue: '{{segmentCoverage}} segment coverage · {{durationCoverage}} duration coverage',
            segmentCoverage: formatPercent(segmentCoverage),
            durationCoverage: formatPercent(durationCoverage),
          })}
        </div>
      </div>
      <div className="settings-dashboard-coverage-grid">
        <CoverageMeter
          label={t('settings.dashboard.segment_coverage', { defaultValue: 'Speaker-Tagged Segments' })}
          numerator={speakers.speakerTaggedSegmentCount}
          denominator={speakers.totalSegmentCount}
          unitFormatter={(value) => formatNumber(value)}
        />
        <CoverageMeter
          label={t('settings.dashboard.duration_coverage', { defaultValue: 'Speaker-Tagged Duration' })}
          numerator={speakers.speakerAttributedDuration}
          denominator={speakers.totalSegmentDuration}
          unitFormatter={(value) => formatDuration(value, t)}
        />
      </div>
      <IdentifiedAnonymousDistribution
        identifiedDuration={speakers.identifiedDuration}
        anonymousDuration={speakers.anonymousDuration}
        t={t}
      />
    </div>
  );
}

function RankedSpeakers({
  speakers,
  t,
}: {
  speakers: DashboardSpeakerLeader[];
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
  if (speakers.length === 0) {
    return (
      <div className="settings-dashboard-empty-inline" data-testid="dashboard-top-speakers-empty">
        {t('settings.dashboard.top_speakers_empty', {
          defaultValue: 'No identified speakers yet. Anonymous coverage still appears above when available.',
        })}
      </div>
    );
  }

  const maxDuration = speakers.reduce((max, speaker) => Math.max(max, speaker.durationSeconds), 0);

  return (
    <div className="settings-dashboard-bar-list">
      {speakers.slice(0, 5).map((speaker) => {
        const width = maxDuration > 0 ? `${Math.max((speaker.durationSeconds / maxDuration) * 100, 10)}%` : '10%';
        return (
          <div key={speaker.speakerId} className="settings-dashboard-bar-item">
            <div className="settings-dashboard-bar-label-row">
              <div>
                <div className="settings-dashboard-bar-label">{speaker.label}</div>
                <div className="settings-dashboard-note">
                  {t('settings.dashboard.top_speakers_meta', {
                    defaultValue: '{{segments}} segments · {{items}} items',
                    segments: speaker.segmentCount,
                    items: speaker.itemCount,
                  })}
                </div>
              </div>
              <div className="settings-dashboard-bar-value">{formatDuration(speaker.durationSeconds, t)}</div>
            </div>
            <div className="settings-dashboard-progress-track" aria-hidden="true">
              <div className="settings-dashboard-progress-fill alt" style={{ width }} />
            </div>
          </div>
        );
      })}
    </div>
  );
}

function SpeakerRankingCard({
  speakers,
  t,
  statusMessage,
}: {
  speakers: DashboardSpeakerStats | null;
  t: (key: string, options?: Record<string, unknown>) => string;
  statusMessage?: string | null;
}): React.JSX.Element {
  return (
    <div className={joinClassNames('settings-dashboard-chart-card', 'settings-dashboard-ranking-card', !speakers ? 'muted' : '')}>
      <div className="settings-dashboard-chart-header">
        <div className="settings-dashboard-subtitle">
          {t('settings.dashboard.top_identified_speakers', { defaultValue: 'Top Identified Speakers' })}
        </div>
        <div className="settings-dashboard-note">
          {speakers
            ? t('settings.dashboard.top_identified_speakers_hint', {
              defaultValue: 'Ranked by speaker-attributed duration, with segment and item counts alongside.',
            })
            : statusMessage}
        </div>
      </div>
      {speakers ? (
        <RankedSpeakers speakers={speakers.topIdentifiedSpeakers} t={t} />
      ) : (
        <div className="settings-dashboard-placeholder-stack" aria-hidden="true">
          <div className="settings-dashboard-placeholder-line wide" />
          <div className="settings-dashboard-placeholder-line" />
          <div className="settings-dashboard-placeholder-line short" />
        </div>
      )}
    </div>
  );
}

function UsageBreakdown<TValue extends string>({
  title,
  breakdown,
  t,
}: {
  title: string;
  breakdown: DashboardLlmUsageBreakdown<TValue>[];
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
  if (breakdown.length === 0) {
    return (
      <div className="settings-dashboard-chart-card">
        <div className="settings-dashboard-subtitle">{title}</div>
        <div className="settings-dashboard-empty-inline">
          {t('settings.dashboard.no_tracked_calls', { defaultValue: 'No tracked calls yet.' })}
        </div>
      </div>
    );
  }

  const maxValue = breakdown.reduce((max, item) => Math.max(max, item.stats.totalTokens, item.stats.callCount), 0);

  return (
    <div className="settings-dashboard-chart-card">
      <div className="settings-dashboard-subtitle">{title}</div>
      <div className="settings-dashboard-bar-list">
        {breakdown.slice(0, 6).map((item) => {
          const value = Math.max(item.stats.totalTokens, item.stats.callCount);
          const width = maxValue > 0 ? `${Math.max((value / maxValue) * 100, 8)}%` : '8%';
          return (
            <div key={item.key} className="settings-dashboard-bar-item">
              <div className="settings-dashboard-bar-label-row">
                <div>
                  <div className="settings-dashboard-bar-label">{item.key}</div>
                  <div className="settings-dashboard-note">
                    {t('settings.dashboard.calls_and_tokens', {
                      defaultValue: '{{calls}} calls · {{tokens}} tokens',
                      calls: formatNumber(item.stats.callCount),
                      tokens: formatNumber(item.stats.totalTokens),
                    })}
                  </div>
                </div>
              </div>
              <div className="settings-dashboard-progress-track" aria-hidden="true">
                <div className="settings-dashboard-progress-fill alt" style={{ width }} />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function renderContentTrends(points: DashboardContentTrendPoint[], t: (key: string, options?: Record<string, unknown>) => string): React.JSX.Element {
  return (
    <div className="settings-dashboard-trend-grid">
      <TrendCard
        title={t('settings.dashboard.recent_item_trend', { defaultValue: 'Recent 30 Day Item Trend' })}
        description={t('settings.dashboard.recent_item_trend_hint', {
          defaultValue: 'Saved recordings and imports per day.',
        })}
        points={points.map((point) => ({
          label: formatDateLabel(point.date),
          value: point.itemCount,
        }))}
      />
      <TrendCard
        title={t('settings.dashboard.recent_duration_trend', { defaultValue: 'Recent 30 Day Duration Trend' })}
        description={t('settings.dashboard.recent_duration_trend_hint', {
          defaultValue: 'Total saved duration per day.',
        })}
        points={points.map((point) => ({
          label: formatDateLabel(point.date),
          value: point.durationSeconds,
        }))}
      />
    </div>
  );
}

function renderTokenTrend(points: DashboardLlmUsageTrendPoint[], t: (key: string, options?: Record<string, unknown>) => string): React.JSX.Element {
  return (
    <TrendCard
      title={t('settings.dashboard.recent_token_trend', { defaultValue: 'Recent 30 Day Token Trend' })}
      description={t('settings.dashboard.recent_token_trend_hint', {
        defaultValue: 'Prompt + completion tokens recorded each day.',
      })}
      points={points.map((point) => ({
        label: formatDateLabel(point.date),
        value: point.totalTokens,
      }))}
      tone="info"
    />
  );
}

export function SettingsDashboardTab(): React.JSX.Element {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [isFastLoading, setIsFastLoading] = useState(true);
  const [isDeepLoading, setIsDeepLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let cancelled = false;

    async function loadDashboard(): Promise<void> {
      setIsFastLoading(true);
      setIsDeepLoading(false);
      setError(null);
      setSnapshot(null);

      try {
        const fastSnapshot = await dashboardService.getFastSnapshot();
        if (cancelled) {
          return;
        }

        setSnapshot(fastSnapshot);
        setIsFastLoading(false);

        if (fastSnapshot.content.overview.itemCount === 0) {
          return;
        }

        setIsDeepLoading(true);

        try {
          const deepSnapshot = await dashboardService.getDeepSnapshot();
          if (!cancelled) {
            setSnapshot(deepSnapshot);
          }
        } catch (deepError) {
          if (!cancelled) {
            setError(normalizeError(deepError).message);
          }
        } finally {
          if (!cancelled) {
            setIsDeepLoading(false);
          }
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(normalizeError(loadError).message);
          setIsFastLoading(false);
        }
      }
    }

    void loadDashboard();

    return () => {
      cancelled = true;
    };
  }, [reloadToken]);

  if (isFastLoading && !snapshot) {
    return (
      <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
        <div className="settings-dashboard-state loading" data-testid="dashboard-loading">
          <LoaderCircle className="spin" size={18} />
          <span>{t('settings.dashboard.loading', { defaultValue: 'Loading dashboard...' })}</span>
        </div>
      </SettingsTabContainer>
    );
  }

  if (!snapshot) {
    return (
      <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
        <div className="settings-dashboard-state error" data-testid="dashboard-error">
          <AlertCircle size={18} />
          <div>
            <div className="settings-dashboard-state-title">{t('settings.dashboard.error_title', { defaultValue: 'Dashboard unavailable' })}</div>
            <div className="settings-dashboard-note">{error || t('settings.dashboard.error_body', { defaultValue: 'Try reloading dashboard data.' })}</div>
          </div>
          <button className="btn" onClick={() => setReloadToken((value) => value + 1)}>
            {t('settings.dashboard.retry', { defaultValue: 'Retry' })}
          </button>
        </div>
      </SettingsTabContainer>
    );
  }

  const { overview, speakers } = snapshot.content;
  const llmUsage = snapshot.llmUsage;
  const isEmpty = overview.itemCount === 0 && llmUsage.totals.callCount === 0;
  const segmentCoverage = speakers
    ? calculateCoverage(speakers.speakerTaggedSegmentCount, speakers.totalSegmentCount)
    : 0;
  const durationCoverage = speakers
    ? calculateCoverage(speakers.speakerAttributedDuration, speakers.totalSegmentDuration)
    : 0;

  if (isEmpty) {
    return (
      <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
        <SettingsPageHeader
          icon={<BarChart3 size={28} />}
          title={t('settings.dashboard.title', { defaultValue: 'Dashboard' })}
          description={t('settings.dashboard.description', {
            defaultValue: 'Review global content and LLM usage without leaving Settings.',
          })}
        />
        <div className="settings-dashboard-empty-page" data-testid="dashboard-empty">
          <div className="settings-dashboard-state-title">{t('settings.dashboard.empty_title', { defaultValue: 'No dashboard data yet' })}</div>
          <div className="settings-dashboard-note">
            {t('settings.dashboard.empty_body', {
              defaultValue: 'Saved transcripts and successful LLM calls will start appearing here automatically.',
            })}
          </div>
        </div>
      </SettingsTabContainer>
    );
  }

  return (
    <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
      <SettingsPageHeader
        icon={<BarChart3 size={28} />}
        title={t('settings.dashboard.title', { defaultValue: 'Dashboard' })}
        description={t('settings.dashboard.description', {
          defaultValue: 'Review global content and LLM usage without leaving Settings.',
        })}
      />

      <SettingsSection
        title={t('settings.dashboard.global_content', { defaultValue: 'Global Content' })}
        description={t('settings.dashboard.global_content_description', {
          defaultValue: 'Saved content volume, where it lives, and how much transcript data already has speaker attribution.',
        })}
        icon={<FileText size={20} />}
      >
        <div className="settings-dashboard-panel">
          <div className="settings-dashboard-subsection">
            <div className="settings-dashboard-subsection-header">
              <div className="settings-dashboard-subtitle-stack">
                <div className="settings-dashboard-subtitle">{t('settings.dashboard.content_overview', { defaultValue: 'Content Overview' })}</div>
              </div>
              {(isDeepLoading || error) && (
                <div className="settings-dashboard-status-rail">
                  {isDeepLoading && (
                    <StatusBadge
                      icon={<LoaderCircle className="spin" size={14} />}
                      label={t('settings.dashboard.partial_loading', { defaultValue: 'Loading transcript and speaker details...' })}
                      testId="dashboard-partial"
                    />
                  )}
                  {error && (
                    <StatusBadge
                      icon={<AlertCircle size={14} />}
                      label={t('settings.dashboard.partial_error', {
                        defaultValue: 'Some dashboard details could not be refreshed: {{message}}',
                        message: error,
                      })}
                      tone="warning"
                    />
                  )}
                </div>
              )}
            </div>

            <div className="settings-dashboard-feature-grid">
              <KpiCard
                label={t('settings.dashboard.items', { defaultValue: 'Items' })}
                value={formatNumber(overview.itemCount)}
                badge={<FileText size={16} />}
                variant="feature"
                tone="accent"
                detail={(
                  <div className="settings-dashboard-pill-row">
                    <StatPill>
                      {t('settings.dashboard.recording_pill', {
                        defaultValue: '{{count}} recording',
                        count: formatNumber(overview.recordingCount),
                      })}
                    </StatPill>
                    <StatPill>
                      {t('settings.dashboard.batch_pill', {
                        defaultValue: '{{count}} batch',
                        count: formatNumber(overview.batchCount),
                      })}
                    </StatPill>
                  </div>
                )}
              />
              <KpiCard
                label={t('settings.dashboard.total_duration', { defaultValue: 'Total Duration' })}
                value={formatDuration(overview.totalDurationSeconds, t)}
                badge={<Clock3 size={16} />}
                variant="feature"
                tone="warm"
              />
            </div>

            <div className="settings-dashboard-support-grid">
              <KpiCard
                label={t('settings.dashboard.transcript_characters', { defaultValue: 'Transcript Characters' })}
                value={typeof overview.transcriptCharacterCount === 'number'
                  ? formatNumber(overview.transcriptCharacterCount)
                  : t('settings.dashboard.scanning', { defaultValue: 'Scanning...' })}
                muted={typeof overview.transcriptCharacterCount !== 'number'}
                detail={typeof overview.transcriptCharacterCount !== 'number'
                  ? t('settings.dashboard.partial_loading', { defaultValue: 'Loading transcript and speaker details...' })
                  : undefined}
              />
              <KpiCard
                label={t('settings.dashboard.projects', { defaultValue: 'Projects' })}
                value={formatNumber(overview.projectCount)}
                detail={(
                  <div className="settings-dashboard-pill-row">
                    <StatPill>
                      {t('settings.dashboard.inbox_pill', {
                        defaultValue: '{{count}} in Inbox',
                        count: formatNumber(overview.inboxCount),
                      })}
                    </StatPill>
                    <StatPill>
                      {t('settings.dashboard.project_pill', {
                        defaultValue: '{{count}} in projects',
                        count: formatNumber(overview.projectAssignedCount),
                      })}
                    </StatPill>
                  </div>
                )}
              />
            </div>

            {renderContentTrends(overview.recentDailyItems, t)}
          </div>

          <div className="settings-dashboard-subsection">
            <div className="settings-dashboard-subsection-header">
              <div className="settings-dashboard-subtitle-stack">
                <div className="settings-dashboard-subtitle">{t('settings.dashboard.speaker_insights', { defaultValue: 'Speaker Insights' })}</div>
              </div>
              {isDeepLoading && (
                <div className="settings-dashboard-status-rail">
                  <StatusBadge
                    icon={<LoaderCircle className="spin" size={14} />}
                    label={t('settings.dashboard.scanning', { defaultValue: 'Scanning...' })}
                    testId="dashboard-speaker-loading"
                  />
                </div>
              )}
            </div>

            <div className="settings-dashboard-speaker-kpi-grid">
              <KpiCard
                label={t('settings.dashboard.annotated_items', { defaultValue: 'Speaker-Annotated Items' })}
                value={speakers ? formatNumber(speakers.annotatedItemCount) : '...'}
                muted={!speakers}
                compact
              />
              <KpiCard
                label={t('settings.dashboard.speaker_attributed_duration', { defaultValue: 'Speaker-Attributed Duration' })}
                value={speakers ? formatDuration(speakers.speakerAttributedDuration, t) : '...'}
                muted={!speakers}
                compact
              />
              <KpiCard
                label={t('settings.dashboard.identified_speakers', { defaultValue: 'Identified Speakers' })}
                value={speakers ? formatNumber(speakers.identifiedSpeakerCount) : '...'}
                muted={!speakers}
                compact
              />
              <KpiCard
                label={t('settings.dashboard.anonymous_slots', { defaultValue: 'Anonymous Speaker Slots' })}
                value={speakers ? formatNumber(speakers.anonymousSpeakerSlotCount) : '...'}
                muted={!speakers}
                compact
              />
            </div>

            <div className="settings-dashboard-speaker-detail-grid">
              <SpeakerOverviewCard
                speakers={speakers}
                segmentCoverage={segmentCoverage}
                durationCoverage={durationCoverage}
                t={t}
                statusMessage={isDeepLoading
                  ? t('settings.dashboard.deep_scan_loading', { defaultValue: 'Speaker stats are still scanning saved transcripts.' })
                  : error}
              />
              <SpeakerRankingCard
                speakers={speakers}
                t={t}
                statusMessage={isDeepLoading
                  ? t('settings.dashboard.deep_scan_loading', { defaultValue: 'Speaker stats are still scanning saved transcripts.' })
                  : error}
              />
            </div>
          </div>
        </div>
      </SettingsSection>

      <SettingsSection
        title={t('settings.dashboard.llm_usage', { defaultValue: 'LLM Usage' })}
        description={t('settings.dashboard.llm_usage_description', {
          defaultValue: 'All-time successful LLM calls tracked since this analytics file started, including connection tests.',
        })}
        icon={<Bot size={20} />}
      >
        <div className="settings-dashboard-panel">
          <div className="settings-dashboard-feature-grid">
            <KpiCard
              label={t('settings.dashboard.llm_call_count', { defaultValue: 'Successful Calls' })}
              value={formatNumber(llmUsage.totals.callCount)}
              badge={<Bot size={16} />}
              variant="feature"
              tone="info"
            />
            <KpiCard
              label={t('settings.dashboard.total_tokens', { defaultValue: 'Total Tokens' })}
              value={formatNumber(llmUsage.totals.totalTokens)}
              badge={<BarChart3 size={16} />}
              variant="feature"
              tone="accent"
            />
          </div>

          <div className="settings-dashboard-support-grid">
            <KpiCard
              label={t('settings.dashboard.calls_with_usage', { defaultValue: 'Calls With Usage' })}
              value={formatNumber(llmUsage.totals.callsWithUsage)}
            />
            <KpiCard
              label={t('settings.dashboard.calls_without_usage', { defaultValue: 'Calls Missing Usage' })}
              value={formatNumber(llmUsage.totals.callsWithoutUsage)}
              tone={llmUsage.totals.callsWithoutUsage > 0 ? 'warm' : 'default'}
            />
          </div>

          <div className="settings-dashboard-meta-rail">
            <div className="settings-dashboard-meta-pill">
              <BarChart3 size={14} />
              <span>
                {t('settings.dashboard.tokens_hint', {
                  defaultValue: '{{prompt}} prompt / {{completion}} completion',
                  prompt: formatNumber(llmUsage.totals.promptTokens),
                  completion: formatNumber(llmUsage.totals.completionTokens),
                })}
              </span>
            </div>
            <div className="settings-dashboard-meta-pill">
              <Clock3 size={14} />
              <span>
                {llmUsage.startedAt
                  ? t('settings.dashboard.tracking_since', {
                    defaultValue: 'Tracking since {{date}}',
                    date: new Date(llmUsage.startedAt).toLocaleString(),
                  })
                  : t('settings.dashboard.no_tracked_calls', { defaultValue: 'No tracked calls yet.' })}
              </span>
            </div>
            {llmUsage.totals.callsWithoutUsage > 0 && (
              <div className="settings-dashboard-meta-pill warning">
                <AlertCircle size={14} />
                <span>
                  {t('settings.dashboard.missing_usage_hint', {
                    defaultValue: '{{count}} successful calls did not include token usage from the provider.',
                    count: formatNumber(llmUsage.totals.callsWithoutUsage),
                  })}
                </span>
              </div>
            )}
          </div>

          <div className="settings-dashboard-usage-grid">
            {renderTokenTrend(llmUsage.recentDaily, t)}
            <UsageBreakdown
              title={t('settings.dashboard.by_provider', { defaultValue: 'By Provider' })}
              breakdown={llmUsage.byProvider}
              t={t}
            />
            <UsageBreakdown
              title={t('settings.dashboard.by_category', { defaultValue: 'By Category' })}
              breakdown={llmUsage.byCategory}
              t={t}
            />
          </div>
        </div>
      </SettingsSection>
    </SettingsTabContainer>
  );
}
