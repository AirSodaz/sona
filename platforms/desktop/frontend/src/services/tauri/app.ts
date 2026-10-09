import type { AppConfig, AppLogLevel, ResolvedAppTheme } from '../../types/config';
import { flattenAppConfig } from '../../types/llm';
import type {
  ModelCatalogSelectedIds as UiModelCatalogSelectedIds,
  ModelCatalogSnapshot as UiModelCatalogSnapshot,
} from '../../types/modelCatalog';
import type { ProjectRecord } from '../../types/project';
import type {
  AsrRuntimeMetricsSnapshot,
  RuntimeEnvironmentStatus,
  RuntimePathStatus,
} from '../../types/runtime';
import {
  normalizeModelCatalogSelectedIds,
  normalizeModelCatalogSnapshot,
} from '../modelCatalogNormalizers';
import { TauriCommand } from './commands';
import type { TauriCommandArgs, TauriCommandResult } from './contracts';
import { invokeTauri } from './invoke';

export type DownloadFileRequest = TauriCommandArgs<typeof TauriCommand.app.downloadFile>;

export type DownloadPresetModelRequest = TauriCommandArgs<
  typeof TauriCommand.app.downloadPresetModel
>;

export type ExtractTarBz2Request = TauriCommandArgs<typeof TauriCommand.app.extractTarBz2>;

export type UpdateTrayMenuRequest = TauriCommandArgs<typeof TauriCommand.app.updateTrayMenu>;

export type ModelSelectionPaths = TauriCommandArgs<
  typeof TauriCommand.app.resolveModelCatalogSelectedIds
>['paths'];

export type ModelCatalogSelectedIds = UiModelCatalogSelectedIds;

export type AppConfigMigrationResult = TauriCommandResult<typeof TauriCommand.app.migrateAppConfig>;

export { normalizeModelCatalogSelectedIds, normalizeModelCatalogSnapshot };

export async function extractTarBz2(request: ExtractTarBz2Request): Promise<void> {
  await invokeTauri(TauriCommand.app.extractTarBz2, request);
}

export async function downloadFile(request: DownloadFileRequest): Promise<void> {
  await invokeTauri(TauriCommand.app.downloadFile, request);
}

export async function downloadPresetModel(request: DownloadPresetModelRequest): Promise<string> {
  return await invokeTauri(TauriCommand.app.downloadPresetModel, request);
}

export async function deletePresetModel(modelId: string): Promise<void> {
  await invokeTauri(TauriCommand.app.deletePresetModel, { modelId });
}

export async function cancelDownload(id: string): Promise<void> {
  await invokeTauri(TauriCommand.app.cancelDownload, { id });
}

export async function openLogFolder(): Promise<void> {
  await invokeTauri(TauriCommand.app.openLogFolder);
}

export async function getModelCatalogSnapshot(): Promise<UiModelCatalogSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.app.getModelCatalogSnapshot);
  return normalizeModelCatalogSnapshot(snapshot);
}

export async function resolveModelCatalogSelectedIds(
  paths: ModelSelectionPaths
): Promise<ModelCatalogSelectedIds> {
  const selectedIds = await invokeTauri(TauriCommand.app.resolveModelCatalogSelectedIds, { paths });
  return normalizeModelCatalogSelectedIds(selectedIds);
}

export { getDiagnosticsCoreSnapshot } from '../diagnosticsOperations';

export async function loadAppConfig(): Promise<AppConfig | null> {
  const config = await invokeTauri(TauriCommand.app.loadAppConfig);
  return config ? flattenAppConfig(config) : null;
}

export async function saveAppConfig(config: AppConfig): Promise<void> {
  await invokeTauri(TauriCommand.app.saveAppConfig, { config });
}

export async function getAppSetting<T = unknown>(key: string): Promise<T | null> {
  return invokeTauri(TauriCommand.app.getAppSetting, { key }) as Promise<T | null>;
}

export async function setAppSetting(key: string, value: unknown): Promise<void> {
  await invokeTauri(TauriCommand.app.setAppSetting, { key, value });
}

export async function migrateAppConfig(
  savedConfig: AppConfig | null | undefined,
  defaultRuleSetName: string
): Promise<AppConfigMigrationResult> {
  const res = await invokeTauri(TauriCommand.app.migrateAppConfig, {
    savedConfig: savedConfig ?? null,
    defaultRuleSetName,
  });
  return {
    ...res,
    config: flattenAppConfig(res.config),
  };
}

export async function resolveEffectiveConfig(
  globalConfig: AppConfig,
  project: ProjectRecord | null
): Promise<AppConfig> {
  const res = await invokeTauri(TauriCommand.app.resolveEffectiveConfig, {
    globalConfig,
    project,
  });
  return flattenAppConfig(res);
}

export async function getRuntimeEnvironmentStatus(): Promise<RuntimeEnvironmentStatus> {
  return invokeTauri(TauriCommand.app.getRuntimeEnvironmentStatus);
}

export async function getAsrRuntimeMetrics(): Promise<AsrRuntimeMetricsSnapshot> {
  return invokeTauri(TauriCommand.app.getAsrRuntimeMetrics);
}

export async function getPathStatuses(paths: string[]): Promise<RuntimePathStatus[]> {
  return invokeTauri(TauriCommand.app.getPathStatuses, { paths });
}

export async function hasActiveDownloads(): Promise<boolean> {
  return invokeTauri(TauriCommand.app.hasActiveDownloads);
}
export async function hasActiveApiServerJobs(): Promise<boolean> {
  try {
    const res = await invokeTauri(TauriCommand.apiServer.hasActiveJobs);
    return res?.hasActive ?? false;
  } catch {
    return false;
  }
}

export async function forceExit(): Promise<void> {
  await invokeTauri(TauriCommand.app.forceExit);
}

export async function updateTrayMenu(request: UpdateTrayMenuRequest): Promise<void> {
  await invokeTauri(TauriCommand.app.updateTrayMenu, request);
}

export async function setMinimizeToTray(enabled: boolean): Promise<void> {
  await invokeTauri(TauriCommand.app.setMinimizeToTray, { enabled });
}

export async function setAutoStart(enabled: boolean): Promise<void> {
  await invokeTauri(TauriCommand.app.setAutoStart, { enabled });
}

export async function isAutoStartEnabled(): Promise<boolean> {
  return await invokeTauri(TauriCommand.app.isAutoStartEnabled);
}

export async function setLogLevel(level: AppLogLevel): Promise<void> {
  await invokeTauri(TauriCommand.app.setLogLevel, { level });
}

/**
 * Recolors the native window frame to match the resolved application theme.
 *
 * The frame is drawn by the OS, so it cannot read our CSS custom properties;
 * this keeps it in sync with `--color-bg-secondary` as the theme changes.
 */
export async function setWindowTheme(theme: ResolvedAppTheme): Promise<void> {
  await invokeTauri(TauriCommand.app.setWindowTheme, { theme });
}
