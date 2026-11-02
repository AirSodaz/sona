import { Mic } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CudaAddonInspection } from '../../../bindings';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import {
  buildModelSelectionPatch,
  syncOnlineAsrProviderConfig,
} from '../../../services/asrConfigService';
import { cudaAddonService } from '../../../services/cudaAddonService';
import { getProviderDefinition } from '../../../services/llm/providers';
import {
  buildLlmConfigPatch,
  getFeatureModelEntry,
  getOrderedLlmModels,
} from '../../../services/llm/state';
import {
  getOnlineAsrProviderDefinition,
  getOnlineProviderConfig,
  providerSupportsSpeakerDiarization,
} from '../../../services/onlineAsrProviders';
import { isDiarizationDegraded } from '../../../services/speakerService';
import {
  useConfigStore,
  useLlmAssistantConfig,
  useSetConfig,
  useTranscriptionConfig,
} from '../../../stores/configStore';
import { useDialogStore } from '../../../stores/dialogStore';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import type { AppConfig } from '../../../types/config';
import type { ModelInfo } from '../../../types/modelCatalog';
import { PRESET_MODELS } from '../../../types/modelCatalog';
import type { LlmProvider } from '../../../types/transcript';
import { getScenarioModelPath } from '../../../utils/scenarioModels';
import { getCurrentLlmSettings } from '../llm/helpers';
import { DrillRow, SectionLabel, SelectRow, SwitchRow } from './SettingRows';
import {
  displaySize,
  familyTitle,
  formatByteLabel,
  formatMegabytes,
  GOOGLE_TRANSLATE_VALUE,
  isGoogleProvider,
  isLocalLlmProvider,
  type ModelPageScope,
  modelForPath,
  modesAreLinked,
  pickSpeechDownload,
  presetModelById,
  recommendedVariantId,
  resolveModelTitle,
  type SpeechDownloadPick,
  setPolishAndSummary,
  setTranslationChoice,
  speakersEnabled,
  speechModelsAssigned,
  summarizeInstalled,
} from './speechAiLogic';
import { useLlmProbe } from './useLlmProbe';

export function SpeechHome({
  offline,
  onOpen,
}: {
  offline: boolean;
  onOpen: (scope: ModelPageScope | 'ai') => void;
}): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const transcription = useTranscriptionConfig();
  const { installedModels, modelCatalog, downloads, handleDownload, handleCancelDownload } =
    useModelManagerContext();
  const downloadItems = useModelDownloadStore((state) => state.downloads);
  const recommendedId = recommendedVariantId(i18n.resolvedLanguage || i18n.language);
  const catalogModels = modelCatalog.models.length > 0 ? modelCatalog.models : PRESET_MODELS;
  const linked = modesAreLinked(config);
  const ready = speechModelsAssigned(config, installedModels);
  const speechDownload = pickSpeechDownload(
    config,
    Object.fromEntries(
      Object.entries(downloadItems).map(([modelId, item]) => [
        modelId,
        {
          modelId: item.modelId || modelId,
          status: item.status,
          progress: item.progress,
          downloadedBytes: item.downloadedBytes,
          totalBytes: item.totalBytes,
          speed: item.speed,
          error: item.error,
        },
      ])
    ),
    recommendedId
  );
  const managerDownload = speechDownload ? undefined : downloads[recommendedId];
  const activeDownload: SpeechDownloadPick | undefined =
    speechDownload ??
    (managerDownload
      ? { modelId: recommendedId, status: 'downloading', progress: managerDownload.progress }
      : undefined);
  const recommended = presetModelById(recommendedId);
  const recommendedName = familyTitle(recommended, t);
  const downloadModel =
    presetModelById(activeDownload?.modelId ?? '') ??
    catalogModels.find((model) => model.id === activeDownload?.modelId);
  const downloadName = downloadModel ? familyTitle(downloadModel, t) : recommendedName;

  const startRecommended = () => {
    const model = presetModelById(recommendedId);
    if (!model) return;
    if (installedModels.has(model.id)) {
      const livePatch = buildModelSelectionPatch(config, 'live', model.id, modelCatalog);
      const batchPatch = buildModelSelectionPatch(
        { ...config, ...livePatch },
        'batch',
        model.id,
        modelCatalog
      );
      updateConfig({ ...livePatch, ...batchPatch, speechModesLinked: true });
      return;
    }
    void handleDownload(model);
  };

  const liveValue = resolveModelTitle(config, 'live', catalogModels, t);
  const batchValue = linked
    ? t('settings.speech_ai.same_as_live')
    : resolveModelTitle(config, 'batch', catalogModels, t);
  const liveTone = toneFor(activeDownload, 'live', config, ready);
  const batchTone = linked ? undefined : toneFor(activeDownload, 'batch', config, ready);

  const punctuationValue = punctuationSelection(config, catalogModels);
  const punctuationOptions = [
    { value: '', label: t('settings.speech_ai.automatic') },
    ...catalogModels
      .filter(
        (model) =>
          model.type === 'punctuation' &&
          (installedModels.has(model.id) || model.id === punctuationValue)
      )
      .map((model) => ({ value: model.id, label: model.name })),
  ];

  const summary = summarizeInstalled(catalogModels, installedModels);
  const downloadedValue =
    summary.count === 0
      ? t('settings.speech_ai.downloaded_none')
      : t('settings.speech_ai.downloaded_summary', {
          size: formatMegabytes(summary.megabytes),
          count: summary.count,
        });

  const liveSpeakers = speakersEnabled(config, 'live');
  const batchSpeakers = speakersEnabled(config, 'batch');
  const showSpeakerChildren =
    liveSpeakers || batchSpeakers || cloudEnabled(config, 'live') || cloudEnabled(config, 'batch');

  return (
    <>
      {offline ? (
        <div className="speech-ai-banner is-offline" role="status">
          <span>{t('settings.speech_ai.offline')}</span>
        </div>
      ) : null}
      {activeDownload?.status === 'failed' ? (
        <div className="speech-ai-banner is-err" role="alert">
          <span>
            {t('settings.speech_ai.err_download', {
              model: downloadName,
              reason: activeDownload.error || t('settings.speech_ai.err_network'),
            })}
          </span>
          <button type="button" className="btn btn-secondary" onClick={startRecommended}>
            {t('settings.speech_ai.retry')}
          </button>
        </div>
      ) : null}
      {!ready && activeDownload?.status === 'downloading' ? (
        <SetupProgress
          title={downloadName}
          progress={activeDownload.progress ?? 0}
          label={t('settings.speech_ai.setup_progress', {
            done: formatByteLabel(activeDownload.downloadedBytes) || '0 MB',
            total:
              formatByteLabel(activeDownload.totalBytes) || displaySize(recommended?.size) || '—',
            speed: activeDownload.speed || '',
          })}
          onCancel={() => handleCancelDownload(activeDownload.modelId || recommendedId)}
        />
      ) : null}
      {!ready && activeDownload?.status !== 'downloading' && activeDownload?.status !== 'failed' ? (
        <div className="speech-ai-setup">
          <span className="speech-ai-setup-mark" aria-hidden="true">
            <Mic size={18} />
          </span>
          <span className="speech-ai-setup-copy">
            <strong>{t('settings.speech_ai.setup_title')}</strong>
            <span>
              {t('settings.speech_ai.setup_body', {
                model: recommendedName,
                langs: t('settings.speech_ai.recommended_langs'),
                size: displaySize(recommended?.size),
              })}
            </span>
          </span>
          <button type="button" className="btn btn-primary" onClick={startRecommended}>
            {t('settings.speech_ai.setup_cta')}
          </button>
        </div>
      ) : null}

      <SectionLabel>{t('settings.speech_ai.sec_speech')}</SectionLabel>
      <div className="speech-ai-card">
        <DrillRow
          id="settings-speech-live"
          title={t('settings.speech_ai.live')}
          hint={t('settings.speech_ai.live_hint')}
          value={
            liveValue ||
            (liveTone === 'busy'
              ? t('settings.speech_ai.downloading')
              : t('settings.speech_ai.not_set_up'))
          }
          tone={liveTone}
          onClick={() => onOpen('live')}
        />
        <DrillRow
          id="settings-speech-batch"
          title={t('settings.speech_ai.batch')}
          hint={t('settings.speech_ai.batch_hint')}
          value={
            batchValue ||
            (batchTone === 'busy'
              ? t('settings.speech_ai.downloading')
              : t('settings.speech_ai.not_set_up'))
          }
          tone={batchTone}
          onClick={() => onOpen('batch')}
        />
        <SpeakerBlock scenario="live" onOpen={onOpen} />
        <SpeakerBlock scenario="batch" onOpen={onOpen} />
        {showSpeakerChildren ? <SpeakerShared /> : null}
        <SelectRow
          id="settings-punctuation-path"
          controlId="settings-punctuation-dropdown"
          title={t('settings.speech_ai.add_punctuation')}
          hint={t('settings.speech_ai.add_punctuation_hint')}
          value={punctuationValue}
          options={punctuationOptions}
          onChange={(modelId) => {
            if (!modelId) {
              updateConfig({ livePunctuationModelPath: '', batchPunctuationModelPath: '' });
              return;
            }
            const path = modelCatalog.modelPathById[modelId] ?? '';
            if (!path) return;
            updateConfig({ livePunctuationModelPath: path, batchPunctuationModelPath: path });
          }}
        />
        <SelectRow
          id="settings-vad-path"
          controlId="settings-batch-vad"
          title={t('settings.speech_ai.detect_pauses')}
          hint={t('settings.speech_ai.detect_pauses_hint')}
          value={transcription.batchVadEnabled === false ? 'off' : 'automatic'}
          options={[
            { value: 'automatic', label: t('settings.speech_ai.automatic') },
            { value: 'off', label: t('settings.speech_ai.off') },
          ]}
          onChange={(value) => updateConfig({ batchVadEnabled: value !== 'off' })}
        />
        <SwitchRow
          title={t('settings.speech_ai.low_power')}
          hint={t('settings.speech_ai.low_power_hint')}
          checked={Boolean(transcription.liveSentenceOnly)}
          onChange={(checked) => updateConfig({ liveSentenceOnly: checked })}
        />
        <CtcRow scenario="live" onOpen={onOpen} />
        <CtcRow scenario="batch" onOpen={onOpen} />
      </div>

      <SectionLabel>{t('settings.speech_ai.sec_ai')}</SectionLabel>
      <AiBlock offline={offline} onOpen={() => onOpen('ai')} />

      <SectionLabel>{t('settings.speech_ai.sec_adv')}</SectionLabel>
      <div className="speech-ai-card">
        <DrillRow
          id="settings-speech-downloads"
          title={t('settings.speech_ai.downloaded')}
          hint={t('settings.speech_ai.downloaded_hint')}
          value={downloadedValue}
          onClick={() => onOpen('downloaded')}
        />
        <MirrorRow />
        <GpuRow />
      </div>
    </>
  );
}

function SpeakerBlock({
  scenario,
  onOpen,
}: {
  scenario: 'live' | 'batch';
  onOpen: (scope: ModelPageScope) => void;
}): React.JSX.Element | null {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const { modelCatalog } = useModelManagerContext();
  const selection = config.asr?.selections[scenario];
  const online = selection?.engine === 'online';
  const providerId = selection?.providerId;
  const cloud = online && providerSupportsSpeakerDiarization(providerId);
  if (online && !cloud) return null;

  const enabled = cloud ? cloudSpeakerOn(config, providerId) : speakersEnabled(config, scenario);
  const degraded =
    !cloud &&
    enabled &&
    isDiarizationDegraded(config, scenario, { requireSegmentation: scenario === 'batch' });
  const title =
    scenario === 'live'
      ? t('settings.speech_ai.speakers_live')
      : t('settings.speech_ai.speakers_batch');

  const toggle = (checked: boolean) => {
    if (cloud && providerId) {
      updateConfig(
        syncOnlineAsrProviderConfig(config, providerId, { speakerDiarization: checked })
      );
      return;
    }
    updateConfig(
      scenario === 'batch'
        ? { batchSpeakerDiarizationEnabled: checked }
        : { liveSpeakerDiarizationEnabled: checked }
    );
  };

  return (
    <>
      <SwitchRow
        id={scenario === 'live' ? 'settings-speech-speakers' : 'settings-speech-speakers-batch'}
        title={title}
        hint={
          degraded
            ? t('settings.speech_ai.speakers_degraded')
            : t('settings.speech_ai.speakers_hint')
        }
        hintTone={degraded ? 'err' : undefined}
        checked={enabled}
        onChange={toggle}
      />
      {enabled ? (
        <>
          <DrillRow
            indent
            title={t('settings.speech_ai.embedding')}
            hint={t('settings.speech_ai.embedding_hint')}
            value={companionLabel(
              config,
              'speakerEmbeddingModelPath',
              scenario,
              modelCatalog.models,
              t
            )}
            onClick={() => onOpen(scenario === 'live' ? 'embedding-live' : 'embedding-batch')}
          />
          {cloud ? null : (
            <DrillRow
              indent
              title={t('settings.speech_ai.segmentation')}
              hint={
                scenario === 'live'
                  ? t('settings.speech_ai.segmentation_live_hint')
                  : t('settings.speech_ai.segmentation_hint')
              }
              value={companionLabel(
                config,
                'speakerSegmentationModelPath',
                scenario,
                modelCatalog.models,
                t
              )}
              onClick={() =>
                onOpen(scenario === 'live' ? 'segmentation-live' : 'segmentation-batch')
              }
            />
          )}
        </>
      ) : null}
    </>
  );
}

function SpeakerShared(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const count =
    config.speakerExpectedCount && config.speakerExpectedCount >= 2
      ? String(config.speakerExpectedCount)
      : 'auto';
  const countOptions = [
    { value: 'auto', label: t('settings.speech_ai.automatic') },
    ...Array.from({ length: 9 }, (_, index) => {
      const value = String(index + 2);
      return { value, label: value };
    }),
  ];
  return (
    <>
      <SelectRow
        indent
        title={t('settings.speech_ai.count')}
        hint={t('settings.speech_ai.count_hint')}
        value={count}
        options={countOptions}
        onChange={(value) =>
          updateConfig({ speakerExpectedCount: value === 'auto' ? null : Number(value) })
        }
      />
      <SelectRow
        indent
        title={t('settings.speech_ai.strictness')}
        hint={t('settings.speech_ai.strict_hint')}
        value={config.speakerDiarizationSensitivity ?? 'balanced'}
        options={[
          { value: 'permissive', label: t('settings.speech_ai.permissive') },
          { value: 'balanced', label: t('settings.speech_ai.balanced') },
          { value: 'strict', label: t('settings.speech_ai.strict') },
        ]}
        onChange={(value) =>
          updateConfig({
            speakerDiarizationSensitivity: value as 'permissive' | 'balanced' | 'strict',
          })
        }
      />
    </>
  );
}

function CtcRow({
  scenario,
  onOpen,
}: {
  scenario: 'live' | 'batch';
  onOpen: (scope: ModelPageScope) => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const { modelCatalog } = useModelManagerContext();
  return (
    <DrillRow
      id={scenario === 'live' ? 'settings-alignment-path' : 'settings-alignment-batch'}
      title={
        scenario === 'live' ? t('settings.speech_ai.ctc_live') : t('settings.speech_ai.ctc_batch')
      }
      hint={t('settings.speech_ai.ctc_hint')}
      value={companionLabel(config, 'alignmentModelPath', scenario, modelCatalog.models, t)}
      onClick={() => onOpen(scenario === 'live' ? 'alignment-live' : 'alignment-batch')}
    />
  );
}

function AiBlock({ offline, onOpen }: { offline: boolean; onOpen: () => void }): React.JSX.Element {
  const { t } = useTranslation();
  const llmConfig = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const settings = useMemo(() => getCurrentLlmSettings(llmConfig), [llmConfig]);
  const polish = getFeatureModelEntry({ llmSettings: settings }, 'polish');
  const summary = getFeatureModelEntry({ llmSettings: settings }, 'summary');
  const translation = getFeatureModelEntry({ llmSettings: settings }, 'translation');
  const writingModels = getOrderedLlmModels(settings).filter(
    (entry) => !isGoogleProvider(entry.provider)
  );
  const polishId =
    polish && !isGoogleProvider(polish.provider)
      ? polish.id
      : summary && !isGoogleProvider(summary.provider)
        ? summary.id
        : '';
  const probe = useLlmProbe(
    polish && !isGoogleProvider(polish.provider) ? polish.provider : null,
    polish?.model,
    Boolean(polish && !isGoogleProvider(polish.provider))
  );
  const service = serviceValue(polish, probe, writingModels.length > 0, t);
  const translationValue =
    translation && !isGoogleProvider(translation.provider)
      ? translation.id
      : GOOGLE_TRANSLATE_VALUE;

  const modelOptions = writingModels.map((entry) => ({
    value: entry.id,
    label: `${providerLabel(entry.provider, settings, t)} · ${entry.model}`,
  }));

  return (
    <div className="speech-ai-card" id="settings-llm-models-config">
      <DrillRow
        id="settings-speech-ai-services"
        title={t('settings.speech_ai.ai_service')}
        hint={t('settings.speech_ai.ai_service_hint')}
        value={service.label}
        tone={service.tone}
        onClick={onOpen}
      />
      <SelectRow
        title={t('settings.speech_ai.polish')}
        hint={
          modelOptions.length === 0
            ? t('settings.speech_ai.polish_disabled')
            : t('settings.speech_ai.polish_hint')
        }
        value={polishId}
        disabled={modelOptions.length === 0}
        options={modelOptions.length === 0 ? [{ value: '', label: '—' }] : modelOptions}
        onChange={(modelId) => {
          if (!modelId) return;
          updateConfig(buildLlmConfigPatch(setPolishAndSummary(settings, modelId)));
        }}
      />
      <SelectRow
        title={t('settings.speech_ai.translate')}
        hint={
          offline && translationValue === GOOGLE_TRANSLATE_VALUE
            ? t('settings.speech_ai.translate_offline')
            : t('settings.speech_ai.translate_google_hint')
        }
        value={translationValue}
        options={[
          {
            value: GOOGLE_TRANSLATE_VALUE,
            label: t('settings.speech_ai.translate_google'),
            description: offline ? t('settings.speech_ai.translate_offline') : undefined,
            disabled: offline,
          },
          ...modelOptions,
        ]}
        onChange={(value) => {
          updateConfig(buildLlmConfigPatch(setTranslationChoice(settings, value)));
        }}
      />
    </div>
  );
}

function MirrorRow(): React.JSX.Element {
  const { t } = useTranslation();
  const mirror = useConfigStore((state) => state.config.modelDownloadMirror || 'auto');
  const updateConfig = useSetConfig();
  return (
    <SelectRow
      title={t('settings.speech_ai.mirror')}
      hint={t('settings.speech_ai.mirror_hint')}
      controlId="settings-download-mirror"
      value={mirror}
      onChange={(value) => updateConfig({ modelDownloadMirror: value })}
      options={[
        { value: 'auto', label: t('settings.model_download_mirror_auto') },
        { value: 'direct', label: t('settings.model_download_mirror_direct') },
        { value: 'ghproxy', label: t('settings.model_download_mirror_ghproxy') },
        { value: 'ghnet', label: t('settings.model_download_mirror_ghnet') },
        { value: 'hf-mirror', label: t('settings.model_download_mirror_hfmirror') },
      ]}
    />
  );
}

function GpuRow(): React.JSX.Element {
  const { t } = useTranslation();
  const gpu = useTranscriptionConfig().gpuAcceleration ?? 'auto';
  const updateConfig = useSetConfig();
  const [cudaDownloading, setCudaDownloading] = useState(false);
  const [cudaProgress, setCudaProgress] = useState(0);
  const [cudaStatus, setCudaStatus] = useState<CudaAddonInspection | null>(null);

  const onGpu = async (value: string) => {
    if (value === 'cuda') {
      const status = cudaStatus ?? (await cudaAddonService.getStatus().catch(() => null));
      setCudaStatus(status);
      if (!status?.isInstalled) {
        if (!status) return;
        const confirmed = await useDialogStore
          .getState()
          .confirm(t('settings.cuda_addon_download_confirm'), {
            title: t('settings.cuda_addon_download_title'),
            confirmLabel: t('common.download'),
          });
        if (!confirmed) return;
        setCudaDownloading(true);
        setCudaProgress(0);
        try {
          const next = await cudaAddonService.downloadAndInstall({
            onProgress: (progress) => setCudaProgress(progress.progressPercent),
          });
          setCudaStatus(next);
          updateConfig({ gpuAcceleration: 'cuda' });
        } catch (error) {
          useDialogStore.getState().alert(String(error), {
            title: t('settings.cuda_addon_download_title'),
            variant: 'error',
          });
        } finally {
          setCudaDownloading(false);
        }
        return;
      }
    }
    updateConfig({ gpuAcceleration: value as 'auto' | 'cpu' | 'vulkan' | 'metal' | 'cuda' });
  };

  return (
    <SelectRow
      id="settings-gpu-acceleration"
      controlId="settings-gpu-acceleration-dropdown"
      title={t('settings.gpu_acceleration_label')}
      hint={
        cudaDownloading
          ? t('settings.cuda_addon_downloading', { percent: cudaProgress })
          : t('settings.gpu_acceleration_hint')
      }
      value={gpu}
      disabled={cudaDownloading}
      onChange={(value) => void onGpu(value)}
      options={[
        { value: 'auto', label: t('settings.gpu_acceleration_auto') },
        { value: 'cpu', label: t('settings.value_off') },
        { value: 'vulkan', label: 'Vulkan' },
        { value: 'metal', label: 'Metal' },
        { value: 'cuda', label: 'CUDA' },
      ]}
    />
  );
}

function SetupProgress({
  title,
  progress,
  label,
  onCancel,
}: {
  title: string;
  progress: number;
  label: string;
  onCancel: () => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  return (
    <div className="speech-ai-setup is-progress">
      <span className="speech-ai-setup-copy">
        <strong>{title}</strong>
        <span>{label}</span>
        <span
          className="speech-ai-progress"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progress)}
        >
          <span style={{ width: `${Math.max(0, Math.min(100, progress))}%` }} />
        </span>
      </span>
      <button type="button" className="btn btn-secondary" onClick={onCancel}>
        {t('common.cancel')}
      </button>
    </div>
  );
}

function companionLabel(
  config: Parameters<typeof getScenarioModelPath>[0],
  kind: Parameters<typeof getScenarioModelPath>[1],
  scenario: 'live' | 'batch',
  models: Array<ModelInfo & { installPath?: string }>,
  t: (key: string) => string
): string {
  const match = modelForPath(getScenarioModelPath(config, kind, scenario), models);
  if (!match) return t('settings.speech_ai.off');
  return match.versionLabel ? `${match.name} · ${match.versionLabel}` : match.name;
}

function punctuationSelection(
  config: { livePunctuationModelPath?: string },
  models: Array<ModelInfo & { installPath?: string }>
): string {
  return modelForPath(config.livePunctuationModelPath, models)?.id ?? '';
}

function cloudEnabled(config: AppConfig, scenario: 'live' | 'batch'): boolean {
  const selection = config.asr?.selections[scenario];
  if (selection?.engine !== 'online' || !providerSupportsSpeakerDiarization(selection.providerId)) {
    return false;
  }
  return cloudSpeakerOn(config, selection.providerId);
}

function cloudSpeakerOn(config: AppConfig, providerId: string | null | undefined): boolean {
  if (!providerId) return false;
  const providerConfig = getOnlineProviderConfig(config.asr?.providers, providerId);
  const definition = getOnlineAsrProviderDefinition(providerId);
  return (
    (providerConfig?.speakerDiarization as boolean | undefined) ??
    definition?.defaultConfig?.speakerDiarization !== false
  );
}

function toneFor(
  download: { modelId?: string; status?: string } | undefined,
  scenario: 'live' | 'batch',
  config: {
    asr?: {
      selections: { live?: { modelId?: string | null }; batch?: { modelId?: string | null } };
    };
  },
  ready: boolean
): 'busy' | 'err' | undefined {
  if (!download || (download.status !== 'downloading' && download.status !== 'failed')) {
    return undefined;
  }
  const selected = config.asr?.selections[scenario]?.modelId;
  const matches = !selected || selected === download.modelId || !ready;
  if (!matches) return undefined;
  return download.status === 'downloading' ? 'busy' : 'err';
}

function serviceValue(
  polish: { provider: LlmProvider; model: string } | null,
  probe: { busy: boolean; ok: boolean },
  hasService: boolean,
  t: (key: string, options?: Record<string, unknown>) => string
): { label: string; tone?: 'ok' | 'err' | 'busy' } {
  if (!polish || isGoogleProvider(polish.provider)) {
    return hasService
      ? { label: t('settings.speech_ai.ai_none'), tone: undefined }
      : { label: t('settings.speech_ai.ai_none') };
  }
  const local = isLocalLlmProvider(polish.provider);
  if (probe.busy) return { label: t('settings.speech_ai.ai_checking'), tone: 'busy' };
  if (!probe.ok) {
    return {
      label: local
        ? t('settings.speech_ai.ai_not_running')
        : t('settings.speech_ai.ai_unreachable'),
      tone: 'err',
    };
  }
  const name = t(getProviderDefinition(polish.provider).labelKey);
  return {
    label: local ? t('settings.speech_ai.ai_local_value', { name }) : name,
    tone: 'ok',
  };
}

function providerLabel(
  provider: LlmProvider,
  settings: ReturnType<typeof getCurrentLlmSettings>,
  t: (key: string) => string
): string {
  const custom = settings.customProviders?.[provider as `custom-${string}`]?.name;
  if (custom) return custom;
  return t(getProviderDefinition(provider, settings.customProviders).labelKey);
}
