export type TrendCardPoint = {
  label: string;
  value: number;
};

export type TrendChartCoordinate = TrendCardPoint & {
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
