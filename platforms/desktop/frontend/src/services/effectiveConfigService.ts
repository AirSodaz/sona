import type { AppConfig } from '../types/config';
import type { ProjectRecord } from '../types/project';
import { resolveEffectiveConfig as resolveEffectiveConfigInRust } from './tauri/app';

export function resolveEffectiveConfig(
  globalConfig: AppConfig,
  project: ProjectRecord | null,
): Promise<AppConfig> {
  return resolveEffectiveConfigInRust(globalConfig, project);
}
