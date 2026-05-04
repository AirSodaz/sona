import React from 'react';
import { joinClassNames } from './classNames';
import { DashboardTrendChart, type DashboardChartPoint } from './charts';
import type {
  DashboardContentTrendPoint,
  DashboardLlmUsageTrendPoint,
} from '../../../types/dashboard';

export function TrendCard({
  title,
  description,
  points,
  valueFormatter,
  tone = 'accent',
}: {
  title: string;
  description: string;
  points: DashboardChartPoint[];
  valueFormatter?: (value: number) => string;
  tone?: 'accent' | 'info';
}): React.JSX.Element {
  return (
    <div className={joinClassNames('settings-dashboard-chart-card', 'settings-dashboard-trend-card', tone)}>
      <div className="settings-dashboard-chart-header">
        <div className="settings-dashboard-subtitle">{title}</div>
        <div className="settings-dashboard-note">{description}</div>
      </div>
      <DashboardTrendChart
        label={title}
        points={points}
        tone={tone}
        valueFormatter={valueFormatter}
      />
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
          label: point.dateLabel,
          value: point.itemCount,
        }))}
        valueFormatter={(value) => (
          points.find((point) => point.itemCount === value)?.itemCountDisplay || String(value)
        )}
      />
      <TrendCard
        title={t('settings.dashboard.recent_duration_trend', { defaultValue: 'Recent 30 Day Duration Trend' })}
        description={t('settings.dashboard.recent_duration_trend_hint', {
          defaultValue: 'Total saved duration per day.',
        })}
        points={points.map((point) => ({
          label: point.dateLabel,
          value: point.durationSeconds,
        }))}
        valueFormatter={(value) => (
          points.find((point) => point.durationSeconds === value)?.durationDisplay || String(value)
        )}
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
        label: point.dateLabel,
        value: point.totalTokens,
      }))}
      valueFormatter={(value) => (
        points.find((point) => point.totalTokens === value)?.totalTokensDisplay || String(value)
      )}
      tone="info"
    />
  );
}
