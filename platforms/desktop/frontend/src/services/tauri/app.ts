import type {
  AsrRuntimeMetricsSnapshot,
  RuntimeEnvironmentStatus,
  RuntimePathStatus,
} from '../../types/runtime';
import type { AppConfig, AppLogLevel } from '../../types/config';
import type { ProjectRecord } from '../../types/project';
import type {
  ModelCatalogModel,
  ModelCatalogRestoreDefaults,
  ModelCatalogSectionType,
  ModelCatalogSelectedIds as UiModelCatalogSelectedIds,
  ModelCatalogSnapshot as UiModelCatalogSnapshot,
  ModelInfo,
  ModelRules,
  TimestampSupportHint,
} from '../modelService';
import type {
  DiagnosticsCoreInput as CoreDiagnosticsInput,
  DiagnosticsCoreSnapshot as CoreDiagnosticsSnapshot,
  ModelCatalogModel as CoreModelCatalogModel,
  ModelCatalogRestoreDefaults as CoreModelCatalogRestoreDefaults,
  ModelCatalogSelectedIds as CoreModelCatalogSelectedIds,
  ModelCatalogSnapshot as CoreModelCatalogSnapshot,
} from '../../bindings';
import type {
  DiagnosticsCoreInput,
  DiagnosticsCoreFactsSnapshot,
} from '../diagnosticsSnapshotBuilders';
import { TauriCommand } from './commands';
import type { TauriCommandArgs, TauriCommandResult } from './contracts';
import { invokeTauri } from './invoke';
import { flattenAppConfig } from '../../types/llm';

export type DownloadFileRequest = TauriCommandArgs<typeof TauriCommand.app.downloadFile>;

export type ExtractTarBz2Request = TauriCommandArgs<typeof TauriCommand.app.extractTarBz2>;

export type UpdateTrayMenuRequest = TauriCommandArgs<typeof TauriCommand.app.updateTrayMenu>;

export type ModelSelectionPaths = TauriCommandArgs<
  typeof TauriCommand.app.resolveModelCatalogSelectedIds
>['paths'];

export type ModelCatalogSelectedIds = UiModelCatalogSelectedIds;

export type AppConfigMigrationResult = TauriCommandResult<
  typeof TauriCommand.app.migrateAppConfig
>;

function buildDiagnosticsTransportInput(input: DiagnosticsCoreInput): CoreDiagnosticsInput {
  const normalizeProbe = (probe: DiagnosticsCoreInput['microphoneProbe']) => ({
    options: probe.options.map(({ label, value }) => ({ label, value })),
    available: probe.available,
    errorMessage: probe.errorMessage ?? null,
  });

  return {
    config: input.config,
    permissionState: input.permissionState,
    microphoneProbe: normalizeProbe(input.microphoneProbe),
    systemAudioProbe: normalizeProbe(input.systemAudioProbe),
    voiceTypingReadiness: {
      state: input.voiceTypingReadiness.state,
      lastErrorMessage: input.voiceTypingReadiness.lastErrorMessage,
    },
  };
}

function normalizePermissionState(
  value: string,
): DiagnosticsCoreFactsSnapshot['permissionState'] {
  switch (value) {
    case 'denied':
    case 'granted':
    case 'prompt':
    case 'unsupported':
      return value;
    default:
      throw new Error(`Unexpected diagnostics permission state: ${value}`);
  }
}

function normalizeVoiceTypingState(
  value: string,
): DiagnosticsCoreFactsSnapshot['voiceTypingReadiness']['state'] {
  switch (value) {
    case 'off':
    case 'needs_shortcut':
    case 'needs_live_model':
    case 'needs_vad':
    case 'failed':
    case 'preparing':
    case 'ready':
      return value;
    default:
      throw new Error(`Unexpected diagnostics voice typing state: ${value}`);
  }
}

function normalizeDiagnosticsSnapshot(
  snapshot: CoreDiagnosticsSnapshot,
): DiagnosticsCoreFactsSnapshot {
  return {
    ...snapshot,
    config: {
      streamingModelPath: snapshot.config.streamingModelPath,
      batchModelPath: snapshot.config.batchModelPath,
      vadModelPath: snapshot.config.vadModelPath ?? '',
      punctuationModelPath: snapshot.config.punctuationModelPath ?? '',
      microphoneId: snapshot.config.microphoneId ?? 'default',
    },
    permissionState: normalizePermissionState(snapshot.permissionState),
    voiceTypingReadiness: {
      state: normalizeVoiceTypingState(snapshot.voiceTypingReadiness.state),
      lastErrorMessage: snapshot.voiceTypingReadiness.lastErrorMessage,
    },
  };
}

function optionalString(value: string | null | undefined): string | undefined {
  return value ?? undefined;
}

function requireFiniteNumber(value: number | null, fieldName: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`Expected a finite number for model catalog ${fieldName}`);
  }
  return value;
}

function normalizeModelType(value: string): ModelInfo['type'] {
  switch (value) {
    case 'zipformer':
    case 'sensevoice':
    case 'paraformer':
    case 'punctuation':
    case 'vad':
    case 'itn':
    case 'whisper':
    case 'funasr-nano':
    case 'fire-red-asr':
    case 'dolphin':
    case 'qwen3-asr':
    case 'speaker-segmentation':
    case 'speaker-embedding':
      return value;
    default:
      throw new Error(`Unexpected model catalog type: ${value}`);
  }
}

function normalizeModelModes(
  modes: string[] | null | undefined,
): ModelInfo['modes'] {
  if (!modes) {
    return undefined;
  }

  return modes.map((mode) => {
    switch (mode) {
      case 'streaming':
      case 'batch':
        return mode;
      default:
        throw new Error(`Unexpected model catalog mode: ${mode}`);
    }
  });
}

function normalizeTimestampSupportHint(
  value: string | null | undefined,
): TimestampSupportHint | undefined {
  switch (value) {
    case 'token':
    case 'segment':
    case 'unknown':
      return value;
    case null:
    case undefined:
      return undefined;
    default:
      throw new Error(`Unexpected model timestamp support hint: ${value}`);
  }
}

function normalizeCatalogSectionType(value: string): ModelCatalogSectionType {
  switch (value) {
    case 'asr':
    case 'punctuation':
    case 'vad':
    case 'speaker-segmentation':
    case 'speaker-embedding':
      return value;
    default:
      throw new Error(`Unexpected model catalog section type: ${value}`);
  }
}

function normalizeModelRules(modelRules: CoreModelCatalogModel['rules']): ModelRules {
  const timestampSupportHint = normalizeTimestampSupportHint(modelRules.timestampSupportHint);

  return {
    requiresVad: modelRules.requiresVad,
    requiresPunctuation: modelRules.requiresPunctuation,
    ...(timestampSupportHint === undefined ? {} : { timestampSupportHint }),
  };
}

function normalizeCatalogModel(model: CoreModelCatalogModel): ModelCatalogModel {
  const modes = normalizeModelModes(model.modes);
  const sha256 = optionalString(model.sha256);
  const isRecommended = model.isRecommended ?? undefined;
  const filename = optionalString(model.filename);
  const groupId = optionalString(model.groupId);
  const versionLabel = optionalString(model.versionLabel);

  if (model.engine !== 'sherpa-onnx') {
    throw new Error(`Unexpected model catalog engine: ${model.engine}`);
  }

  return {
    id: model.id,
    name: model.name,
    description: model.description,
    url: model.url,
    type: normalizeModelType(model.type),
    ...(modes === undefined ? {} : { modes }),
    language: model.language,
    size: model.size,
    ...(sha256 === undefined ? {} : { sha256 }),
    ...(isRecommended === undefined ? {} : { isRecommended }),
    isArchive: model.isArchive,
    ...(filename === undefined ? {} : { filename }),
    engine: model.engine,
    rules: normalizeModelRules(model.rules),
    ...(groupId === undefined ? {} : { groupId }),
    ...(versionLabel === undefined ? {} : { versionLabel }),
    installPath: model.installPath,
    downloadPath: model.downloadPath,
    isInstalled: model.isInstalled,
  };
}

function normalizeRestoreDefaults(
  restoreDefaults: CoreModelCatalogRestoreDefaults,
): ModelCatalogRestoreDefaults {
  const streamingModelPath = optionalString(restoreDefaults.streamingModelPath);
  const batchModelPath = optionalString(restoreDefaults.batchModelPath);
  const vadModelPath = optionalString(restoreDefaults.vadModelPath);
  const punctuationModelPath = optionalString(restoreDefaults.punctuationModelPath);
  const speakerSegmentationModelPath = optionalString(restoreDefaults.speakerSegmentationModelPath);
  const speakerEmbeddingModelPath = optionalString(restoreDefaults.speakerEmbeddingModelPath);

  return {
    ...(streamingModelPath === undefined ? {} : { streamingModelPath }),
    ...(batchModelPath === undefined ? {} : { batchModelPath }),
    ...(vadModelPath === undefined ? {} : { vadModelPath }),
    ...(punctuationModelPath === undefined ? {} : { punctuationModelPath }),
    ...(speakerSegmentationModelPath === undefined ? {} : { speakerSegmentationModelPath }),
    ...(speakerEmbeddingModelPath === undefined ? {} : { speakerEmbeddingModelPath }),
    enableITN: restoreDefaults.enableItn,
    batchVadEnabled: restoreDefaults.batchVadEnabled,
    vadBufferSize: requireFiniteNumber(restoreDefaults.vadBufferSize, 'vadBufferSize'),
    maxConcurrent: restoreDefaults.maxConcurrent,
  };
}

function normalizeModelCatalogSnapshot(
  snapshot: CoreModelCatalogSnapshot,
): UiModelCatalogSnapshot {
  return {
    modelsDir: snapshot.modelsDir,
    models: snapshot.models.map(normalizeCatalogModel),
    sections: snapshot.sections.map((section) => ({
      type: normalizeCatalogSectionType(section.type),
      groups: section.groups.map((group) => ({
        key: group.key,
        models: group.models.map(normalizeCatalogModel),
      })),
    })),
    selectionOptions: snapshot.selectionOptions,
    modelPathById: snapshot.modelPathById,
    modelIdByNormalizedPath: snapshot.modelIdByNormalizedPath,
    pathMatchTokens: snapshot.pathMatchTokens,
    dependencyRequestsByModelId: snapshot.dependencyRequestsByModelId,
    restoreDefaults: normalizeRestoreDefaults(snapshot.restoreDefaults),
  };
}

function normalizeModelCatalogSelectedIds(
  selectedIds: CoreModelCatalogSelectedIds,
): UiModelCatalogSelectedIds {
  return {
    streaming: selectedIds.streaming ?? null,
    batch: selectedIds.batch ?? null,
    speakerSegmentation: selectedIds.speakerSegmentation ?? null,
    speakerEmbedding: selectedIds.speakerEmbedding ?? null,
  };
}

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

export async function getModelCatalogSnapshot(): Promise<UiModelCatalogSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.app.getModelCatalogSnapshot);
  return normalizeModelCatalogSnapshot(snapshot);
}

export async function resolveModelCatalogSelectedIds(
  paths: ModelSelectionPaths,
): Promise<ModelCatalogSelectedIds> {
  const selectedIds = await invokeTauri(TauriCommand.app.resolveModelCatalogSelectedIds, { paths });
  return normalizeModelCatalogSelectedIds(selectedIds);
}

export async function getDiagnosticsCoreSnapshot(
  input: DiagnosticsCoreInput,
): Promise<DiagnosticsCoreFactsSnapshot> {
  const snapshot = await invokeTauri(TauriCommand.app.getDiagnosticsCoreSnapshot, {
    input: buildDiagnosticsTransportInput(input),
  });
  return normalizeDiagnosticsSnapshot(snapshot);
}

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
  legacyConfig: unknown,
  defaultRuleSetName: string,
): Promise<AppConfigMigrationResult> {
  const res = await invokeTauri(TauriCommand.app.migrateAppConfig, {
    savedConfig: savedConfig ?? null,
    legacyConfig: legacyConfig ?? null,
    defaultRuleSetName,
  });
  return {
    ...res,
    config: flattenAppConfig(res.config)
  };
}

export async function resolveEffectiveConfig(
  globalConfig: AppConfig,
  project: ProjectRecord | null,
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
