import { AlignLeft, Check, Cpu, Download, Globe, Loader2, Sparkles, Trash2, X } from 'lucide-react';
import type React from 'react';
import type { LocalLlmModelCard as LocalLlmModelCardType } from '../../../bindings';
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
  activeFeatures: {
    polish: boolean;
    translation: boolean;
    summary: boolean;
  };
  onDownload: (card: LocalLlmModelCardType) => void;
  onCancelDownload: (cardId: string) => void;
  onDelete: (card: LocalLlmModelCardType) => void;
  onApplyFeature: (
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

export function LocalModelCard({
  card,
  downloadState,
  activeFeatures,
  onDownload,
  onCancelDownload,
  onDelete,
  onApplyFeature,
  t,
}: LocalModelCardProps): React.JSX.Element {
  const isDownloading = Boolean(downloadState);
  const isInstalled = card.isInstalled;
  const isAllApplied =
    activeFeatures.polish && activeFeatures.translation && activeFeatures.summary;

  const isQwen = card.id.toLowerCase().includes('qwen') || card.name.toLowerCase().includes('qwen');

  return (
    <div
      className={`local-model-card${isInstalled ? ' is-installed' : ''}`}
      data-testid={`local-model-card-${card.id}`}
    >
      <div className="local-model-card-header">
        <div className="local-model-card-identity">
          <div className="local-model-logo-box">
            {isQwen ? (
              <ModelBrandLogo brand="qwen" size={36} />
            ) : (
              <Cpu size={24} className="text-secondary" />
            )}
          </div>
          <div className="local-model-title-wrap">
            <div className="local-model-title-row">
              <span className="local-model-name">{card.name}</span>
              {card.isRecommended && (
                <span className="local-model-badge local-model-badge-rec">
                  {t('settings.llm.recommended_model_tag', { defaultValue: '推荐' })}
                </span>
              )}
              {card.parameters && (
                <span className="local-model-badge local-model-badge-param">{card.parameters}</span>
              )}
              {card.quantization && (
                <span className="local-model-badge local-model-badge-quant">
                  {card.quantization}
                </span>
              )}
            </div>
            <div className="local-model-caps-row">
              {card.capabilities.map((cap) => {
                let icon = null;
                let label = cap;
                if (cap === 'chat') {
                  label = t('settings.llm.capability_chat', { defaultValue: '智能对话' });
                } else if (cap === 'polish') {
                  icon = <Sparkles size={11} />;
                  label = t('settings.llm.capability_polish', { defaultValue: '文本润色' });
                } else if (cap === 'summary') {
                  icon = <AlignLeft size={11} />;
                  label = t('settings.llm.capability_summary', { defaultValue: '长文总结' });
                } else if (cap === 'translate') {
                  icon = <Globe size={11} />;
                  label = t('settings.llm.capability_translate', { defaultValue: '多语翻译' });
                }
                return (
                  <span key={cap} className="local-model-cap-pill">
                    {icon}
                    <span>{label}</span>
                  </span>
                );
              })}
            </div>
          </div>
        </div>

        <div className="local-model-status-chip-wrap">
          {isDownloading ? (
            <span className="model-status-chip model-status-downloading">
              <Loader2 className="animate-spin" size={12} />
              <span>
                {t('settings.llm.status_downloading', { defaultValue: '正在下载' })}{' '}
                {downloadState ? Math.round(downloadState.progress) : 0}%
              </span>
            </span>
          ) : isInstalled ? (
            <span className="model-status-chip model-status-installed">
              <Check size={12} />
              <span>{t('settings.llm.installed_ready', { defaultValue: '已就绪' })}</span>
            </span>
          ) : (
            <span className="model-status-chip model-status-not-installed">
              {t('settings.not_installed', { defaultValue: '未安装' })}
            </span>
          )}
        </div>
      </div>

      <div className="local-model-card-desc">
        {t(card.description, { defaultValue: card.description })}
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
          <span className="local-model-spec-value">{card.quantization || 'Q4_K_M'}</span>
        </div>
        <div className="local-model-spec-item">
          <span className="local-model-spec-label">
            {t('settings.llm.specs_backend', { defaultValue: '后端' })}
          </span>
          <span className="local-model-spec-value">{card.backend} (GGUF)</span>
        </div>
      </div>

      <div className="local-model-footer">
        {isDownloading ? (
          <div className="local-download-progress-container">
            <div className="local-download-track">
              <div
                className="local-download-fill"
                style={{ width: `${Math.min(100, Math.max(0, downloadState?.progress ?? 0))}%` }}
              />
            </div>
            <div className="local-download-info-row">
              <span>
                {downloadState?.statusText ||
                  `${Math.round(downloadState?.progress ?? 0)}% ${
                    downloadState?.downloadedBytes && downloadState.totalBytes
                      ? `(${formatBytes(downloadState.downloadedBytes)} / ${formatBytes(downloadState.totalBytes)})`
                      : ''
                  }`}
              </span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => onCancelDownload(card.id)}
              >
                <X size={14} />
                <span>{t('settings.llm.cancel_download', { defaultValue: '取消下载' })}</span>
              </button>
            </div>
          </div>
        ) : isInstalled ? (
          <>
            <div className="local-apply-buttons-group">
              <button
                type="button"
                className={`btn btn-sm ${isAllApplied ? 'btn-success' : 'btn-primary'}`}
                onClick={() => onApplyFeature(card, 'all')}
                title={t('settings.llm.apply_all_features')}
              >
                {isAllApplied ? <Check size={14} /> : <Sparkles size={14} />}
                <span>
                  {isAllApplied
                    ? t('settings.llm.applied_all', { defaultValue: '已用于全部功能' })
                    : t('settings.llm.apply_all_features', { defaultValue: '设为全部功能模型' })}
                </span>
              </button>

              <button
                type="button"
                className={`btn btn-sm ${activeFeatures.polish ? 'btn-secondary-active' : 'btn-secondary'}`}
                onClick={() => onApplyFeature(card, 'polish')}
              >
                <Sparkles size={12} />
                <span>{t('settings.llm.polish_model', { defaultValue: '润色' })}</span>
                {activeFeatures.polish && <Check size={12} />}
              </button>

              <button
                type="button"
                className={`btn btn-sm ${activeFeatures.translation ? 'btn-secondary-active' : 'btn-secondary'}`}
                onClick={() => onApplyFeature(card, 'translation')}
              >
                <Globe size={12} />
                <span>{t('settings.llm.translation_model', { defaultValue: '翻译' })}</span>
                {activeFeatures.translation && <Check size={12} />}
              </button>

              <button
                type="button"
                className={`btn btn-sm ${activeFeatures.summary ? 'btn-secondary-active' : 'btn-secondary'}`}
                onClick={() => onApplyFeature(card, 'summary')}
              >
                <AlignLeft size={12} />
                <span>{t('settings.llm.summary_model', { defaultValue: '摘要' })}</span>
                {activeFeatures.summary && <Check size={12} />}
              </button>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              {card.installedPath && (
                <span className="local-model-path-hint" title={card.installedPath}>
                  {card.installedPath}
                </span>
              )}
              <button
                type="button"
                className="btn btn-icon btn-secondary-soft"
                onClick={() => onDelete(card)}
                title={t('settings.llm.delete_model', { defaultValue: '删除模型' })}
                aria-label={t('settings.llm.delete_model', { defaultValue: '删除模型' })}
              >
                <Trash2 size={15} />
              </button>
            </div>
          </>
        ) : (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              width: '100%',
            }}
          >
            <button type="button" className="btn btn-primary" onClick={() => onDownload(card)}>
              <Download size={16} />
              <span>
                {t('settings.llm.download_model_with_size', {
                  size: card.size,
                  defaultValue: `点击下载 (${card.size})`,
                })}
              </span>
            </button>
            <span
              className="local-model-size-hint"
              style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}
            >
              {card.filename}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
