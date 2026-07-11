export type DashboardTranslation = (key: string, options?: Record<string, unknown>) => string;

export function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(Math.round(value));
}

export function formatDuration(seconds: number, t: DashboardTranslation): string {
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

export function formatPercent(value: number): string {
  return `${Math.round(value * 100)}%`;
}

export function formatDateLabel(dateKey: string): string {
  if (!dateKey) {
    return '';
  }

  const date = new Date(`${dateKey}T00:00:00`);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function calculateCoverage(numerator: number, denominator: number): number {
  if (denominator <= 0) {
    return 0;
  }

  return numerator / denominator;
}
