import type {
  ModelCatalogModel as CoreModelCatalogModel,
  ModelCatalogRestoreDefaults as CoreModelCatalogRestoreDefaults,
  ModelCatalogSelectedIds as CoreModelCatalogSelectedIds,
  ModelCatalogSnapshot as CoreModelCatalogSnapshot,
} from '../bindings';
import type {
  ModelCatalogModel,
  ModelCatalogRestoreDefaults,
  ModelInfo,
  ModelRules,
  TimestampSupportHint,
  ModelCatalogSelectedIds as UiModelCatalogSelectedIds,
  ModelCatalogSnapshot as UiModelCatalogSnapshot,
} from '../types/modelCatalog';

function optionalString(value: string | null | undefined): string | undefined {
  return value ?? undefined;
}

function requireFiniteNumber(value: number | null, fieldName: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`Expected a finite number for model catalog ${fieldName}`);
  }
  return value;
}

function normalizeModelRules(modelRules: CoreModelCatalogModel['rules']): ModelRules {
  const timestampSupportHint = (modelRules.timestampSupportHint ?? undefined) as
    | TimestampSupportHint
    | undefined;

  return {
    requiresVad: modelRules.requiresVad,
    requiresPunctuation: modelRules.requiresPunctuation,
    ...(timestampSupportHint === undefined ? {} : { timestampSupportHint }),
    ...(modelRules.initialRefreshRateMs != null
      ? { initialRefreshRateMs: modelRules.initialRefreshRateMs }
      : {}),
  };
}

const LANGUAGE_MODES = ['selectable', 'auto', 'fixed', 'none'] as const;

function normalizeLanguageMode(value: string): (typeof LANGUAGE_MODES)[number] {
  return (LANGUAGE_MODES as readonly string[]).includes(value)
    ? (value as (typeof LANGUAGE_MODES)[number])
    : 'none';
}

function normalizeModelArtifacts(
  artifacts: CoreModelCatalogModel['artifacts']
): NonNullable<ModelInfo['artifacts']> {
  return (artifacts ?? []).map((artifact) => ({
    url: artifact.url,
    filename: artifact.filename,
    ...(artifact.sha256 === null || artifact.sha256 === undefined
      ? {}
      : { sha256: artifact.sha256 }),
    ...(artifact.sizeBytes === null || artifact.sizeBytes === undefined
      ? {}
      : { sizeBytes: artifact.sizeBytes }),
  }));
}

function normalizeCatalogModel(model: CoreModelCatalogModel): ModelCatalogModel {
  const artifacts = normalizeModelArtifacts(model.artifacts);
  const isRecommended = model.isRecommended ?? undefined;
  const filename = optionalString(model.filename);
  const groupId = optionalString(model.groupId);
  const versionLabel = optionalString(model.versionLabel);

  return {
    id: model.id,
    name: model.name,
    description: model.description,
    type: model.type,
    ...(model.modes ? { modes: model.modes } : {}),
    languages: Array.isArray(model.languages) ? model.languages : [],
    languageMode: normalizeLanguageMode(model.languageMode),
    size: model.size,
    ...(artifacts.length === 0 ? {} : { artifacts }),
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
  restoreDefaults: CoreModelCatalogRestoreDefaults
): ModelCatalogRestoreDefaults {
  const streamingModelPath = optionalString(restoreDefaults.streamingModelPath);
  const batchModelPath = optionalString(restoreDefaults.batchModelPath);
  const vadModelPath = optionalString(restoreDefaults.vadModelPath);
  const punctuationModelPath = optionalString(restoreDefaults.punctuationModelPath);
  const speakerSegmentationModelPath = optionalString(restoreDefaults.speakerSegmentationModelPath);
  const speakerEmbeddingModelPath = optionalString(restoreDefaults.speakerEmbeddingModelPath);
  const alignmentModelPath = optionalString(restoreDefaults.alignmentModelPath);

  return {
    ...(streamingModelPath === undefined ? {} : { streamingModelPath }),
    ...(batchModelPath === undefined ? {} : { batchModelPath }),
    ...(vadModelPath === undefined ? {} : { vadModelPath }),
    ...(punctuationModelPath === undefined ? {} : { punctuationModelPath }),
    ...(speakerSegmentationModelPath === undefined ? {} : { speakerSegmentationModelPath }),
    ...(speakerEmbeddingModelPath === undefined ? {} : { speakerEmbeddingModelPath }),
    ...(alignmentModelPath === undefined ? {} : { alignmentModelPath }),
    enableITN: restoreDefaults.enableItn,
    batchVadEnabled: restoreDefaults.batchVadEnabled,
    vadBufferSize: requireFiniteNumber(restoreDefaults.vadBufferSize, 'vadBufferSize'),
    maxConcurrent: restoreDefaults.maxConcurrent,
  };
}

export function normalizeModelCatalogSnapshot(
  snapshot: CoreModelCatalogSnapshot
): UiModelCatalogSnapshot {
  return {
    modelsDir: snapshot.modelsDir,
    models: snapshot.models.map(normalizeCatalogModel),
    sections: snapshot.sections.map((section) => ({
      type: section.type,
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

export function normalizeModelCatalogSelectedIds(
  selectedIds: CoreModelCatalogSelectedIds
): UiModelCatalogSelectedIds {
  return {
    streaming: selectedIds.streaming ?? null,
    batch: selectedIds.batch ?? null,
    speakerSegmentation: selectedIds.speakerSegmentation ?? null,
    speakerEmbedding: selectedIds.speakerEmbedding ?? null,
    alignment: selectedIds.alignment ?? null,
  };
}
