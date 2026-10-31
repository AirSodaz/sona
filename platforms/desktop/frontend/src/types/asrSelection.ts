/**
 * Leaf ASR selection DTOs shared by config and media-server tool overrides.
 * Kept separate so `config` ↔ `mediaServer` do not form an import cycle.
 */

export type AsrEngine = 'local' | 'online';
export type AsrMode = 'streaming' | 'batch';

export interface AsrModelSelection {
  engine: AsrEngine;
  mode: AsrMode;
  modelId?: string | null;
  modelPath: string;
  providerId?: string | null;
  profileId?: string | null;
}
