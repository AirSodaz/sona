import type { SyncPresetV1 } from '../../../types/sync';

const PRESET_RANK: Record<SyncPresetV1, number> = {
  content: 0,
  standard: 1,
  full: 2,
};

export function isPresetShrink(current: SyncPresetV1, next: SyncPresetV1): boolean {
  return PRESET_RANK[next] < PRESET_RANK[current];
}
