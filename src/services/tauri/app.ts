import type {
  AsrRuntimeMetricsSnapshot,
  RuntimeEnvironmentStatus,
  RuntimePathStatus,
} from '../../types/runtime';
import type { AppConfig, AppLogLevel } from '../../types/config';
import type { ProjectRecord } from '../../types/project';
import type { ModelCatalogSnapshot } from '../modelService';
import type {
  DiagnosticsCoreInput,
  DiagnosticsCoreSnapshotSpec,
} from '../diagnosticsSnapshotBuilders';
import { TauriCommand } from './commands';
import type { TauriCommandArgs, TauriCommandResult } from './contracts';
import { invokeTauri } from './invoke';

export type DownloadFileRequest = TauriCommandArgs<typeof TauriCommand.app.downloadFile>;

export type ExtractTarBz2Request = TauriCommandArgs<typeof TauriCommand.app.extractTarBz2>;

export type UpdateTrayMenuRequest = TauriCommandArgs<typeof TauriCommand.app.updateTrayMenu>;

export type ModelSelectionPaths = TauriCommandArgs<
  typeof TauriCommand.app.resolveModelCatalogSelectedIds
>['paths'];

export type ModelCatalogSelectedIds = TauriCommandResult<
  typeof TauriCommand.app.resolveModelCatalogSelectedIds
>;

export type AppConfigMigrationResult = TauriCommandResult<
  typeof TauriCommand.app.migrateAppConfig
>;

export async function extractTarBz2(request: ExtractTarBz2Request): Promise<void> {
  await invokeTauri(TauriCommand.app.extractTarBz2, request);
}

export async function downloadFile(request: DownloadFileRequest): Promise<void> {
  await invokeTauri(TauriCommand.app.downloadFile, request);
}

export async function cancelDownload(id: string): Promise<void> {
  await invokeTauri(TauriCommand.app.cancelDownload, { id });
}

export async function openLogFolder(): Promise<void> {
  await invokeTauri(TauriCommand.app.openLogFolder);
}

export async function getModelCatalogSnapshot(): Promise<ModelCatalogSnapshot> {
  return invokeTauri(TauriCommand.app.getModelCatalogSnapshot);
}

export async function resolveModelCatalogSelectedIds(
  paths: ModelSelectionPaths,
): Promise<ModelCatalogSelectedIds> {
  return invokeTauri(TauriCommand.app.resolveModelCatalogSelectedIds, { paths });
}

export async function getDiagnosticsCoreSnapshot(
  input: DiagnosticsCoreInput,
): Promise<DiagnosticsCoreSnapshotSpec> {
  return invokeTauri(TauriCommand.app.getDiagnosticsCoreSnapshot, { input });
}

export async function migrateAppConfig(
  savedConfig: AppConfig | null | undefined,
  legacyConfig: unknown,
  defaultRuleSetName: string,
): Promise<AppConfigMigrationResult> {
  return invokeTauri(TauriCommand.app.migrateAppConfig, {
    savedConfig: savedConfig ?? null,
    legacyConfig: legacyConfig ?? null,
    defaultRuleSetName,
  });
}

export async function resolveEffectiveConfig(
  globalConfig: AppConfig,
  project: ProjectRecord | null,
): Promise<AppConfig> {
  return invokeTauri(TauriCommand.app.resolveEffectiveConfig, {
    globalConfig,
    project,
  });
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

export async function forceExit(): Promise<void> {
  await invokeTauri(TauriCommand.app.forceExit);
}

export async function updateTrayMenu(request: UpdateTrayMenuRequest): Promise<void> {
  await invokeTauri(TauriCommand.app.updateTrayMenu, request);
}

export async function setMinimizeToTray(enabled: boolean): Promise<void> {
  await invokeTauri(TauriCommand.app.setMinimizeToTray, { enabled });
}

export async function setLogLevel(level: AppLogLevel): Promise<void> {
  await invokeTauri(TauriCommand.app.setLogLevel, { level });
}
