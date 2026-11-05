import {
  addLlmModel,
  findLlmModelId,
  getOrderedLlmModels,
  setFeatureModelSelection,
} from '../../../services/llm/state';
import type { AppConfig, AsrModelSelection } from '../../../types/config';
import type { LlmSettings } from '../../../types/llm';
import { type ModelInfo, PRESET_MODELS, PRESET_MODELS_MAP } from '../../../types/modelCatalog';
import type { ScenarioModelKind } from '../../../utils/scenarioModels';

type Translate = (key: string, options?: Record<string, unknown>) => string;

export const SENSEVOICE_STANDARD_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
export const WHISPER_TURBO_ID = 'sherpa-onnx-whisper-turbo';
export const RECOMMENDED_SEGMENTATION_ID = 'sherpa-onnx-pyannote-segmentation-3-0';
export const RECOMMENDED_EMBEDDING_ID =
  '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx';

export const CURATED_GROUP_ORDER = ['sensevoice', 'whisper', 'paraformer', 'parakeet-tdt'] as const;

const ASR_TYPES = new Set<ModelInfo['type']>([
  'zipformer',
  'sensevoice',
  'paraformer',
  'whisper',
  'funasr-nano',
  'fire-red-asr',
  'dolphin',
  'qwen3-asr',
  'parakeet-tdt',
  'moonshine',
  'omnilingual',
  'canary',
]);

export type SpeechScope = 'both' | 'live' | 'batch';
export type ModelPageScope =
  | 'live'
  | 'batch'
  | 'downloaded'
  | 'embedding-live'
  | 'embedding-batch'
  | 'segmentation-live'
  | 'segmentation-batch'
  | 'alignment-live'
  | 'alignment-batch';
export type SpeechAiScreen = 'home' | 'model' | 'ai';
export type LanguageFilter = 'any' | 'zh' | 'en' | 'many' | 'installed';

const COMPANION_TYPES = new Set<ModelInfo['type']>([
  'speaker-segmentation',
  'speaker-embedding',
  'alignment',
]);

export interface CuratedFamily {
  groupId: string;
  titleKey: string;
  defaultVariantId: string;
}

export const CURATED_FAMILIES: Record<string, CuratedFamily> = {
  sensevoice: {
    groupId: 'sensevoice',
    titleKey: 'settings.speech_ai.family.sensevoice',
    defaultVariantId: SENSEVOICE_STANDARD_ID,
  },
  whisper: {
    groupId: 'whisper',
    titleKey: 'settings.speech_ai.family.whisper',
    defaultVariantId: WHISPER_TURBO_ID,
  },
  paraformer: {
    groupId: 'paraformer',
    titleKey: 'settings.speech_ai.family.paraformer',
    defaultVariantId: 'sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en-int8',
  },
  'parakeet-tdt': {
    groupId: 'parakeet-tdt',
    titleKey: 'settings.speech_ai.family.parakeet',
    defaultVariantId: 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8',
  },
};

export function isEnglishUi(language: string | undefined): boolean {
  const value = (language ?? '').toLowerCase();
  return value === 'en' || value.startsWith('en-');
}

export function recommendedGroupId(language: string | undefined): string {
  return isEnglishUi(language) ? 'whisper' : 'sensevoice';
}

export function recommendedVariantId(language: string | undefined): string {
  return recommendedGroupId(language) === 'whisper' ? WHISPER_TURBO_ID : SENSEVOICE_STANDARD_ID;
}

export function curatedGroupOrder(language: string | undefined): string[] {
  const recommended = recommendedGroupId(language);
  return [recommended, ...CURATED_GROUP_ORDER.filter((id) => id !== recommended)];
}

export function isAsrCatalogModel(model: ModelInfo): boolean {
  return ASR_TYPES.has(model.type);
}

export function groupKeyForModel(model: ModelInfo): string {
  return model.groupId || model.id;
}

export interface PickerGroup {
  key: string;
  models: ModelInfo[];
  curated?: CuratedFamily;
}

export function groupAsrModels(models: ModelInfo[]): PickerGroup[] {
  const groups = new Map<string, ModelInfo[]>();
  for (const model of models) {
    if (!isAsrCatalogModel(model)) continue;
    const key = groupKeyForModel(model);
    const list = groups.get(key) ?? [];
    list.push(model);
    groups.set(key, list);
  }
  return [...groups.entries()].map(([key, members]) => ({
    key,
    models: sortVersions(key, members),
    curated: CURATED_FAMILIES[key],
  }));
}

function sortVersions(groupId: string, models: ModelInfo[]): ModelInfo[] {
  const preferred = CURATED_FAMILIES[groupId]?.defaultVariantId;
  return [...models].sort((a, b) => {
    if (a.id === preferred) return -1;
    if (b.id === preferred) return 1;
    return 0;
  });
}

export function matchesLanguageFilter(model: ModelInfo, filter: LanguageFilter): boolean {
  if (filter === 'any' || filter === 'installed') return true;
  const languages = model.languages ?? [];
  if (filter === 'zh') return languages.includes('zh') || languages.includes('yue');
  if (filter === 'en') return languages.includes('en');
  return languages.length >= 8;
}

export function orderPickerGroups(
  groups: PickerGroup[],
  language: string | undefined,
  filter: LanguageFilter,
  showAll: boolean
): PickerGroup[] {
  const order = curatedGroupOrder(language);
  const filtered = groups
    .map((group) => ({
      ...group,
      models: group.models.filter((model) => matchesLanguageFilter(model, filter)),
    }))
    .filter((group) => group.models.length > 0);
  const curated = order
    .map((id) => filtered.find((group) => group.key === id))
    .filter((group): group is PickerGroup => Boolean(group));
  if (!showAll) return curated;
  const curatedIds = new Set(curated.map((group) => group.key));
  const rest = filtered
    .filter((group) => !curatedIds.has(group.key))
    .sort((a, b) => a.models[0].name.localeCompare(b.models[0].name));
  return [...curated, ...rest];
}

export function presetModelById(id: string): ModelInfo | undefined {
  return PRESET_MODELS_MAP.get(id) ?? PRESET_MODELS.find((model) => model.id === id);
}

export function displaySize(size: string | undefined): string {
  return (size ?? '').replace(/^~\s*/, '').trim();
}

export function selectionForScope(
  config: Pick<AppConfig, 'asr'>,
  scope: 'live' | 'batch'
): AsrModelSelection | undefined {
  return config.asr?.selections?.[scope];
}

export function modelIdForSelection(selection: AsrModelSelection | undefined): string | null {
  if (!selection) return null;
  if (selection.engine === 'online') {
    const provider = selection.providerId ?? '';
    return selection.modelId ? `${provider}::${selection.modelId}` : provider || null;
  }
  return selection.modelId ?? null;
}

type InstallableModel = ModelInfo & { installPath?: string };

export function pathIsInstalled(
  path: string | undefined,
  models: InstallableModel[],
  installedIds: Set<string>
): boolean {
  if (!path?.trim()) return false;
  const normalized = path.replace(/\\/g, '/').toLowerCase();
  const match = models.find((model) => {
    const install = model.installPath?.replace(/\\/g, '/').toLowerCase();
    return Boolean(install) && install === normalized;
  });
  return Boolean(match && installedIds.has(match.id));
}

/** Speakers can be Ready only when every enabled mode has its helper files installed. */
export function speakerHelpersReady(
  config: Pick<
    AppConfig,
    | 'liveSpeakerEmbeddingModelPath'
    | 'batchSpeakerEmbeddingModelPath'
    | 'liveSpeakerSegmentationModelPath'
    | 'batchSpeakerSegmentationModelPath'
  > &
    Partial<Pick<AppConfig, 'liveSpeakerDiarizationEnabled' | 'batchSpeakerDiarizationEnabled'>>,
  models: InstallableModel[],
  installedIds: Set<string>
): boolean {
  const readyFor = (scope: 'live' | 'batch') => {
    if (!speakersEnabled(config, scope)) return true;
    const embedding =
      scope === 'live'
        ? config.liveSpeakerEmbeddingModelPath
        : config.batchSpeakerEmbeddingModelPath;
    const segmentation =
      scope === 'live'
        ? config.liveSpeakerSegmentationModelPath
        : config.batchSpeakerSegmentationModelPath;
    if (!pathIsInstalled(embedding, models, installedIds)) return false;
    if (scope === 'batch' && !pathIsInstalled(segmentation, models, installedIds)) return false;
    return true;
  };
  return readyFor('live') && readyFor('batch');
}

/** Main's diarization switches, not the presence of a helper file. */
export function speakersEnabled(
  config: Partial<
    Pick<AppConfig, 'liveSpeakerDiarizationEnabled' | 'batchSpeakerDiarizationEnabled'>
  >,
  scope: 'live' | 'batch'
): boolean {
  return scope === 'batch'
    ? Boolean(config.batchSpeakerDiarizationEnabled)
    : Boolean(config.liveSpeakerDiarizationEnabled);
}

export function scopeScenario(scope: ModelPageScope): 'live' | 'batch' | null {
  if (scope === 'live' || scope.endsWith('-live')) return 'live';
  if (scope === 'batch' || scope.endsWith('-batch')) return 'batch';
  return null;
}

export function companionType(scope: ModelPageScope): ModelInfo['type'] | null {
  if (scope.startsWith('embedding')) return 'speaker-embedding';
  if (scope.startsWith('segmentation')) return 'speaker-segmentation';
  if (scope.startsWith('alignment')) return 'alignment';
  return null;
}

export function companionKind(scope: ModelPageScope): ScenarioModelKind | null {
  if (scope.startsWith('embedding')) return 'speakerEmbeddingModelPath';
  if (scope.startsWith('segmentation')) return 'speakerSegmentationModelPath';
  if (scope.startsWith('alignment')) return 'alignmentModelPath';
  return null;
}

export function speakerPathsEqual(
  config: Pick<
    AppConfig,
    | 'liveSpeakerEmbeddingModelPath'
    | 'batchSpeakerEmbeddingModelPath'
    | 'liveSpeakerSegmentationModelPath'
    | 'batchSpeakerSegmentationModelPath'
  >
): boolean {
  return (
    (config.liveSpeakerEmbeddingModelPath ?? '') ===
      (config.batchSpeakerEmbeddingModelPath ?? '') &&
    (config.liveSpeakerSegmentationModelPath ?? '') ===
      (config.batchSpeakerSegmentationModelPath ?? '')
  );
}

export function recognitionSelectionsEqual(config: Pick<AppConfig, 'asr'>): boolean {
  const live = selectionForScope(config, 'live');
  const batch = selectionForScope(config, 'batch');
  if (!live && !batch) return true;
  if (!live || !batch) return false;
  if (live.engine !== batch.engine) return false;
  if (live.engine === 'online') {
    return (
      (live.providerId ?? '') === (batch.providerId ?? '') &&
      (live.modelId ?? '') === (batch.modelId ?? '')
    );
  }
  return (
    (live.modelId ?? '') === (batch.modelId ?? '') &&
    (live.modelPath ?? '') === (batch.modelPath ?? '')
  );
}

/** Explicit flag wins. Otherwise link only when recording and files already match. */
export function modesAreLinked(
  config: Pick<
    AppConfig,
    | 'asr'
    | 'speechModesLinked'
    | 'liveSpeakerEmbeddingModelPath'
    | 'batchSpeakerEmbeddingModelPath'
    | 'liveSpeakerSegmentationModelPath'
    | 'batchSpeakerSegmentationModelPath'
  >
): boolean {
  if (config.speechModesLinked === true) return true;
  if (config.speechModesLinked === false) return false;
  return recognitionSelectionsEqual(config) && speakerPathsEqual(config);
}

export function speakerCountLabel(count: number | null | undefined, t: Translate): string {
  if (count == null || count < 2) {
    return t('settings.speech_ai.automatic');
  }
  return String(count);
}

export function familyTitle(model: ModelInfo | undefined, t: Translate): string {
  if (!model) return '';
  const curated = CURATED_FAMILIES[groupKeyForModel(model)];
  if (curated) return t(curated.titleKey);
  return model.name;
}

export function resolveModelTitle(
  config: Pick<AppConfig, 'asr'>,
  scope: 'live' | 'batch',
  catalogModels: ModelInfo[],
  t: Translate
): string {
  const selection = selectionForScope(config, scope);
  if (!selection || (!selection.modelId && !selection.modelPath && selection.engine !== 'online')) {
    return '';
  }
  if (selection.engine === 'online') {
    return selection.providerId || t('settings.speech_ai.cloud_model');
  }
  const id = selection.modelId;
  const fromCatalog = id
    ? catalogModels.find((model) => model.id === id)
    : catalogModels.find((model) => model.id && selection.modelPath?.includes(model.id));
  const preset = id ? presetModelById(id) : undefined;
  return familyTitle(fromCatalog ?? preset, t);
}

export function formatByteLabel(bytes: number | undefined): string {
  if (!bytes || bytes <= 0) return '';
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.max(1, Math.round(mb))} MB`;
}

export function helperModel(id: string): ModelInfo | undefined {
  return presetModelById(id);
}

export interface SpeechDownloadPick {
  modelId: string;
  status: string;
  progress: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: string;
  error?: string;
}

/** Prefer an in-progress download of the assigned speech model, then any ASR download. */
export function pickSpeechDownload(
  config: Pick<AppConfig, 'asr' | 'speechModesLinked'>,
  store: Record<string, SpeechDownloadPick>,
  recommendedId: string
): SpeechDownloadPick | undefined {
  const liveId = modelIdForSelection(selectionForScope(config, 'live'));
  const batchId = modelIdForSelection(selectionForScope(config, 'batch'));
  const assigned = [liveId, batchId].filter(
    (id): id is string => typeof id === 'string' && !id.includes('::')
  );
  const candidates = assigned.length > 0 ? assigned : [recommendedId];
  const downloading = Object.values(store).filter((item) => item.status === 'downloading');
  const assignedActive = downloading.find((item) => candidates.includes(item.modelId));
  if (assignedActive) return assignedActive;
  const asrActive = downloading.find((item) => {
    const preset = PRESET_MODELS_MAP.get(item.modelId);
    return preset ? isAsrCatalogModel(preset) : false;
  });
  if (asrActive) return asrActive;
  const failed = Object.values(store).find(
    (item) => item.status === 'failed' && candidates.includes(item.modelId)
  );
  return failed;
}

export function speechModelsAssigned(
  config: Pick<AppConfig, 'asr' | 'speechModesLinked'>,
  installedIds: Set<string>
): boolean {
  const linked = modesAreLinked(config);
  const liveId = modelIdForSelection(selectionForScope(config, 'live'));
  const batchId = modelIdForSelection(selectionForScope(config, 'batch'));
  const required = linked ? [liveId] : [liveId, batchId];
  if (required.some((id) => !id)) return false;
  return !required.some(
    (id) => id && !id.includes('::') && PRESET_MODELS_MAP.has(id) && !installedIds.has(id)
  );
}

type Installable = ModelInfo & { installPath?: string };

export function modelForPath(
  path: string | undefined,
  models: Installable[]
): Installable | undefined {
  if (!path?.trim()) return undefined;
  const normalized = path.replace(/\\/g, '/').toLowerCase();
  return models.find((model) => {
    const install = model.installPath?.replace(/\\/g, '/').toLowerCase();
    if (install && install === normalized) return true;
    const id = model.id.toLowerCase();
    return normalized.includes(`/${id}`) || normalized.endsWith(id);
  });
}

function sizeToMb(size: string | undefined): number {
  const text = (size ?? '').replace(/^~\s*/, '').trim().toUpperCase();
  const value = Number.parseFloat(text);
  if (!Number.isFinite(value)) return 0;
  if (text.includes('GB')) return value * 1024;
  if (text.includes('KB')) return value / 1024;
  return value;
}

export function summarizeInstalled(
  models: Installable[],
  installedIds: Set<string>
): { count: number; megabytes: number } {
  const picked = models.filter(
    (model) =>
      installedIds.has(model.id) && (isAsrCatalogModel(model) || COMPANION_TYPES.has(model.type))
  );
  return {
    count: picked.length,
    megabytes: picked.reduce((sum, model) => sum + sizeToMb(model.size), 0),
  };
}

export function formatMegabytes(megabytes: number): string {
  if (megabytes >= 1024) {
    const gb = megabytes / 1024;
    const text = gb >= 10 ? gb.toFixed(0) : gb.toFixed(2);
    return `${text.replace(/0+$/, '').replace(/\.$/, '')} GB`;
  }
  if (megabytes <= 0) return '0 MB';
  return `${Math.max(1, Math.round(megabytes))} MB`;
}

const GOOGLE_PROVIDERS = new Set(['google_translate', 'google_translate_free']);

export function isGoogleProvider(provider: string | undefined): boolean {
  return Boolean(provider && GOOGLE_PROVIDERS.has(provider));
}

export function isLocalLlmProvider(provider: string | undefined): boolean {
  return provider === 'ollama' || provider === 'lm_studio';
}

export function hasWritingModel(settings: LlmSettings | undefined): boolean {
  return getOrderedLlmModels(settings).some((entry) => !isGoogleProvider(entry.provider));
}

/** First connection fills polish and summary. Translation changes only when asked. */
export function assignConnectedModel(
  before: LlmSettings,
  afterAdd: LlmSettings,
  modelId: string | undefined,
  includeTranslation: boolean
): LlmSettings {
  if (!modelId) return afterAdd;
  let next = afterAdd;
  if (!hasWritingModel(before)) {
    next = setFeatureModelSelection(next, 'polish', modelId);
    next = setFeatureModelSelection(next, 'summary', modelId);
  }
  if (includeTranslation) {
    next = setFeatureModelSelection(next, 'translation', modelId);
  }
  return next;
}

export function setPolishAndSummary(settings: LlmSettings, modelId: string): LlmSettings {
  return setFeatureModelSelection(
    setFeatureModelSelection(settings, 'polish', modelId),
    'summary',
    modelId
  );
}

export const GOOGLE_TRANSLATE_VALUE = 'google';

export function setTranslationChoice(settings: LlmSettings, value: string): LlmSettings {
  if (value !== GOOGLE_TRANSLATE_VALUE) {
    return setFeatureModelSelection(settings, 'translation', value);
  }
  let next = settings;
  let id = findLlmModelId(next, 'google_translate_free', 'default');
  if (!id) {
    next = addLlmModel(next, { provider: 'google_translate_free', model: 'default' });
    id = findLlmModelId(next, 'google_translate_free', 'default');
  }
  return id ? setFeatureModelSelection(next, 'translation', id) : next;
}
