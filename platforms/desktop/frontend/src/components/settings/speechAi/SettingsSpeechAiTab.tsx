import {
  Box,
  ChevronLeft,
  Globe,
  HardDrive,
  Mic,
  PenLine,
  Plug,
  Users,
  WifiOff,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { buildModelSelectionPatch } from '../../../services/asrConfigService';
import { buildLlmConfig, getProviderDefinition } from '../../../services/llm/providers';
import { getFeatureModelEntry } from '../../../services/llm/state';
import { generateLlmText } from '../../../services/tauri/llm';
import { useConfigStore, useLlmAssistantConfig, useSetConfig } from '../../../stores/configStore';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import type { ModelInfo } from '../../../types/modelCatalog';
import { PRESET_MODELS } from '../../../types/modelCatalog';
import { MODAL_OVERLAY_SELECTOR } from '../../../utils/focusUtils';
import { getCurrentLlmSettings } from '../llm/helpers';
import { SettingsPageHeader, SettingsTabContainer } from '../SettingsLayout';
import { SettingsLLMServiceTab } from '../SettingsLLMServiceTab';
import { CapabilityRow, MoreOptionButton } from './CapabilityRow';
import { ConnectAiDialog } from './ConnectAiDialog';
import { DownloadsPanel } from './DownloadsPanel';
import { formatCheckedAgo, probeIsFresh, readLlmProbe, writeLlmProbe } from './llmProbe';
import { RecognitionPanel } from './RecognitionPanel';
import { SpeakersPanel } from './SpeakersPanel';
import { SpeechModelPicker } from './SpeechModelPicker';
import './speech-ai.css';
import {
  buildSpeechRow,
  familyTitle,
  groupAsrModels,
  pickSpeechDownload,
  presetModelById,
  recommendedVariantId,
  resolveModelTitle,
  type SpeechScope,
} from './speechAiLogic';

type Frame =
  | { name: 'home' }
  | { name: 'picker'; scope: SpeechScope }
  | { name: 'recognition' }
  | { name: 'speakers'; scope: SpeechScope }
  | { name: 'ai' }
  | { name: 'downloads' };

function frameFromEntry(view: string | undefined): Frame {
  switch (view) {
    case 'recognition':
      return { name: 'recognition' };
    case 'speakers':
      return { name: 'speakers', scope: 'both' };
    case 'ai':
      return { name: 'ai' };
    case 'downloads':
      return { name: 'downloads' };
    case 'picker':
      return { name: 'picker', scope: 'both' };
    default:
      return { name: 'home' };
  }
}

export function SettingsSpeechAiTab({
  speechEntry,
}: {
  speechEntry?: { view: string; requestId: number } | null;
}): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const llmConfig = useLlmAssistantConfig();
  const {
    installedModels,
    modelCatalog,
    downloads,
    handleDownload,
    handleCancelDownload,
    handleDelete,
    handleDiscardDownload,
    catalogLoadState,
  } = useModelManagerContext();
  const downloadItems = useModelDownloadStore((state) => state.downloads);
  const [stack, setStack] = useState<Frame[]>(() => [frameFromEntry(speechEntry?.view)]);
  const [connectOpen, setConnectOpen] = useState(false);
  const [offline, setOffline] = useState(
    typeof navigator !== 'undefined' ? navigator.onLine === false : false
  );
  const [probeRequest, setProbeRequest] = useState(0);
  const [probeRevision, setProbeRevision] = useState(0);
  const [probing, setProbing] = useState(false);
  const seenEntry = useRef(speechEntry?.requestId);

  if (speechEntry && speechEntry.requestId !== seenEntry.current) {
    seenEntry.current = speechEntry.requestId;
    setStack([frameFromEntry(speechEntry.view)]);
  }

  const frame = stack[stack.length - 1] ?? { name: 'home' };
  const push = (next: Frame) => setStack((current) => [...current, next]);
  const pop = () =>
    setStack((current) => {
      if (current.length > 1) return current.slice(0, -1);
      if (current[0]?.name !== 'home') return [{ name: 'home' }];
      return current;
    });

  useEffect(() => {
    const on = () => setOffline(false);
    const off = () => setOffline(true);
    window.addEventListener('online', on);
    window.addEventListener('offline', off);
    return () => {
      window.removeEventListener('online', on);
      window.removeEventListener('offline', off);
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      const overlays = document.querySelectorAll(MODAL_OVERLAY_SELECTOR);
      const top = overlays[overlays.length - 1];
      if (top && !top.classList.contains('settings-overlay')) return;
      if (connectOpen) {
        event.preventDefault();
        event.stopImmediatePropagation();
        setConnectOpen(false);
        return;
      }
      if (frame.name === 'home') return;
      event.preventDefault();
      event.stopImmediatePropagation();
      setStack((current) => {
        if (current.length > 1) return current.slice(0, -1);
        if (current[0]?.name !== 'home') return [{ name: 'home' }];
        return current;
      });
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [connectOpen, frame.name]);

  const recommendedId = recommendedVariantId(i18n.resolvedLanguage || i18n.language);
  const catalogModels = modelCatalog.models.length > 0 ? modelCatalog.models : PRESET_MODELS;
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
  const activeDownload = speechDownload
    ? speechDownload
    : managerDownload
      ? {
          modelId: recommendedId,
          status: 'downloading',
          progress: managerDownload.progress,
        }
      : undefined;
  const activeDownloadModel =
    presetModelById(activeDownload?.modelId ?? '') ??
    catalogModels.find((model) => model.id === activeDownload?.modelId);
  const speech = buildSpeechRow({
    config,
    catalogModels,
    installedIds: installedModels,
    recommendedId,
    downloadName: activeDownloadModel ? familyTitle(activeDownloadModel, t) : undefined,
    offline,
    t,
    download: activeDownload,
  });

  const polish = getFeatureModelEntry(llmConfig, 'polish');
  const translation = getFeatureModelEntry(llmConfig, 'translation');
  const probeRecord = readLlmProbe();
  const probeMatches =
    Boolean(polish) &&
    probeRecord?.ok &&
    probeRecord.provider === polish?.provider &&
    probeRecord.model === polish?.model;

  useEffect(() => {
    if (!polish) return;
    if (polish.provider === 'google_translate' || polish.provider === 'google_translate_free') {
      return;
    }
    if (probeRequest === 0) {
      const existing = readLlmProbe();
      if (
        existing &&
        existing.provider === polish.provider &&
        existing.model === polish.model &&
        probeIsFresh(existing)
      ) {
        return;
      }
    }
    const current = getCurrentLlmSettings(llmConfig);
    const setting = current.providers[polish.provider];
    if (!setting) return;
    const definition = getProviderDefinition(polish.provider, current.customProviders);
    if (definition.requiresApiKey && !setting.apiKey) return;
    let cancelled = false;
    setProbing(true);
    const started = Date.now();
    const providerConfig = buildLlmConfig(polish.provider, setting, current.customProviders);
    generateLlmText({
      config: { ...providerConfig, model: polish.model },
      input: 'Hi',
      source: 'connection_test',
    })
      .then(() => {
        if (cancelled) return;
        writeLlmProbe({
          provider: polish.provider,
          model: polish.model,
          ok: true,
          at: Date.now(),
          latencyMs: Date.now() - started,
        });
        setProbeRevision((value) => value + 1);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        writeLlmProbe({
          provider: polish.provider,
          model: polish.model,
          ok: false,
          at: Date.now(),
          error: error instanceof Error ? error.message : String(error),
        });
        setProbeRevision((value) => value + 1);
      })
      .finally(() => {
        if (!cancelled) setProbing(false);
      });
    return () => {
      cancelled = true;
    };
  }, [llmConfig, polish, probeRequest]);

  const writing = useMemo(() => {
    void probeRevision;
    const record = readLlmProbe();
    if (
      !polish ||
      polish.provider === 'google_translate' ||
      polish.provider === 'google_translate_free'
    ) {
      return {
        status: 'idle' as const,
        label: t('settings.speech_ai.not_connected'),
        detail: t('settings.speech_ai.writing_blurb'),
        action: 'connect' as const,
        value: '',
      };
    }
    const definition = getProviderDefinition(polish.provider);
    const name = t(definition.labelKey);
    const matched =
      record?.provider === polish.provider && record.model === polish.model ? record : null;
    if (matched?.ok) {
      return {
        status: 'ok' as const,
        label: t('settings.speech_ai.connected'),
        detail: t('settings.speech_ai.writing_connected', {
          provider: name,
          model: polish.model,
          when: formatCheckedAgo(matched.at, t),
        }),
        action: 'manage' as const,
        value: name,
      };
    }
    if (
      matched &&
      !matched.ok &&
      (polish.provider === 'ollama' || polish.provider === 'lm_studio')
    ) {
      return {
        status: 'warn' as const,
        label: t('settings.speech_ai.cant_reach', {
          name,
        }),
        detail: matched.error || t('settings.speech_ai.local_empty_ollama'),
        action: 'retry' as const,
        value: '',
      };
    }
    return {
      status: 'idle' as const,
      label: t('settings.speech_ai.not_connected'),
      detail: t('settings.speech_ai.writing_blurb'),
      action: 'connect' as const,
      value: '',
    };
  }, [polish, probeRevision, t]);

  const translationUsesAi =
    translation &&
    translation.provider !== 'google_translate' &&
    translation.provider !== 'google_translate_free';
  const translationRow = (() => {
    if (offline && !translationUsesAi) {
      return {
        status: 'warn' as const,
        label: t('settings.speech_ai.unavailable_offline'),
        detail: t('settings.speech_ai.translation_offline'),
        value: t('settings.speech_ai.translation_value_google'),
      };
    }
    if (translationUsesAi) {
      const definition = getProviderDefinition(translation.provider);
      const name = t(definition.labelKey);
      if (probeMatches) {
        return {
          status: 'ok' as const,
          label: t('settings.speech_ai.ready'),
          detail: t('settings.speech_ai.translation_using_ai', {
            provider: name,
            model: translation.model,
          }),
          value: name,
        };
      }
      if (probing) {
        return {
          status: 'busy' as const,
          label: t('settings.speech_ai.checking'),
          detail: t('settings.speech_ai.translation_checking', {
            provider: name,
          }),
          value: name,
        };
      }
      return {
        status: 'idle' as const,
        label: t('settings.speech_ai.not_connected'),
        detail: t('settings.speech_ai.translation_not_connected', {
          provider: name,
          model: translation.model,
        }),
        value: name,
      };
    }
    return {
      status: 'ok' as const,
      label: t('settings.speech_ai.ready'),
      detail: t('settings.speech_ai.translation_google_detail'),
      value: t('settings.speech_ai.translation_value_google'),
    };
  })();

  const applyModel = (model: ModelInfo, scope: SpeechScope) => {
    const livePatch =
      scope === 'batch' ? {} : buildModelSelectionPatch(config, 'live', model.id, modelCatalog);
    const batchPatch =
      scope === 'live'
        ? {}
        : buildModelSelectionPatch({ ...config, ...livePatch }, 'batch', model.id, modelCatalog);
    updateConfig({
      ...livePatch,
      ...batchPatch,
      ...(scope === 'both' ? { speechModesLinked: true } : {}),
    });
    pop();
  };

  const startRecommended = () => {
    const model = presetModelById(recommendedId);
    if (!model) return;
    if (installedModels.has(model.id)) {
      applyModel(model, 'both');
      setStack([{ name: 'home' }]);
      return;
    }
    void handleDownload(model);
  };

  const downloadForPicker = (model: ModelInfo) => {
    const scope = frame.name === 'picker' ? frame.scope : 'both';
    const snapshot = {
      streamingModelPath: config.streamingModelPath,
      batchModelPath: config.batchModelPath,
      asr: config.asr,
    };
    void handleDownload(model).then(() => {
      const current = useConfigStore.getState().config;
      if (scope === 'both') {
        updateConfig({ speechModesLinked: true });
        return;
      }
      if (!current.asr || !snapshot.asr) return;
      if (scope === 'live') {
        updateConfig({
          batchModelPath: snapshot.batchModelPath,
          asr: {
            ...current.asr,
            selections: {
              ...current.asr.selections,
              batch: snapshot.asr.selections.batch,
            },
          },
        });
        return;
      }
      updateConfig({
        streamingModelPath: snapshot.streamingModelPath,
        asr: {
          ...current.asr,
          selections: {
            ...current.asr.selections,
            live: snapshot.asr.selections.live,
            caption: snapshot.asr.selections.caption,
            voiceTyping: snapshot.asr.selections.voiceTyping,
          },
        },
      });
    });
  };

  const groups = useMemo(() => groupAsrModels(catalogModels), [catalogModels]);
  const drillTitle =
    frame.name === 'picker'
      ? t('settings.speech_ai.picker_title')
      : frame.name === 'recognition'
        ? t('settings.speech_ai.recognition')
        : frame.name === 'speakers'
          ? t('settings.speech_ai.speakers')
          : frame.name === 'ai'
            ? t('settings.speech_ai.ai_services')
            : frame.name === 'downloads'
              ? t('settings.speech_ai.downloads')
              : '';

  return (
    <SettingsTabContainer id="settings-panel-models" ariaLabelledby="settings-tab-models">
      {frame.name === 'home' ? (
        <SettingsPageHeader
          id="settings-speech-ai-title"
          icon={<Box size={28} />}
          title={t('settings.speech_ai.title')}
          description={t('settings.speech_ai.description')}
        />
      ) : null}
      <div className="speech-ai" id="settings-model-hub">
        {frame.name !== 'home' ? (
          <div className="speech-ai-sticky">
            <button type="button" className="speech-ai-back" onClick={pop}>
              <ChevronLeft size={16} />
              {t('settings.speech_ai.title')}
            </button>
            <h2 className="settings-page-title">{drillTitle}</h2>
          </div>
        ) : null}

        {frame.name === 'home' ? (
          <>
            {offline ? (
              <div className="speech-ai-banner" role="status">
                <WifiOff size={16} />
                <span>{t('settings.speech_ai.offline_banner')}</span>
              </div>
            ) : null}
            <div className="speech-ai-card">
              <CapabilityRow
                icon={<Mic size={18} />}
                title={t('settings.speech_ai.speech_title')}
                status={speech.status}
                statusLabel={speech.statusLabel}
                detail={speech.detail}
                lines={speech.lines}
                progress={speech.progress}
                progressLabel={speech.progressLabel}
                value={
                  speech.action === 'change'
                    ? resolveModelTitle(config, 'live', catalogModels, t) ||
                      t('settings.speech_ai.not_set_up')
                    : speech.action === 'manage'
                      ? t('settings.speech_ai.currently_different')
                      : undefined
                }
                onActivate={() =>
                  speech.action === 'manage'
                    ? push({ name: 'recognition' })
                    : push({ name: 'picker', scope: 'both' })
                }
                action={
                  speech.action === 'download' ||
                  speech.action === 'use' ||
                  speech.action === 'retry' ? (
                    <button type="button" className="btn btn-primary" onClick={startRecommended}>
                      {speech.action === 'retry'
                        ? t('settings.speech_ai.try_again')
                        : speech.action === 'use'
                          ? t('settings.speech_ai.use_recommended')
                          : t('settings.speech_ai.download_recommended')}
                    </button>
                  ) : speech.action === 'cancel' ? (
                    <button
                      type="button"
                      className="btn btn-text"
                      onClick={() => handleCancelDownload(activeDownload?.modelId || recommendedId)}
                    >
                      {t('common.cancel')}
                    </button>
                  ) : undefined
                }
              />
              <CapabilityRow
                icon={<PenLine size={18} />}
                title={t('settings.speech_ai.writing_title')}
                status={writing.status}
                statusLabel={writing.label}
                detail={writing.detail}
                value={writing.action === 'manage' ? writing.value : undefined}
                onActivate={writing.action === 'manage' ? () => push({ name: 'ai' }) : undefined}
                action={
                  writing.action === 'retry' ? (
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => setProbeRequest((value) => value + 1)}
                    >
                      {t('settings.speech_ai.check_again')}
                    </button>
                  ) : writing.action === 'connect' ? (
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => setConnectOpen(true)}
                    >
                      {t('settings.speech_ai.connect')}
                    </button>
                  ) : undefined
                }
              />
              <CapabilityRow
                icon={<Globe size={18} />}
                title={t('settings.speech_ai.translation_title')}
                status={translationRow.status}
                statusLabel={translationRow.label}
                detail={translationRow.detail}
                value={translationRow.value}
                onActivate={() => push({ name: 'ai' })}
              />
            </div>
            <div className="speech-ai-section-label">{t('settings.speech_ai.more_options')}</div>
            <div className="speech-ai-card">
              <MoreOptionButton
                id="settings-speech-recognition-link"
                icon={<Mic size={16} />}
                title={t('settings.speech_ai.recognition')}
                hint={t('settings.speech_ai.recognition_hint')}
                onClick={() => push({ name: 'recognition' })}
              />
              <MoreOptionButton
                id="settings-speech-speakers"
                icon={<Users size={16} />}
                title={t('settings.speech_ai.speakers')}
                hint={t('settings.speech_ai.speakers_hint')}
                onClick={() => push({ name: 'speakers', scope: 'both' })}
              />
              <MoreOptionButton
                id="settings-speech-ai-services"
                icon={<Plug size={16} />}
                title={t('settings.speech_ai.ai_services')}
                hint={t('settings.speech_ai.ai_services_hint')}
                onClick={() => push({ name: 'ai' })}
              />
              <MoreOptionButton
                id="settings-speech-downloads"
                icon={<HardDrive size={16} />}
                title={t('settings.speech_ai.downloads')}
                hint={t('settings.speech_ai.downloads_hint')}
                onClick={() => push({ name: 'downloads' })}
              />
            </div>
          </>
        ) : null}

        {frame.name === 'picker' ? (
          <SpeechModelPicker
            groups={groups}
            scope={frame.scope}
            language={i18n.resolvedLanguage || i18n.language}
            installedModels={installedModels}
            downloads={downloads}
            actionsDisabled={catalogLoadState !== 'ready'}
            onDownload={downloadForPicker}
            onDelete={handleDelete}
            onCancelDownload={handleCancelDownload}
            onDiscardDownload={handleDiscardDownload}
          />
        ) : null}
        {frame.name === 'recognition' ? (
          <RecognitionPanel
            onChangeModel={(scope) => push({ name: 'picker', scope })}
            onConfigureSpeakers={(scope) => push({ name: 'speakers', scope })}
          />
        ) : null}
        {frame.name === 'speakers' ? (
          <SpeakersPanel scope={frame.scope} onChangeHelper={() => push({ name: 'downloads' })} />
        ) : null}
        {frame.name === 'ai' ? <SettingsLLMServiceTab embedded /> : null}
        {frame.name === 'downloads' ? <DownloadsPanel /> : null}
      </div>
      {connectOpen ? <ConnectAiDialog onClose={() => setConnectOpen(false)} /> : null}
    </SettingsTabContainer>
  );
}
