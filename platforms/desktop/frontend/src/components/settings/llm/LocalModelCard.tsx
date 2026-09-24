import { BrainCircuit, Check, Cpu, FileText, Image, Loader2, Mic, Video } from 'lucide-react';
import type React from 'react';
import type { LocalLlmModelCard as LocalLlmModelCardType } from '../../../bindings';
import { DownloadIcon, TrashIcon, XIcon } from '../../Icons';
import { ModelBrandLogo } from '../../icons/ModelLogos';
import './LocalModelCard.css';

export interface LocalDownloadProgressState {
  progress: number;
  statusText?: string;
  downloadedBytes?: number;
  totalBytes?: number;
}

interface LocalModelCardProps {
  card: LocalLlmModelCardType;
  downloadState?: LocalDownloadProgressState;
  activeFeatures?: {
    polish?: boolean;
    translation?: boolean;
    summary?: boolean;
  };
  onDownload: (card: LocalLlmModelCardType) => void;
  onCancelDownload: (cardId: string) => void;
  onDelete: (card: LocalLlmModelCardType) => void;
  onApplyFeature?: (
    card: LocalLlmModelCardType,
    feature: 'polish' | 'translation' | 'summary' | 'all'
  ) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

function formatBytes(bytes?: number): string {
  if (!bytes || bytes <= 0) return '';
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(0)} MB`;
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
  card,
  downloadState,
  onDownload,
  onCancelDownload,
  onDelete,
  t,
}: LocalModelCardProps): React.JSX.Element {
  const isDownloading = Boolean(downloadState);
  const isInstalled = card.isInstalled;
  const modalities = resolveModelModalities(card);

  const isQwen = card.id.toLowerCase().includes('qwen') || card.name.toLowerCase().includes('qwen');
  const isGemma =
    card.id.toLowerCase().includes('gemma') || card.name.toLowerCase().includes('gemma');

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
                {t('settings.llm.status_downloading', { defaultValue: '正在下载' })}{' '}
                {downloadState ? Math.round(downloadState.progress) : 0}%
              </span>
            </span>
          ) : isInstalled ? (
            <span className="model-status-chip model-status-installed">
              <Check size={11} />
              <span>{t('settings.llm.installed_ready', { defaultValue: '已就绪' })}</span>
            </span>
          ) : (
            <span className="model-status-chip model-status-not-installed">
              {t('settings.not_installed', { defaultValue: '未安装' })}
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
            {t('settings.llm.recommended_model_tag', { defaultValue: '推荐' })}
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
              label = t('settings.llm.capability_reasoning', { defaultValue: '深度思考' });
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
            {t('settings.llm.specs_context', { defaultValue: '上下文' })}
          </span>
          <span className="local-model-spec-value">
            {Math.round(card.contextWindow / 1024)}K ({card.contextWindow.toLocaleString()} tokens)
          </span>
        </div>
        <div className="local-model-spec-item">
          <span className="local-model-spec-label">
            {t('settings.llm.specs_size', { defaultValue: '文件大小' })}
          </span>
          <span className="local-model-spec-value">{card.size}</span>
        </div>
        <div className="local-model-spec-item">
          <span className="local-model-spec-label">
            {t('settings.llm.specs_quantization', { defaultValue: '量化' })}
          </span>
          <span className="local-model-spec-value">{card.quantization || '-'}</span>
        </div>
      </div>

      {isDownloading && (
        <div className="progress-container-mini local-download-progress-container">
          <div className="progress-info-mini" aria-live="polite">
            <span className="progress-status-mini">
              {downloadState?.statusText ||
                `${t('settings.llm.status_downloading', { defaultValue: '正在下载' })} ${
                  downloadState?.downloadedBytes && downloadState.totalBytes
                    ? `(${formatBytes(downloadState.downloadedBytes)} / ${formatBytes(downloadState.totalBytes)})`
                    : ''
                }`}
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
          {isDownloading ? (
            <button
              type="button"
              className="model-action-icon"
              onClick={() => onCancelDownload(card.id)}
              aria-label={t('common.cancel', { defaultValue: '取消' })}
              data-tooltip={t('common.cancel', { defaultValue: '取消' })}
              data-tooltip-pos="top"
            >
              <XIcon />
            </button>
          ) : isInstalled ? (
            <button
              type="button"
              className="model-action-icon model-action-delete"
              onClick={() => onDelete(card)}
              aria-label={`${t('common.delete', { defaultValue: '删除' })} ${card.name}`}
              data-tooltip={t('common.delete', { defaultValue: '删除' })}
              data-tooltip-pos="top"
            >
              <TrashIcon />
            </button>
          ) : (
            <button
              type="button"
              className="model-action-icon"
              onClick={() => onDownload(card)}
              aria-label={`${t('common.download', { defaultValue: '下载' })} ${card.name}`}
              data-tooltip={t('common.download', { defaultValue: '下载' })}
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
