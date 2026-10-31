import {
  AlertCircle,
  BrainCircuit,
  Check,
  Cpu,
  FileText,
  Image,
  Languages,
  Loader2,
  Mic,
  Video,
  Zap,
} from 'lucide-react';
import type React from 'react';
import type { LocalLlmModelCard as LocalLlmModelCardType } from '../../../bindings';
import { formatBytes } from '../../../hooks/notifications/modelDownloadAdapter';
import { DownloadIcon, StopIcon, TrashIcon } from '../../Icons';
import { ModelBrandLogo } from '../../icons/ModelLogos';
import { LanguageBadges } from '../../LanguageBadges';
import './LocalModelCard.css';
export interface LocalDownloadProgressState {
  progress: number;
  statusText?: string;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: string;
}

export interface LocalModelTestState {
  status: 'idle' | 'loading' | 'success' | 'error';
  speed?: number;
  error?: string;
}

export interface LocalModelCardProps {
  card?: LocalLlmModelCardType;
  cards?: LocalLlmModelCardType[];
  downloadState?: LocalDownloadProgressState;
  downloads?: Record<string, LocalDownloadProgressState>;
  activeFeatures?: {
    polish?: boolean;
    translation?: boolean;
    summary?: boolean;
  };
  testState?: LocalModelTestState;
  modelTestStates?: Record<string, LocalModelTestState>;
  onTest?: (card: LocalLlmModelCardType) => void;
  isAnyTesting?: boolean;
  onDownload: (card: LocalLlmModelCardType) => void;
  onCancelDownload: (cardId: string) => void;
  onDelete: (card: LocalLlmModelCardType) => void;
  onDiscardDownload?: (card: LocalLlmModelCardType) => void;
  onApplyFeature?: (
    card: LocalLlmModelCardType,
    feature: 'polish' | 'translation' | 'summary' | 'all'
  ) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
  disabled?: boolean;
}

export function resolveFamilyName(card: LocalLlmModelCardType): string {
  if (card.groupId === 'index-translate') return 'Index-Translate';
  if (card.groupId === 'hunyuan-mt') return 'Hunyuan-MT';
  if (card.groupId === 'opus-mt') return 'Opus-MT';
  if (card.groupId === 'qwen3.5') return 'Qwen3.5';
  if (card.groupId === 'gemma-4') return 'Gemma 4';
  if (card.groupId === 'qwen3') return 'Qwen3';
  if (card.name.includes('(')) {
    return card.name.split('(')[0].trim();
  }
  return card.name;
}

export type SupportedModality = 'text' | 'image' | 'audio' | 'video';

const MODALITY_CONFIG: Record<
  SupportedModality,
  { labelKey: string; tooltipKey: string; label: string; icon: React.ReactNode }
> = {
  text: {
    labelKey: 'settings.llm.modality_text',
    tooltipKey: 'settings.llm.modality_text',
    label: '文本',
    icon: <FileText size={10} />,
  },
  image: {
    labelKey: 'settings.llm.modality_image',
    tooltipKey: 'settings.llm.modality_image',
    label: '图像',
    icon: <Image size={10} />,
  },
  audio: {
    labelKey: 'settings.llm.modality_audio',
    tooltipKey: 'settings.llm.modality_audio',
    label: '音频',
    icon: <Mic size={10} />,
  },
  video: {
    labelKey: 'settings.llm.modality_video',
    tooltipKey: 'settings.llm.modality_video',
    label: '视频',
    icon: <Video size={10} />,
  },
};

export function resolveModelModalities(card: LocalLlmModelCardType): SupportedModality[] {
  if (card.modalities && card.modalities.length > 0) {
    const valid: SupportedModality[] = [];
    for (const m of card.modalities) {
      const lower = m.toLowerCase();
      if (lower === 'text' && !valid.includes('text')) valid.push('text');
      else if ((lower === 'image' || lower === 'vision') && !valid.includes('image'))
        valid.push('image');
      else if (
        (lower === 'audio' || lower === 'speech' || lower === 'voice') &&
        !valid.includes('audio')
      )
        valid.push('audio');
      else if (lower === 'video' && !valid.includes('video')) valid.push('video');
    }
    if (valid.length > 0) return valid;
  }

  const text = `${card.id} ${card.name} ${card.model} ${card.filename}`.toLowerCase();
  const modalities: SupportedModality[] = ['text'];

  if (
    text.includes('vl') ||
    text.includes('vision') ||
    text.includes('image') ||
    text.includes('multimodal') ||
    card.id.includes('gemma')
  ) {
    modalities.push('image');
  }
  if (
    text.includes('audio') ||
    text.includes('voice') ||
    text.includes('omni') ||
    text.includes('speech')
  ) {
    modalities.push('audio');
  }
  if (text.includes('video')) {
    modalities.push('video');
  }

  return modalities;
}

export function LocalModelCard({
  card: cardProp,
  cards: cardsProp,
  downloadState: downloadStateProp,
  downloads,
  testState: testStateProp,
  modelTestStates,
  onTest,
  isAnyTesting = false,
  disabled = false,
  onDownload,
  onCancelDownload,
  onDelete,
  onDiscardDownload,
  t,
}: LocalModelCardProps): React.JSX.Element | null {
  const models = cardsProp && cardsProp.length > 0 ? cardsProp : cardProp ? [cardProp] : [];
  if (models.length === 0) return null;

  const isMultiVersion = models.length > 1;

  if (isMultiVersion) {
    const baseModel = models[0];
    const familyName = resolveFamilyName(baseModel);
    const allLanguages = Array.from(new Set(models.flatMap((m) => m.languages ?? [])));

    const downloadingModel = models.find((m) => downloads?.[m.id]);
    const downloadingState = downloadingModel ? downloads?.[downloadingModel.id] : undefined;
    const installedCount = models.filter((m) => m.isInstalled).length;
    const interruptedModel = models.find((m) => !m.isInstalled && m.interruptedDownload);

    const isQwen =
      baseModel.id.toLowerCase().includes('qwen') || familyName.toLowerCase().includes('qwen');
    const isGemma =
      baseModel.id.toLowerCase().includes('gemma') || familyName.toLowerCase().includes('gemma');
    const isTranslation =
      baseModel.groupId === 'opus-mt' ||
      baseModel.groupId === 'index-translate' ||
      baseModel.groupId === 'hunyuan-mt' ||
      baseModel.backend === 'onnx-nmt' ||
      (baseModel.capabilities?.length === 1 && baseModel.capabilities[0] === 'translate');

    return (
      <div
        className={`local-model-card model-card${installedCount === models.length ? ' is-installed' : ''}`}
        data-testid={`local-model-card-${baseModel.groupId || baseModel.id}`}
      >
        <div className="model-card-header local-model-card-header">
          <div className="model-card-identity local-model-card-identity">
            <div className="model-card-logo-badge">
              {isQwen ? (
                <ModelBrandLogo brand="qwen" size={36} />
              ) : isGemma ? (
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path
                    d="M12 2L14.4 9.6L22 12L14.4 14.4L12 22L9.6 14.4L2 12L9.6 9.6L12 2Z"
                    fill="url(#gemma-gradient)"
                  />
                  <defs>
                    <linearGradient
                      id="gemma-gradient"
                      x1="2"
                      y1="2"
                      x2="22"
                      y2="22"
                      gradientUnits="userSpaceOnUse"
                    >
                      <stop stopColor="#4285F4" />
                      <stop offset="0.5" stopColor="#9B72CB" />
                      <stop offset="1" stopColor="#EA4335" />
                    </linearGradient>
                  </defs>
                </svg>
              ) : isTranslation ? (
                <Languages size={20} className="text-secondary" />
              ) : (
                <Cpu size={20} className="text-secondary" />
              )}
            </div>
            <div className="model-card-title">
              <span className="model-name local-model-name">{familyName}</span>
              <LanguageBadges languages={allLanguages} />
            </div>
          </div>

          <div className="local-model-status-chip-wrap">
            {downloadingState ? (
              <span className="model-status-chip model-status-downloading">
                <Loader2 className="animate-spin" size={11} />
                <span>
                  {t('settings.llm.status_downloading', { defaultValue: 'Downloading' })}{' '}
                  {Math.round(downloadingState.progress)}%
                </span>
              </span>
            ) : installedCount === models.length ? (
              <span className="model-status-chip model-status-installed">
                <Check size={11} />
                <span>{t('settings.llm.installed_ready', { defaultValue: 'Ready' })}</span>
              </span>
            ) : installedCount > 0 ? (
              <span className="model-status-chip model-status-installed">
                <Check size={11} />
                <span>
                  {t('settings.model_status_partial_installed', {
                    installed: installedCount,
                    total: models.length,
                    defaultValue: `${installedCount}/${models.length} Installed`,
                  })}
                </span>
              </span>
            ) : interruptedModel?.interruptedDownload ? (
              <span className="model-status-chip model-status-interrupted">
                <span>
                  {interruptedModel.interruptedDownload.percentage != null
                    ? t('settings.model_status_interrupted', {
                        progress: Math.round(interruptedModel.interruptedDownload.percentage),
                        defaultValue: `Paused (${Math.round(interruptedModel.interruptedDownload.percentage)}%)`,
                      })
                    : t('settings.model_status_interrupted_no_progress', {
                        defaultValue: 'Paused',
                      })}
                </span>
              </span>
            ) : (
              <span className="model-status-chip model-status-not-installed">
                {t('settings.not_installed', { defaultValue: 'Not installed' })}
              </span>
            )}
          </div>
        </div>

        <div className="model-description local-model-card-desc">
          {t(baseModel.description, { defaultValue: baseModel.description })}
        </div>

        <div className="model-versions">
          {models.map((model) => {
            const isModelInstalled = model.isInstalled;
            const modelDl = downloads?.[model.id];
            const isModelDownloading = Boolean(modelDl);
            const mTestState = modelTestStates?.[model.id];
            const isModelTesting = mTestState?.status === 'loading';
            const mTestTooltip = !isModelInstalled
              ? t('settings.llm.install_required_to_test', {
                  defaultValue: 'Download and install model first to test',
                })
              : isModelTesting
                ? t('settings.llm.testing', { defaultValue: 'Generating...' })
                : mTestState?.status === 'success'
                  ? `✓ ${t('settings.llm.connection_success', { defaultValue: 'Inference successful' })}${mTestState.speed != null ? ` (${mTestState.speed} t/s)` : ''}`
                  : mTestState?.status === 'error'
                    ? `✕ ${mTestState.error || t('settings.llm.connection_failed', { defaultValue: 'Test failed' })}`
                    : t('settings.llm.test_model', { defaultValue: 'Test Connection' });

            return (
              <div
                key={model.id}
                className="model-version-row"
                data-testid={`model-version-row-${model.id}`}
              >
                <div className="model-version-main">
                  <div className="model-version-info">
                    <span className="model-version-label">{model.versionLabel || model.name}</span>
                    <span className="model-tag model-tag-engine">
                      {model.backend === 'onnx-nmt' ? 'ONNX' : 'GGUF'}
                    </span>
                    {isModelInstalled && <Check className="model-version-check" size={14} />}
                    {!isModelInstalled && !isModelDownloading && model.interruptedDownload && (
                      <span className="model-status-chip model-status-interrupted">
                        {model.interruptedDownload.percentage != null
                          ? t('settings.model_status_interrupted', {
                              progress: Math.round(model.interruptedDownload.percentage),
                              defaultValue: `Paused (${Math.round(model.interruptedDownload.percentage)}%)`,
                            })
                          : t('settings.model_status_interrupted_no_progress', {
                              defaultValue: 'Paused',
                            })}
                      </span>
                    )}
                  </div>
                  <div className="model-card-side">
                    <span className="model-size">{model.size}</span>
                    {mTestState?.speed != null && (
                      <span
                        className="local-model-speed-badge"
                        data-testid={`local-model-speed-${model.id}`}
                      >
                        {mTestState.speed} t/s
                      </span>
                    )}
                    <button
                      type="button"
                      className={`model-action-icon model-action-test${mTestState?.status ? ` ${mTestState.status}` : ''}`}
                      onClick={() => onTest?.(model)}
                      disabled={
                        isModelTesting ||
                        !isModelInstalled ||
                        isModelDownloading ||
                        (isAnyTesting && !isModelTesting)
                      }
                      aria-label={`${t('settings.llm.test_model', { defaultValue: 'Test Connection' })} ${model.name}`}
                      data-tooltip={mTestTooltip}
                      data-tooltip-pos="top"
                    >
                      {isModelTesting ? (
                        <Loader2 size={15} className="animate-spin" />
                      ) : mTestState?.status === 'success' ? (
                        <Check size={15} />
                      ) : mTestState?.status === 'error' ? (
                        <AlertCircle size={15} />
                      ) : (
                        <Zap size={15} />
                      )}
                    </button>
                    {isModelDownloading ? (
                      <button
                        type="button"
                        className="model-action-icon model-action-cancel"
                        onClick={() => onCancelDownload(model.id)}
                        aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
                        data-tooltip={t('common.cancel', { defaultValue: 'Cancel' })}
                        data-tooltip-pos="top"
                      >
                        <StopIcon />
                      </button>
                    ) : isModelInstalled ? (
                      <button
                        type="button"
                        className="model-action-icon model-action-delete"
                        onClick={() => onDelete(model)}
                        aria-label={`${t('common.delete', { defaultValue: 'Delete' })} ${model.name}`}
                        data-tooltip={t('common.delete', { defaultValue: 'Delete' })}
                        data-tooltip-pos="top"
                      >
                        <TrashIcon />
                      </button>
                    ) : model.interruptedDownload ? (
                      <>
                        <button
                          type="button"
                          className="model-action-icon"
                          onClick={() => onDownload(model)}
                          disabled={disabled}
                          aria-label={`${t('settings.resume_download', { defaultValue: 'Resume download' })} ${model.name}`}
                          data-tooltip={
                            model.interruptedDownload.percentage != null
                              ? t('settings.resume_download_tooltip', {
                                  progress: Math.round(model.interruptedDownload.percentage),
                                  defaultValue: `Resume download (${Math.round(model.interruptedDownload.percentage)}% completed)`,
                                })
                              : t('settings.resume_download', { defaultValue: 'Resume download' })
                          }
                          data-tooltip-pos="top"
                        >
                          <DownloadIcon />
                        </button>
                        {onDiscardDownload && (
                          <button
                            type="button"
                            className="model-action-icon model-action-delete"
                            onClick={() => onDiscardDownload(model)}
                            disabled={disabled}
                            aria-label={`${t('settings.discard_download', { defaultValue: 'Discard incomplete download' })} ${model.name}`}
                            data-tooltip={t('settings.discard_download_tooltip', {
                              size:
                                formatBytes(model.interruptedDownload.downloadedBytes) || '0 MB',
                              defaultValue: `Discard incomplete download (${formatBytes(model.interruptedDownload.downloadedBytes) || '0 MB'})`,
                            })}
                            data-tooltip-pos="top"
                          >
                            <TrashIcon />
                          </button>
                        )}
                      </>
                    ) : (
                      <button
                        type="button"
                        className="model-action-icon"
                        onClick={() => onDownload(model)}
                        disabled={disabled}
                        aria-label={`${t('common.download', { defaultValue: 'Download' })} ${model.name}`}
                        data-tooltip={t('common.download', { defaultValue: 'Download' })}
                        data-tooltip-pos="top"
                      >
                        <DownloadIcon />
                      </button>
                    )}
                  </div>
                </div>
                {isModelDownloading && (
                  <div className="progress-container-mini local-download-progress-container">
                    <div className="progress-info-mini" aria-live="polite">
                      <span className="progress-status-mini">
                        {modelDl?.statusText ||
                          `${t('settings.llm.status_downloading', { defaultValue: 'Downloading' })} ${
                            modelDl?.downloadedBytes && modelDl.totalBytes
                              ? `(${formatBytes(modelDl.downloadedBytes)} / ${formatBytes(modelDl.totalBytes)})${
                                  modelDl.speed ? ` (${modelDl.speed})` : ''
                                }`
                              : modelDl?.speed
                                ? `(${modelDl.speed})`
                                : ''
                          }`.trim()}
                      </span>
                      <span>{Math.round(modelDl?.progress ?? 0)}%</span>
                    </div>
                    <div
                      className="progress-bar-mini"
                      role="progressbar"
                      aria-valuenow={Math.round(modelDl?.progress ?? 0)}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-label={`${t('common.download')} ${model.name}`}
                    >
                      <div
                        className="progress-fill"
                        style={{ width: `${Math.min(100, Math.max(0, modelDl?.progress ?? 0))}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    );
  }

  const card = models[0];
  const downloadState = downloadStateProp ?? downloads?.[card.id];
  const testState = testStateProp ?? modelTestStates?.[card.id];
  const isDownloading = Boolean(downloadState);
  const isInstalled = card.isInstalled;
  const isTesting = testState?.status === 'loading';
  const modalities = resolveModelModalities(card);
  const testTooltip = !isInstalled
    ? t('settings.llm.install_required_to_test', {
        defaultValue: 'Download and install model first to test',
      })
    : isTesting
      ? t('settings.llm.testing', { defaultValue: 'Generating...' })
      : testState?.status === 'success'
        ? `✓ ${t('settings.llm.connection_success', { defaultValue: 'Inference successful' })}${testState.speed != null ? ` (${testState.speed} t/s)` : ''}`
        : testState?.status === 'error'
          ? `✕ ${testState.error || t('settings.llm.connection_failed', { defaultValue: 'Test failed' })}`
          : t('settings.llm.test_model', { defaultValue: 'Test Connection' });

  const isQwen = card.id.toLowerCase().includes('qwen') || card.name.toLowerCase().includes('qwen');
  const isGemma =
    card.id.toLowerCase().includes('gemma') || card.name.toLowerCase().includes('gemma');
  const isTranslation =
    card.groupId === 'opus-mt' ||
    card.groupId === 'index-translate' ||
    card.groupId === 'hunyuan-mt' ||
    card.backend === 'onnx-nmt' ||
    (card.capabilities?.length === 1 && card.capabilities[0] === 'translate');

  return (
    <div
      className={`local-model-card model-card${isInstalled ? ' is-installed' : ''}`}
      data-testid={`local-model-card-${card.id}`}
    >
      <div className="model-card-header local-model-card-header">
        <div className="model-card-identity local-model-card-identity">
          <div className="model-card-logo-badge">
            {isQwen ? (
              <ModelBrandLogo brand="qwen" size={36} />
            ) : isGemma ? (
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M12 2L14.4 9.6L22 12L14.4 14.4L12 22L9.6 14.4L2 12L9.6 9.6L12 2Z"
                  fill="url(#gemma-gradient)"
                />
                <defs>
                  <linearGradient
                    id="gemma-gradient"
                    x1="2"
                    y1="2"
                    x2="22"
                    y2="22"
                    gradientUnits="userSpaceOnUse"
                  >
                    <stop stopColor="#4285F4" />
                    <stop offset="0.5" stopColor="#9B72CB" />
                    <stop offset="1" stopColor="#EA4335" />
                  </linearGradient>
                </defs>
              </svg>
            ) : isTranslation ? (
              <Languages size={24} color="var(--color-primary)" />
            ) : (
              <Cpu size={20} className="text-secondary" />
            )}
          </div>
          <div className="model-card-title">
            <span className="model-name local-model-name">{card.name}</span>
            <span className="local-model-modality-tags">
              {modalities.map((modality) => {
                const config = MODALITY_CONFIG[modality];
                return (
                  <span
                    key={modality}
                    className={`model-tag model-tag-modality model-tag-modality-${modality}`}
                    data-tooltip={t(config.tooltipKey, { defaultValue: config.label })}
                    data-tooltip-pos="top"
                  >
                    {config.icon}
                    <span>{t(config.labelKey, { defaultValue: config.label })}</span>
                  </span>
                );
              })}
            </span>
          </div>
        </div>

        <div className="local-model-status-chip-wrap">
          {isDownloading ? (
            <span className="model-status-chip model-status-downloading">
              <Loader2 className="animate-spin" size={11} />
              <span>
                {t('settings.llm.status_downloading', { defaultValue: 'Downloading' })}{' '}
                {downloadState ? Math.round(downloadState.progress) : 0}%
              </span>
            </span>
          ) : isInstalled ? (
            <span className="model-status-chip model-status-installed">
              <Check size={11} />
              <span>{t('settings.llm.installed_ready', { defaultValue: 'Ready' })}</span>
            </span>
          ) : card.interruptedDownload ? (
            <span className="model-status-chip model-status-interrupted">
              <span>
                {card.interruptedDownload.percentage != null
                  ? t('settings.model_status_interrupted', {
                      progress: Math.round(card.interruptedDownload.percentage),
                      defaultValue: `Paused (${Math.round(card.interruptedDownload.percentage)}%)`,
                    })
                  : t('settings.model_status_interrupted_no_progress', { defaultValue: 'Paused' })}
              </span>
            </span>
          ) : (
            <span className="model-status-chip model-status-not-installed">
              {t('settings.not_installed', { defaultValue: 'Not installed' })}
            </span>
          )}
        </div>
      </div>

      <div className="model-description local-model-card-desc">
        {t(card.description, { defaultValue: card.description })}
      </div>

      <div className="model-tags">
        {card.isRecommended && (
          <span className="model-tag model-tag-recommended">
            {t('settings.llm.recommended_model_tag', { defaultValue: 'Recommended' })}
          </span>
        )}
        {card.parameters && <span className="model-tag model-tag-param">{card.parameters}</span>}
        {card.quantization && (
          <span className="model-tag model-tag-quant">{card.quantization}</span>
        )}
        {card.capabilities
          .filter(
            (cap) => cap !== 'chat' && cap !== 'polish' && cap !== 'summary' && cap !== 'translate'
          )
          .map((cap) => {
            let icon = null;
            let label = cap;
            if (cap === 'reasoning') {
              label = t('settings.llm.capability_reasoning', { defaultValue: 'Reasoning' });
              icon = <BrainCircuit size={11} />;
            }
            return (
              <span key={cap} className="model-tag model-tag-cap">
                {icon}
                <span>{label}</span>
              </span>
            );
          })}
      </div>

      <div className="local-model-specs-grid">
        <div className="local-model-spec-item">
          <span className="local-model-spec-label">
            {t('settings.llm.specs_context', { defaultValue: 'Context' })}
          </span>
          <span className="local-model-spec-value">
            {Math.round(card.contextWindow / 1024)}K ({card.contextWindow.toLocaleString()} tokens)
          </span>
        </div>
        <div className="local-model-spec-item">
          <span className="local-model-spec-label">
            {t('settings.llm.specs_size', { defaultValue: 'Size' })}
          </span>
          <span className="local-model-spec-value">{card.size}</span>
        </div>
        <div className="local-model-spec-item">
          <span className="local-model-spec-label">
            {t('settings.llm.specs_quantization', { defaultValue: 'Quantization' })}
          </span>
          <span className="local-model-spec-value">{card.quantization || '-'}</span>
        </div>
      </div>

      {isDownloading && (
        <div className="progress-container-mini local-download-progress-container">
          <div className="progress-info-mini" aria-live="polite">
            <span className="progress-status-mini">
              {downloadState?.statusText ||
                `${t('settings.llm.status_downloading', { defaultValue: 'Downloading' })} ${
                  downloadState?.downloadedBytes && downloadState.totalBytes
                    ? `(${formatBytes(downloadState.downloadedBytes)} / ${formatBytes(downloadState.totalBytes)})${
                        downloadState.speed ? ` (${downloadState.speed})` : ''
                      }`
                    : downloadState?.speed
                      ? `(${downloadState.speed})`
                      : ''
                }`.trim()}
            </span>
            <span>{Math.round(downloadState?.progress ?? 0)}%</span>
          </div>
          <div
            className="progress-bar-mini"
            role="progressbar"
            aria-valuenow={Math.round(downloadState?.progress ?? 0)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`${t('common.download')} ${card.name}`}
          >
            <div
              className="progress-fill"
              style={{ width: `${Math.min(100, Math.max(0, downloadState?.progress ?? 0))}%` }}
            />
          </div>
        </div>
      )}

      <div className="local-model-footer model-card-footer">
        <span className="model-size local-model-filename-hint">{card.filename}</span>
        <div className="model-card-side">
          {testState?.speed != null && (
            <span className="local-model-speed-badge" data-testid={`local-model-speed-${card.id}`}>
              {testState.speed} t/s
            </span>
          )}
          <button
            type="button"
            className={`model-action-icon model-action-test${testState?.status ? ` ${testState.status}` : ''}`}
            onClick={() => onTest?.(card)}
            disabled={isTesting || !isInstalled || isDownloading || (isAnyTesting && !isTesting)}
            aria-label={`${t('settings.llm.test_model', { defaultValue: 'Test Connection' })} ${card.name}`}
            data-tooltip={testTooltip}
            data-tooltip-pos="top"
          >
            {isTesting ? (
              <Loader2 size={15} className="animate-spin" />
            ) : testState?.status === 'success' ? (
              <Check size={15} />
            ) : testState?.status === 'error' ? (
              <AlertCircle size={15} />
            ) : (
              <Zap size={15} />
            )}
          </button>
          {isDownloading ? (
            <button
              type="button"
              className="model-action-icon model-action-cancel"
              onClick={() => onCancelDownload(card.id)}
              aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
              data-tooltip={t('common.cancel', { defaultValue: 'Cancel' })}
              data-tooltip-pos="top"
            >
              <StopIcon />
            </button>
          ) : isInstalled ? (
            <button
              type="button"
              className="model-action-icon model-action-delete"
              onClick={() => onDelete(card)}
              aria-label={`${t('common.delete', { defaultValue: 'Delete' })} ${card.name}`}
              data-tooltip={t('common.delete', { defaultValue: 'Delete' })}
              data-tooltip-pos="top"
            >
              <TrashIcon />
            </button>
          ) : card.interruptedDownload ? (
            <>
              <button
                type="button"
                className="model-action-icon"
                onClick={() => onDownload(card)}
                disabled={disabled}
                aria-label={`${t('settings.resume_download', { defaultValue: 'Resume download' })} ${card.name}`}
                data-tooltip={
                  card.interruptedDownload.percentage != null
                    ? t('settings.resume_download_tooltip', {
                        progress: Math.round(card.interruptedDownload.percentage),
                        defaultValue: `Resume download (${Math.round(card.interruptedDownload.percentage)}% completed)`,
                      })
                    : t('settings.resume_download', { defaultValue: 'Resume download' })
                }
                data-tooltip-pos="top"
              >
                <DownloadIcon />
              </button>
              {onDiscardDownload && (
                <button
                  type="button"
                  className="model-action-icon model-action-delete"
                  onClick={() => onDiscardDownload(card)}
                  disabled={disabled}
                  aria-label={`${t('settings.discard_download', { defaultValue: 'Discard incomplete download' })} ${card.name}`}
                  data-tooltip={t('settings.discard_download_tooltip', {
                    size: formatBytes(card.interruptedDownload.downloadedBytes) || '0 MB',
                    defaultValue: `Discard incomplete download (${formatBytes(card.interruptedDownload.downloadedBytes) || '0 MB'})`,
                  })}
                  data-tooltip-pos="top"
                >
                  <TrashIcon />
                </button>
              )}
            </>
          ) : (
            <button
              type="button"
              className="model-action-icon"
              onClick={() => onDownload(card)}
              disabled={disabled}
              aria-label={`${t('common.download', { defaultValue: 'Download' })} ${card.name}`}
              data-tooltip={t('common.download', { defaultValue: 'Download' })}
              data-tooltip-pos="top"
            >
              <DownloadIcon />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
