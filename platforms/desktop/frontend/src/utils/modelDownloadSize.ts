import type { ModelInfo } from '../types/modelCatalog';

/** Confirm downloads at or above this size (~300 MB). */
export const LARGE_DOWNLOAD_BYTES = 300 * 1024 * 1024;

/** Parse human size strings like "1.2 GB" / "350 MB" into bytes. */
export function parseModelSizeToBytes(size: string | undefined): number | null {
  if (!size?.trim()) {
    return null;
  }
  const match = size.trim().match(/^([\d.]+)\s*(B|KB|MB|GB|TB)$/i);
  if (!match) {
    return null;
  }
  const value = Number(match[1]);
  if (!Number.isFinite(value)) {
    return null;
  }
  const unit = match[2].toUpperCase();
  const multipliers: Record<string, number> = {
    B: 1,
    KB: 1024,
    MB: 1024 ** 2,
    GB: 1024 ** 3,
    TB: 1024 ** 4,
  };
  return Math.round(value * (multipliers[unit] ?? 1));
}

export function resolveModelDownloadBytes(model: Pick<ModelInfo, 'size' | 'artifacts'>): number | null {
  const artifactBytes = model.artifacts?.reduce(
    (sum, artifact) => sum + (artifact.sizeBytes ?? 0),
    0
  );
  if (artifactBytes && artifactBytes > 0) {
    return artifactBytes;
  }
  return parseModelSizeToBytes(model.size);
}
