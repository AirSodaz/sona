import i18n from '../i18n';
import type { AppConfig } from '../types/config';
import { migrateAppConfig } from './tauri/app';

export interface MigrationResult {
  config: AppConfig;
  migrated: boolean;
}

/**
 * Delegates settings-shape migration to Rust while keeping storage ownership in
 * the existing frontend startup flow.
 */
export async function migrateConfig(
  savedConfig: AppConfig | null | undefined,
): Promise<MigrationResult> {
  return migrateAppConfig(
    savedConfig ?? null,
    i18n.t('settings.default_rule_set_name', { defaultValue: 'Default Rules' }),
  );
}
