import React, { useId } from 'react';
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import type { TooltipContentProps, TooltipPayloadEntry } from 'recharts';

export type DashboardChartTone = 'accent' | 'info' | 'warm';

export type DashboardChartPoint = {
  label: string;
  value: number;
};

type DashboardTooltipValue = number | string | ReadonlyArray<number | string>;
type DashboardTooltipPayload = ReadonlyArray<TooltipPayloadEntry<DashboardTooltipValue, string | number>>;

interface DashboardTooltipProps {
  active?: boolean;
  label?: string | number;
  payload?: DashboardTooltipPayload;
  valueFormatter: (value: number) => string;
}

const DEFAULT_VALUE_FORMATTER = (value: number) => `${Math.round(value)}`;

function safeChartValue(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

function normalizeChartPoints(points: DashboardChartPoint[], minimumPoints = 30): DashboardChartPoint[] {
  const safePoints = points.map((point) => ({
    label: typeof point.label === 'string' ? point.label : '',
    value: safeChartValue(point.value),
  }));

  if (safePoints.length >= minimumPoints) {
    return safePoints.slice(-minimumPoints);
  }

  return [
    ...Array.from({ length: minimumPoints - safePoints.length }, () => ({ label: '', value: 0 })),
    ...safePoints,
  ];
}

function gradientIdFromReactId(id: string): string {
  return `settings-dashboard-${id.replace(/[^a-zA-Z0-9_-]/g, '')}`;
}

function DashboardTooltip({
  active,
  label,
  payload = [],
  valueFormatter,
}: DashboardTooltipProps): React.JSX.Element | null {
  if (!active || payload.length === 0) {
    return null;
  }

  return (
    <div className="settings-dashboard-chart-tooltip">
      {label && <div className="settings-dashboard-chart-tooltip-label">{label}</div>}
      {payload.map((entry, index) => {
        const rawValue = Array.isArray(entry.value) ? entry.value[0] : entry.value;
        const numericValue = typeof rawValue === 'number' ? rawValue : Number(rawValue);
        const value = Number.isFinite(numericValue)
          ? valueFormatter(numericValue)
          : String(rawValue ?? '');

        return (
          <div key={`${entry.name ?? 'value'}-${index}`} className="settings-dashboard-chart-tooltip-row">
            <span
              className="settings-dashboard-chart-tooltip-swatch"
              style={{ background: entry.color || 'var(--dashboard-accent)' }}
            />
            <span>{entry.name || 'Value'}</span>
            <strong>{value}</strong>
          </div>
        );
      })}
    </div>
  );
}

function renderTooltip(valueFormatter: (value: number) => string) {
  return (
    props: TooltipContentProps<DashboardTooltipValue, string | number>,
  ): React.JSX.Element | null => (
    <DashboardTooltip
      active={props.active}
      label={props.label}
      payload={props.payload as unknown as DashboardTooltipPayload}
      valueFormatter={valueFormatter}
    />
  );
}

export function DashboardSparkline({
  label,
  points,
  tone = 'accent',
}: {
  label: string;
  points: DashboardChartPoint[];
  tone?: DashboardChartTone;
}): React.JSX.Element | null {
  const gradientId = gradientIdFromReactId(useId());

  if (points.length === 0) {
    return null;
  }

  const chartPoints = normalizeChartPoints(points);

  return (
    <div
      className={`settings-dashboard-kpi-sparkline ${tone}`}
      data-testid="dashboard-kpi-sparkline"
      role="img"
      aria-label={label}
    >
      <ResponsiveContainer width="100%" height={36}>
        <AreaChart data={chartPoints} margin={{ top: 6, right: 0, bottom: 2, left: 0 }}>
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--dashboard-accent)" stopOpacity={0.32} />
              <stop offset="100%" stopColor="var(--dashboard-accent)" stopOpacity={0.04} />
            </linearGradient>
          </defs>
          <XAxis dataKey="label" hide />
          <YAxis hide domain={['dataMin', 'dataMax']} />
          <Tooltip
            cursor={false}
            wrapperStyle={{ outline: 'none' }}
            content={renderTooltip(DEFAULT_VALUE_FORMATTER)}
          />
          <Area
            type="monotone"
            dataKey="value"
            name={label}
            stroke="var(--dashboard-accent)"
            strokeWidth={2}
            fill={`url(#${gradientId})`}
            dot={false}
            activeDot={{ r: 3 }}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

export function DashboardTrendChart({
  label,
  points,
  tone = 'accent',
  valueFormatter = DEFAULT_VALUE_FORMATTER,
}: {
  label: string;
  points: DashboardChartPoint[];
  tone?: DashboardChartTone;
  valueFormatter?: (value: number) => string;
}): React.JSX.Element {
  const gradientId = gradientIdFromReactId(useId());
  const chartPoints = normalizeChartPoints(points);
  const startPoint = chartPoints[0];
  const endPoint = chartPoints[chartPoints.length - 1];

  return (
    <div className={`settings-dashboard-trend-surface ${tone}`}>
      <div
        className="settings-dashboard-recharts-surface trend"
        data-testid="dashboard-recharts-trend"
        role="img"
        aria-label={label}
      >
        <ResponsiveContainer width="100%" height={108}>
          <AreaChart data={chartPoints} margin={{ top: 12, right: 8, bottom: 8, left: 8 }}>
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="var(--dashboard-accent)" stopOpacity={0.28} />
                <stop offset="100%" stopColor="var(--dashboard-accent)" stopOpacity={0.02} />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} stroke="var(--dashboard-chart-grid)" strokeDasharray="3 5" />
            <XAxis dataKey="label" hide />
            <YAxis hide width={0} domain={['dataMin', 'dataMax']} />
            <Tooltip
              cursor={{ stroke: 'var(--dashboard-accent)', strokeOpacity: 0.28, strokeWidth: 1 }}
              wrapperStyle={{ outline: 'none' }}
              content={renderTooltip(valueFormatter)}
            />
            <Area
              type="monotone"
              dataKey="value"
              name={label}
              stroke="var(--dashboard-accent)"
              strokeWidth={2.35}
              fill={`url(#${gradientId})`}
              dot={false}
              activeDot={{ r: 4, strokeWidth: 2 }}
              isAnimationActive={false}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
      <div className="settings-dashboard-trend-anchors">
        <div className="settings-dashboard-trend-anchor" data-testid="dashboard-trend-anchor-start">
          <div className="settings-dashboard-trend-anchor-label">{startPoint?.label || '\u00A0'}</div>
        </div>
        <div className="settings-dashboard-trend-anchor end" data-testid="dashboard-trend-anchor-end">
          <div className="settings-dashboard-trend-anchor-label">{endPoint?.label || '\u00A0'}</div>
        </div>
      </div>
    </div>
  );
}

export function CoverageBarChart({
  label,
  value,
  valueFormatter,
}: {
  label: string;
  value: number;
  valueFormatter: (value: number) => string;
}): React.JSX.Element {
  const percentValue = Math.min(100, Math.max(0, value * 100));
  const data = [{ label, value: percentValue }];

  return (
    <div
      className="settings-dashboard-recharts-surface mini-bar"
      data-testid="dashboard-recharts-coverage"
      role="img"
      aria-label={label}
    >
      <ResponsiveContainer width="100%" height={28}>
        <BarChart layout="vertical" data={data} margin={{ top: 6, right: 0, bottom: 6, left: 0 }}>
          <XAxis type="number" hide domain={[0, 100]} />
          <YAxis type="category" dataKey="label" hide />
          <Tooltip
            cursor={false}
            wrapperStyle={{ outline: 'none' }}
            content={renderTooltip(valueFormatter)}
          />
          <Bar
            dataKey="value"
            name={label}
            fill="var(--dashboard-accent)"
            radius={[0, 999, 999, 0]}
            background={{ fill: 'var(--dashboard-chart-track)', radius: 999 }}
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

export function StackedDurationBarChart({
  label,
  identifiedLabel,
  anonymousLabel,
  identifiedDuration,
  anonymousDuration,
  valueFormatter,
}: {
  label: string;
  identifiedLabel: string;
  anonymousLabel: string;
  identifiedDuration: number;
  anonymousDuration: number;
  valueFormatter: (value: number) => string;
}): React.JSX.Element {
  const data = [{
    label,
    identified: safeChartValue(identifiedDuration),
    anonymous: safeChartValue(anonymousDuration),
  }];

  return (
    <div
      className="settings-dashboard-recharts-surface split-bar"
      data-testid="dashboard-recharts-speaker-split"
      role="img"
      aria-label={label}
    >
      <ResponsiveContainer width="100%" height={34}>
        <BarChart layout="vertical" data={data} margin={{ top: 7, right: 0, bottom: 7, left: 0 }}>
          <XAxis type="number" hide />
          <YAxis type="category" dataKey="label" hide />
          <Tooltip
            cursor={false}
            wrapperStyle={{ outline: 'none' }}
            content={renderTooltip(valueFormatter)}
          />
          <Bar
            dataKey="identified"
            name={identifiedLabel}
            stackId="duration"
            fill="#2d7ff9"
            radius={[999, 0, 0, 999]}
            isAnimationActive={false}
          />
          <Bar
            dataKey="anonymous"
            name={anonymousLabel}
            stackId="duration"
            fill="#f59e0b"
            radius={[0, 999, 999, 0]}
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

export function MiniValueBarChart({
  label,
  value,
  maxValue,
  valueFormatter,
  testId,
}: {
  label: string;
  value: number;
  maxValue: number;
  valueFormatter: (value: number) => string;
  testId?: string;
}): React.JSX.Element {
  const safeMaxValue = Math.max(1, safeChartValue(maxValue));
  const data = [{ label, value: safeChartValue(value) }];

  return (
    <div
      className="settings-dashboard-recharts-surface mini-bar"
      data-testid={testId}
      role="img"
      aria-label={label}
    >
      <ResponsiveContainer width="100%" height={26}>
        <BarChart layout="vertical" data={data} margin={{ top: 5, right: 0, bottom: 5, left: 0 }}>
          <XAxis type="number" hide domain={[0, safeMaxValue]} />
          <YAxis type="category" dataKey="label" hide />
          <Tooltip
            cursor={false}
            wrapperStyle={{ outline: 'none' }}
            content={renderTooltip(valueFormatter)}
          />
          <Bar
            dataKey="value"
            name={label}
            fill="var(--dashboard-accent)"
            radius={[0, 999, 999, 0]}
            background={{ fill: 'var(--dashboard-chart-track)', radius: 999 }}
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
