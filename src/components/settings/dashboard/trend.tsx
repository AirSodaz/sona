import React from 'react';
import { formatDateLabel } from './formatters';
import { joinClassNames } from './classNames';
import {
  buildTrendGeometry,
  normalizeTrendPoints,
  type TrendCardPoint,
} from './trendGeometry';
import type {
  DashboardContentTrendPoint,
  DashboardLlmUsageTrendPoint,
} from '../../../types/dashboard';

export function TrendCard({
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

export function ContentTrends({
  points,
  t,
}: {
  points: DashboardContentTrendPoint[];
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
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

export function TokenTrend({
  points,
  t,
}: {
  points: DashboardLlmUsageTrendPoint[];
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
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
