import React from 'react';
import {
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
  coverage,
  coverageLabel,
  unitFormatter,
}: {
  label: string;
  numerator: number;
  denominator: number;
  coverage: number;
  coverageLabel: string;
  unitFormatter: (value: number) => string;
}): React.JSX.Element {
  return (
    <div className="settings-dashboard-coverage-block">
      <div className="settings-dashboard-subtitle">{label}</div>
      <div className="settings-dashboard-coverage-value">{coverageLabel}</div>
      <div className="settings-dashboard-note">
        {unitFormatter(numerator)} / {unitFormatter(denominator)}
      </div>
      <CoverageBarChart
        label={label}
        value={coverage}
        valueFormatter={() => coverageLabel}
      />
    </div>
  );
}

function IdentifiedAnonymousDistribution({
  identifiedDuration,
  anonymousDuration,
  identifiedDurationDisplay,
  anonymousDurationDisplay,
  t,
}: {
  identifiedDuration: number;
  anonymousDuration: number;
  identifiedDurationDisplay: string;
  anonymousDurationDisplay: string;
  t: DashboardTranslation;
}): React.JSX.Element {
  const formatSplitDuration = (value: number): string => (
    value === identifiedDuration ? identifiedDurationDisplay : anonymousDurationDisplay
  );

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
          valueFormatter={formatSplitDuration}
        />
      </div>
      <div className="settings-dashboard-bar-list compact">
        <div className="settings-dashboard-bar-item">
          <div className="settings-dashboard-bar-label-row">
            <span>{t('settings.dashboard.identified_duration', { defaultValue: 'Identified duration' })}</span>
            <span>{identifiedDurationDisplay}</span>
          </div>
        </div>
        <div className="settings-dashboard-bar-item">
          <div className="settings-dashboard-bar-label-row">
            <span>{t('settings.dashboard.anonymous_duration', { defaultValue: 'Anonymous duration' })}</span>
            <span>{anonymousDurationDisplay}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

export function SpeakerOverviewCard({
  speakers,
  t,
  statusMessage,
}: {
  speakers: DashboardSpeakerStats | null;
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
            segmentCoverage: speakers.segmentCoverageLabel,
            durationCoverage: speakers.durationCoverageLabel,
          })}
        </div>
      </div>
      <div className="settings-dashboard-coverage-grid">
        <CoverageMeter
          label={t('settings.dashboard.segment_coverage', { defaultValue: 'Speaker-Tagged Segments' })}
          numerator={speakers.speakerTaggedSegmentCount}
          denominator={speakers.totalSegmentCount}
          coverage={speakers.segmentCoverageRatio}
          coverageLabel={speakers.segmentCoverageLabel}
          unitFormatter={(value) => value === speakers.speakerTaggedSegmentCount
            ? speakers.speakerTaggedSegmentCountDisplay
            : speakers.totalSegmentCountDisplay}
        />
        <CoverageMeter
          label={t('settings.dashboard.duration_coverage', { defaultValue: 'Speaker-Tagged Duration' })}
          numerator={speakers.speakerAttributedDuration}
          denominator={speakers.totalSegmentDuration}
          coverage={speakers.durationCoverageRatio}
          coverageLabel={speakers.durationCoverageLabel}
          unitFormatter={(value) => value === speakers.speakerAttributedDuration
            ? speakers.speakerAttributedDurationDisplay
            : speakers.totalSegmentDurationDisplay}
        />
      </div>
      <IdentifiedAnonymousDistribution
        identifiedDuration={speakers.identifiedDuration}
        anonymousDuration={speakers.anonymousDuration}
        identifiedDurationDisplay={speakers.identifiedDurationDisplay}
        anonymousDurationDisplay={speakers.anonymousDurationDisplay}
        t={t}
      />
    </div>
  );
}

function RankedSpeakers({
  speakers,
  maxValue,
  t,
}: {
  speakers: DashboardSpeakerLeader[];
  maxValue: number;
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

  return (
    <div className="settings-dashboard-bar-list" data-testid="dashboard-recharts-ranking">
      {speakers.map((speaker) => {
        return (
          <div key={speaker.speakerId} className="settings-dashboard-bar-item">
            <div className="settings-dashboard-bar-label-row">
              <div>
                <div className="settings-dashboard-bar-label">{speaker.label}</div>
                <div className="settings-dashboard-note">
                  {t('settings.dashboard.top_speakers_meta', {
                    defaultValue: '{{segments}} segments · {{items}} items',
                    segments: speaker.segmentCountDisplay,
                    items: speaker.itemCountDisplay,
                  })}
                </div>
              </div>
              <div className="settings-dashboard-bar-value">{speaker.durationDisplay}</div>
            </div>
            <MiniValueBarChart
              label={speaker.label}
              value={speaker.durationSeconds}
              maxValue={maxValue}
              valueFormatter={() => speaker.durationDisplay}
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
        <RankedSpeakers
          speakers={speakers.topIdentifiedSpeakerRows}
          maxValue={speakers.topIdentifiedSpeakerMaxValue}
          t={t}
        />
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
  maxValue,
  t,
}: {
  title: string;
  breakdown: DashboardLlmUsageBreakdown<TValue>[];
  maxValue: number;
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

  return (
    <div className="settings-dashboard-chart-card">
      <div className="settings-dashboard-subtitle">{title}</div>
      <div className="settings-dashboard-bar-list" data-testid="dashboard-recharts-breakdown">
        {breakdown.map((item) => {
          return (
            <div key={item.key} className="settings-dashboard-bar-item">
              <div className="settings-dashboard-bar-label-row">
                <div>
                  <div className="settings-dashboard-bar-label">{item.label}</div>
                  <div className="settings-dashboard-note">
                    {t('settings.dashboard.calls_and_tokens', {
                      defaultValue: '{{calls}} calls · {{tokens}} tokens',
                      calls: item.stats.callCountDisplay,
                      tokens: item.stats.totalTokensDisplay,
                    })}
                  </div>
                </div>
              </div>
              <MiniValueBarChart
                label={item.label}
                value={item.value}
                maxValue={maxValue}
                valueFormatter={() => item.valueDisplay}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
