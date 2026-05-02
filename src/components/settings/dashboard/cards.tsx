import React from 'react';
import {
  calculateCoverage,
  formatDuration,
  formatNumber,
  formatPercent,
  type DashboardTranslation,
} from './formatters';
import { joinClassNames } from './classNames';
import {
  CoverageBarChart,
  DashboardSparkline,
  MiniValueBarChart,
  StackedDurationBarChart,
  type DashboardChartPoint,
  type DashboardChartTone,
} from './charts';
import type {
  DashboardLlmUsageBreakdown,
  DashboardSpeakerLeader,
  DashboardSpeakerStats,
} from '../../../types/dashboard';

export function StatPill({ children }: { children: React.ReactNode }): React.JSX.Element {
  return <span className="settings-dashboard-stat-pill">{children}</span>;
}

export function StatusBadge({
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

export function KpiCard({
  label,
  value,
  detail,
  badge,
  muted,
  variant = 'support',
  tone = 'default',
  compact = false,
  sparkline,
  sparklineLabel,
}: {
  label: string;
  value: string;
  detail?: React.ReactNode;
  badge?: React.ReactNode;
  muted?: boolean;
  variant?: 'feature' | 'support';
  tone?: 'default' | 'accent' | 'info' | 'warm';
  compact?: boolean;
  sparkline?: DashboardChartPoint[];
  sparklineLabel?: string;
}): React.JSX.Element {
  const chartTone: DashboardChartTone = tone === 'info' || tone === 'warm' ? tone : 'accent';

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
      {sparkline && sparkline.length > 0 && (
        <DashboardSparkline
          label={sparklineLabel || label}
          points={sparkline}
          tone={chartTone}
        />
      )}
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
      <CoverageBarChart
        label={label}
        value={coverage}
        valueFormatter={formatPercent}
      />
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
  t: DashboardTranslation;
}): React.JSX.Element {
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
      <div className="settings-dashboard-split-chart">
        <StackedDurationBarChart
          label={t('settings.dashboard.identified_vs_anonymous', { defaultValue: 'Identified vs Anonymous' })}
          identifiedLabel={t('settings.dashboard.identified_duration', { defaultValue: 'Identified duration' })}
          anonymousLabel={t('settings.dashboard.anonymous_duration', { defaultValue: 'Anonymous duration' })}
          identifiedDuration={identifiedDuration}
          anonymousDuration={anonymousDuration}
          valueFormatter={(value) => formatDuration(value, t)}
        />
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

export function SpeakerOverviewCard({
  speakers,
  segmentCoverage,
  durationCoverage,
  t,
  statusMessage,
}: {
  speakers: DashboardSpeakerStats | null;
  segmentCoverage: number;
  durationCoverage: number;
  t: DashboardTranslation;
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
  t: DashboardTranslation;
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
    <div className="settings-dashboard-bar-list" data-testid="dashboard-recharts-ranking">
      {speakers.slice(0, 5).map((speaker) => {
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
            <MiniValueBarChart
              label={speaker.label}
              value={speaker.durationSeconds}
              maxValue={maxDuration}
              valueFormatter={(value) => formatDuration(value, t)}
            />
          </div>
        );
      })}
    </div>
  );
}

export function SpeakerRankingCard({
  speakers,
  t,
  statusMessage,
}: {
  speakers: DashboardSpeakerStats | null;
  t: DashboardTranslation;
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

export function UsageBreakdown<TValue extends string>({
  title,
  breakdown,
  t,
}: {
  title: string;
  breakdown: DashboardLlmUsageBreakdown<TValue>[];
  t: DashboardTranslation;
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
      <div className="settings-dashboard-bar-list" data-testid="dashboard-recharts-breakdown">
        {breakdown.slice(0, 6).map((item) => {
          const value = Math.max(item.stats.totalTokens, item.stats.callCount);
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
              <MiniValueBarChart
                label={item.key}
                value={value}
                maxValue={maxValue}
                valueFormatter={formatNumber}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
