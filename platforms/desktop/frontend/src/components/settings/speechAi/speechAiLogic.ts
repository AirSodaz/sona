import type { AppConfig, AsrModelSelection } from '../../../types/config';
import { type ModelInfo, PRESET_MODELS, PRESET_MODELS_MAP } from '../../../types/modelCatalog';

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
export type SpeechAiScreen = 'home' | 'picker' | 'recognition' | 'speakers' | 'ai' | 'downloads';

export type CapabilityStatus = 'idle' | 'ok' | 'busy' | 'warn' | 'err';
export type SpeechAction = 'download' | 'use' | 'cancel' | 'retry' | 'change' | 'manage';
export type LanguageFilter = 'any' | 'zh' | 'en' | 'many';
export type FriendlyTier = 'standard' | 'smaller' | 'full';

export interface CuratedFamily {
  groupId: string;
  titleKey: string;
  titleDefault: string;
  blurbKey: string;
  blurbDefault: string;
  defaultVariantId: string;
}

export const CURATED_FAMILIES: Record<string, CuratedFamily> = {
  sensevoice: {
    groupId: 'sensevoice',
    titleKey: 'settings.speech_ai.family.sensevoice',
    titleDefault: 'SenseVoice Small',
    blurbKey: 'settings.speech_ai.blurb.sensevoice',
    blurbDefault: 'Fast and accurate for everyday Chinese and English. Works live and with files.',
    defaultVariantId: SENSEVOICE_STANDARD_ID,
  },
  whisper: {
    groupId: 'whisper',
    titleKey: 'settings.speech_ai.family.whisper',
    titleDefault: 'Whisper Large v3 Turbo',
    blurbKey: 'settings.speech_ai.blurb.whisper',
    blurbDefault: 'Best for many languages and accented speech. Slower on older computers.',
    defaultVariantId: WHISPER_TURBO_ID,
  },
  paraformer: {
    groupId: 'paraformer',
    titleKey: 'settings.speech_ai.family.paraformer',
    titleDefault: 'Paraformer',
    blurbKey: 'settings.speech_ai.blurb.paraformer',
    blurbDefault: 'Very fast Chinese. Good for long meetings on laptops.',
    defaultVariantId: 'sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en-int8',
  },
  'parakeet-tdt': {
    groupId: 'parakeet-tdt',
    titleKey: 'settings.speech_ai.family.parakeet',
    titleDefault: 'Parakeet TDT',
    blurbKey: 'settings.speech_ai.blurb.parakeet',
    blurbDefault: 'Fast European-language recognition with punctuation built in.',
    defaultVariantId: 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8',
  },
};

export interface VersionPresentation {
  tier: FriendlyTier;
  titleKey: string;
  titleDefault: string;
  hint: string;
}

const ENGINE_TOKEN: Record<ModelInfo['engine'], string> = {
  'sherpa-onnx': 'ONNX',
  'llama-cpp': 'GGUF',
  'onnx-nmt': 'NMT',
};

const QUANT_TOKEN = /\b(?:int8|fp32|fp16|f16|q\d+(?:_\d+)?|quantized)\b/gi;

export function friendlyVersion(
  model: Pick<ModelInfo, 'versionLabel' | 'engine'>
): VersionPresentation {
  const raw = (model.versionLabel ?? '').trim();
  const lower = raw.toLowerCase();
  const token = ENGINE_TOKEN[model.engine] ?? '';
  let tier: FriendlyTier = 'standard';
  if (/fp32|fp16|f16|full/.test(lower)) {
    tier = 'full';
  } else if (/q5|q4|q6|q3|q2|small|medium|lite|tiny/.test(lower)) {
    tier = 'smaller';
  }
  const title =
    tier === 'full'
      ? { titleKey: 'settings.speech_ai.ver_full', titleDefault: 'Full precision' }
      : tier === 'smaller'
        ? { titleKey: 'settings.speech_ai.ver_smaller', titleDefault: 'Smaller' }
        : { titleKey: 'settings.speech_ai.ver_standard', titleDefault: 'Standard' };
  const hint = [raw, token].filter(Boolean).join(' ');
  return { tier, hint, ...title };
}

/** Size, latency, or product name left after the codec token is removed. */
export function versionIdentity(versionLabel: string | undefined): string {
  return (versionLabel ?? '')
    .replace(QUANT_TOKEN, ' ')
    .replace(/\s+/g, ' ')
    .replace(/\(\s+/g, '(')
    .replace(/\s+\)/g, ')')
    .replace(/\(\s*\)/g, '')
    .trim();
}

function isSizeOrLatency(identity: string): boolean {
  return /(\d+(?:\.\d+)?\s*[bm]\b)|\b\d+\s*ms\b|\bflash\b/i.test(identity);
}

export interface VersionRowLabel {
  id: string;
  title: string;
  hint: string;
}

/**
 * Friendly row titles that stay unique inside one model family.
 * Codec tokens stay in the secondary hint unless two rows would otherwise match.
 */
export function versionRowLabels(models: ModelInfo[], t: Translate): VersionRowLabel[] {
  const drafts = models.map((model) => {
    const tier = friendlyVersion(model);
    const tierLabel = t(tier.titleKey, { defaultValue: tier.titleDefault });
    const identity = versionIdentity(model.versionLabel);
    const title = !identity
      ? tierLabel
      : isSizeOrLatency(identity)
        ? `${tierLabel} · ${identity}`
        : identity;
    const codec = (model.versionLabel ?? '').match(
      /\b(?:int8|fp32|fp16|f16|q\d+(?:_\d+)?)\b/i
    )?.[0];
    return {
      id: model.id,
      title,
      hint: tier.hint,
      codec: codec ?? '',
    };
  });
  const counts = new Map<string, number>();
  for (const row of drafts) {
    counts.set(row.title, (counts.get(row.title) ?? 0) + 1);
  }
  return drafts.map((row) => ({
    id: row.id,
    title:
      (counts.get(row.title) ?? 0) > 1 && row.codec ? `${row.title} · ${row.codec}` : row.title,
    hint: row.hint,
  }));
}

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
  const rank = (model: ModelInfo) => {
    if (model.id === preferred) return 0;
    const tier = friendlyVersion(model).tier;
    if (tier === 'standard') return 1;
    if (tier === 'smaller') return 2;
    return 3;
  };
  return [...models].sort((a, b) => rank(a) - rank(b));
}

export function matchesLanguageFilter(model: ModelInfo, filter: LanguageFilter): boolean {
  if (filter === 'any') return true;
  const languages = model.languages ?? [];
  if (filter === 'zh') return languages.includes('zh') || languages.includes('yue');
  if (filter === 'en') return languages.includes('en') && !languages.includes('zh');
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
  >,
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

export function speakersEnabled(
  config: Pick<AppConfig, 'liveSpeakerEmbeddingModelPath' | 'batchSpeakerEmbeddingModelPath'>,
  scope: 'live' | 'batch'
): boolean {
  const path =
    scope === 'live' ? config.liveSpeakerEmbeddingModelPath : config.batchSpeakerEmbeddingModelPath;
  return Boolean(path?.trim());
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
    return t('settings.speech_ai.detect', { defaultValue: 'Detect' });
  }
  return String(count);
}

export function familyTitle(model: ModelInfo | undefined, t: Translate): string {
  if (!model) return '';
  const curated = CURATED_FAMILIES[groupKeyForModel(model)];
  if (curated) return t(curated.titleKey, { defaultValue: curated.titleDefault });
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
    return selection.providerId || t('settings.speech_ai.cloud_model', { defaultValue: 'Cloud' });
  }
  const id = selection.modelId;
  const fromCatalog = id
    ? catalogModels.find((model) => model.id === id)
    : catalogModels.find((model) => model.id && selection.modelPath?.includes(model.id));
  const preset = id ? presetModelById(id) : undefined;
  return familyTitle(fromCatalog ?? preset, t);
}

export interface SpeechRowModel {
  status: CapabilityStatus;
  statusLabel: string;
  detail: string;
  lines: string[];
  action: SpeechAction;
  progress: number | null;
  progressLabel: string;
}

export function buildSpeechRow(options: {
  config: AppConfig;
  catalogModels: ModelInfo[];
  installedIds: Set<string>;
  download?: {
    status: string;
    progress: number;
    downloadedBytes?: number;
    totalBytes?: number;
    speed?: string;
    error?: string;
  };
  recommendedId: string;
  downloadName?: string;
  offline: boolean;
  t: Translate;
}): SpeechRowModel {
  const { config, catalogModels, installedIds, download, recommendedId, downloadName, offline, t } =
    options;
  const linked = modesAreLinked(config);
  const liveTitle = resolveModelTitle(config, 'live', catalogModels, t);
  const batchTitle = resolveModelTitle(config, 'batch', catalogModels, t);
  const liveOn = speakersEnabled(config, 'live');
  const batchOn = speakersEnabled(config, 'batch');
  const count = speakerCountLabel(config.speakerExpectedCount, t);
  const speakersBit = (on: boolean) =>
    on
      ? t('settings.speech_ai.speakers_on', {
          number: count,
          defaultValue: `Speakers on · ${count}`,
        })
      : t('settings.speech_ai.speakers_off', { defaultValue: 'Speakers off' });

  const recommended = presetModelById(recommendedId);
  const recommendedName = familyTitle(recommended, t);
  const activeName = downloadName || recommendedName;
  const recommendedSize = displaySize(recommended?.size);

  if (download?.status === 'downloading') {
    const done = formatByteLabel(download.downloadedBytes);
    const total = formatByteLabel(download.totalBytes) || recommendedSize;
    const progressLabel = [done && total ? `${done} of ${total}` : '', download.speed ?? '']
      .filter(Boolean)
      .join(' · ');
    return {
      status: 'busy',
      statusLabel: t('settings.speech_ai.downloading', { defaultValue: 'Downloading' }),
      detail: [activeName, progressLabel].filter(Boolean).join(' · '),
      lines: [
        t('settings.speech_ai.download_continues', {
          defaultValue:
            "You can close Settings — the download continues and Sona will tell you when it's ready.",
        }),
      ],
      action: 'cancel',
      progress: download.progress ?? 0,
      progressLabel,
    };
  }

  if (download?.status === 'failed') {
    return {
      status: 'err',
      statusLabel: t('settings.speech_ai.download_failed', { defaultValue: 'Download failed' }),
      detail: t('settings.speech_ai.download_failed_detail', {
        model: activeName,
        defaultValue: `${activeName} stopped. Check your connection or try another download source.`,
      }),
      lines: [],
      action: 'retry',
      progress: null,
      progressLabel: '',
    };
  }

  const liveReady = Boolean(liveTitle);
  const batchReady = Boolean(batchTitle);
  const assigned = linked ? liveReady : liveReady && batchReady;
  const catalogReady = installedIds.size > 0 || catalogModels.length > 0;
  const liveId = modelIdForSelection(selectionForScope(config, 'live'));
  const batchId = modelIdForSelection(selectionForScope(config, 'batch'));
  const missingInstall =
    catalogReady &&
    assigned &&
    [liveId, linked ? liveId : batchId].some(
      (id) => id && !id.includes('::') && PRESET_MODELS_MAP.has(id) && !installedIds.has(id)
    );

  if (
    assigned &&
    !missingInstall &&
    (liveOn || batchOn) &&
    !speakerHelpersReady(config, catalogModels, installedIds)
  ) {
    return {
      status: 'warn',
      statusLabel: t('settings.speech_ai.helpers_missing', { defaultValue: 'Helpers missing' }),
      detail: t('settings.speech_ai.helpers_missing_detail', {
        defaultValue:
          'Speaker helpers are not installed yet. Download them from Speakers, or turn speakers off.',
      }),
      lines: [],
      action: 'manage',
      progress: null,
      progressLabel: '',
    };
  }

  if (!assigned || missingInstall) {
    const installedRecommended = installedIds.has(recommendedId);
    return {
      status: 'idle',
      statusLabel: t('settings.speech_ai.not_set_up', { defaultValue: 'Not set up' }),
      detail: t('settings.speech_ai.speech_blurb', {
        defaultValue:
          'Turns your voice and recordings into text. Runs on this computer — nothing is uploaded.',
      }),
      lines: [
        t('settings.speech_ai.recommended_line', {
          model: recommendedName,
          size: recommendedSize,
          defaultValue: `Recommended: ${recommendedName} · Chinese, English and 3 more · ${recommendedSize}`,
        }),
      ],
      action: installedRecommended ? 'use' : 'download',
      progress: null,
      progressLabel: '',
    };
  }

  if (!linked) {
    return {
      status: 'ok',
      statusLabel: t('settings.speech_ai.ready', { defaultValue: 'Ready' }),
      detail: t('settings.speech_ai.detail_split', {
        defaultValue: 'Different for recording and files',
      }),
      lines: [
        t('settings.speech_ai.sub_recording', {
          model: liveTitle,
          speakers: liveOn ? ` · ${speakersBit(true)}` : '',
          defaultValue: `Recording: ${liveTitle}${liveOn ? ` · ${speakersBit(true)}` : ''}`,
        }),
        t('settings.speech_ai.sub_files', {
          model: batchTitle,
          speakers: ` · ${speakersBit(batchOn)}`,
          defaultValue: `Files: ${batchTitle} · ${speakersBit(batchOn)}`,
        }),
      ],
      action: 'manage',
      progress: null,
      progressLabel: '',
    };
  }

  const lines = liveOn
    ? [
        t('settings.speech_ai.speakers_on', {
          number: count,
          defaultValue: `Speakers on · ${count}`,
        }),
      ]
    : [];
  return {
    status: offline ? 'ok' : 'ok',
    statusLabel: t('settings.speech_ai.ready', { defaultValue: 'Ready' }),
    detail: t('settings.speech_ai.detail_mirrored', {
      model: liveTitle,
      defaultValue: `${liveTitle} · used while recording and for files`,
    }),
    lines,
    action: 'change',
    progress: null,
    progressLabel: '',
  };
}

function formatByteLabel(bytes: number | undefined): string {
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
