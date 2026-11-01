import type React from 'react';
import { useTranslation } from 'react-i18next';
import { formatBytes } from '../../hooks/notifications/modelDownloadAdapter';
import { type ModelInfo, resolveModelLabels, resolveModelRatings } from '../../types/modelCatalog';
import { CheckIcon, DownloadIcon, StopIcon, TrashIcon } from '../Icons';
import { isAsrModel, ModelBrandLogo, resolveModelBrand } from '../icons/ModelLogos';
import { LanguageBadges } from '../LanguageBadges';

const ENGINE_LABEL: Record<ModelInfo['engine'], string> = {
  'sherpa-onnx': 'ONNX',
  'llama-cpp': 'GGUF',
  'onnx-nmt': 'NMT',
};

type ModelStatus =
  | { kind: 'not-installed' }
  | { kind: 'downloading'; progress: number }
  | { kind: 'installed' }
  | { kind: 'partial'; installed: number; total: number }
  | { kind: 'interrupted'; progress: number | null };

interface ModelCardProps {
  models: ModelInfo[];
  installedModels: Set<string>;
  downloads: Record<string, { progress: number; status: string }>;
  onDelete: (model: ModelInfo) => void;
  onDownload: (model: ModelInfo) => void;
  onCancelDownload: (modelId: string) => void;
  onDiscardDownload?: (model: ModelInfo) => void;
  actionsDisabled?: boolean;
  isAsr?: boolean;
}

interface ModelCardActionsProps {
  model: ModelInfo;
  isInstalled: boolean;
  isDownloading: boolean;
  onDelete: (model: ModelInfo) => void;
  onDownload: (model: ModelInfo) => void;
  onCancelDownload: () => void;
  onDiscardDownload?: (model: ModelInfo) => void;
  disabled?: boolean;
}

function RatingMeter({ label, value }: { label: string; value?: number }) {
  if (!value) return null;
  const tooltip = `${label} ${value}/5`;
  return (
    // role="img" gives the aria-label somewhere to attach; the dots are decorative.
    <span
      className="model-rating"
      role="img"
      aria-label={tooltip}
      data-tooltip={tooltip}
      data-tooltip-pos="top"
    >
      <span className="model-rating-label">{label}</span>
      <span className="model-rating-dots" aria-hidden="true">
        {[1, 2, 3, 4, 5].map((level) => (
          <span key={level} className={`model-rating-dot${level <= value ? ' filled' : ''}`} />
        ))}
      </span>
    </span>
  );
}

function RatingMeters({ model }: { model: ModelInfo }) {
  const { t } = useTranslation();
  const ratings = resolveModelRatings(model);
  if (!ratings?.accuracy && !ratings?.speed) return null;
  return (
    <span className="model-ratings">
      <RatingMeter
        label={t('settings.model_rating_accuracy', { defaultValue: 'Accuracy' })}
        value={ratings.accuracy}
      />
      <RatingMeter
        label={t('settings.model_rating_speed', { defaultValue: 'Speed' })}
        value={ratings.speed}
      />
    </span>
  );
}

function ModelStatusChip({ status }: { status: ModelStatus }) {
  const { t } = useTranslation();
  switch (status.kind) {
    case 'downloading':
      return (
        <span className="model-status-chip model-status-downloading">
          {t('settings.downloading_progress', {
            progress: Math.round(status.progress),
            defaultValue: `Downloading (${Math.round(status.progress)}%)`,
          })}
        </span>
      );
    case 'installed':
      return (
        <span className="model-status-chip model-status-installed">
          <CheckIcon />
          {t('settings.model_status_installed', { defaultValue: 'Installed' })}
        </span>
      );
    case 'partial':
      return (
        <span className="model-status-chip model-status-installed">
          <CheckIcon />
          {t('settings.model_status_partial_installed', {
            installed: status.installed,
            total: status.total,
            defaultValue: '{{installed}}/{{total}} Installed',
          })}
        </span>
      );
    case 'interrupted':
      return (
        <span className="model-status-chip model-status-interrupted">
          {status.progress != null
            ? t('settings.model_status_interrupted', {
                progress: Math.round(status.progress),
                defaultValue: `Paused (${Math.round(status.progress)}%)`,
              })
            : t('settings.model_status_interrupted_no_progress', { defaultValue: 'Paused' })}
        </span>
      );
    default:
      return (
        <span className="model-status-chip model-status-not-installed">
          {t('settings.not_installed', { defaultValue: 'Not installed' })}
        </span>
      );
  }
}

export function formatModelModeTag(mode: string): string {
  if (mode === 'streaming' || mode === 'live') {
    return 'Live';
  }
  return mode.charAt(0).toUpperCase() + mode.slice(1);
}

export function resolveUniqueModeTags(modes?: ModelInfo['modes']): string[] {
  if (!modes || modes.length === 0) return [];
  const seen = new Set<string>();
  const result: string[] = [];
  for (const mode of modes) {
    const formatted = formatModelModeTag(mode);
    if (!seen.has(formatted)) {
      seen.add(formatted);
      result.push(formatted);
    }
  }
  return result;
}

/** Semantic tag cluster: scenario modes, engine, curated labels, recommendation. */
function ModelTags({ model, showEngine }: { model: ModelInfo; showEngine: boolean }) {
  const { t } = useTranslation();
  const labels = resolveModelLabels(model) ?? [];
  const hasModes = (model.modes?.length ?? 0) > 0;
  if (!hasModes && !showEngine && labels.length === 0 && !model.isRecommended) return null;
  return (
    <div className="model-tags">
      {resolveUniqueModeTags(model.modes).map((tag) => (
        <span key={tag} className="model-tag model-tag-mode">
          {tag}
        </span>
      ))}
      {showEngine && (
        <span className="model-tag model-tag-engine">{ENGINE_LABEL[model.engine]}</span>
      )}
      {labels.map((label) => (
        <span key={label} className={`model-tag model-tag-${label}`}>
          {t(`settings.model_tag_${label}`, { defaultValue: label })}
        </span>
      ))}
      {model.isRecommended && (
        <span className="model-tag model-tag-recommended">{t('common.recommended')}</span>
      )}
    </div>
  );
}

function ModelDownloadProgress({
  model,
  progress,
  status,
}: {
  model: ModelInfo;
  progress: number;
  status: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="progress-container-mini">
      <div className="progress-info-mini" aria-live="polite">
        <span className="progress-status-mini">{status || t('common.loading')}</span>
        <span>{Math.round(progress)}%</span>
      </div>
      <div
        className="progress-bar-mini"
        role="progressbar"
        aria-valuenow={Math.round(progress)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={`${t('common.download')} ${model.name}`}
      >
        <div className="progress-fill" style={{ width: `${progress}%` }} />
      </div>
    </div>
  );
}

/**
 * Ghost icon-only actions: download/cancel share the quiet neutral hover,
 * while the destructive delete turns red on hover so it never competes with
 * the primary flow.
 */
function ModelCardActions({
  model,
  isInstalled,
  isDownloading,
  onDelete,
  onDownload,
  onCancelDownload,
  onDiscardDownload,
  disabled = false,
}: ModelCardActionsProps): React.JSX.Element {
  const { t } = useTranslation();

  if (isInstalled) {
    return (
      <button
        type="button"
        className="model-action-icon model-action-delete"
        onClick={() => onDelete(model)}
        disabled={disabled}
        aria-label={`${t('common.delete')} ${model.name}`}
        data-tooltip={t('common.delete')}
      >
        <TrashIcon />
      </button>
    );
  }

  if (isDownloading) {
    return (
      <button
        type="button"
        className="model-action-icon model-action-cancel"
        onClick={onCancelDownload}
        aria-label={t('common.cancel')}
        data-tooltip={t('common.cancel')}
      >
        <StopIcon />
      </button>
    );
  }

  const isInterrupted = !isInstalled && !isDownloading && !!model.interruptedDownload;
  if (isInterrupted) {
    const pct = model.interruptedDownload?.percentage;
    const downloadedFormatted = formatBytes(model.interruptedDownload?.downloadedBytes) || '0 MB';
    const downloadTooltip =
      pct != null
        ? t('settings.resume_download_tooltip', {
            progress: Math.round(pct),
            defaultValue: `Resume download (${Math.round(pct)}% completed)`,
          })
        : t('settings.resume_download', { defaultValue: 'Resume download' });
    const downloadAriaLabel = `${t('settings.resume_download', { defaultValue: 'Resume download' })} ${model.name}`;

    return (
      <>
        <button
          type="button"
          className="model-action-icon"
          onClick={() => onDownload(model)}
          disabled={disabled}
          aria-label={downloadAriaLabel}
          data-tooltip={downloadTooltip}
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
              size: downloadedFormatted,
              defaultValue: `Discard incomplete download (${downloadedFormatted})`,
            })}
          >
            <TrashIcon />
          </button>
        )}
      </>
    );
  }

  return (
    <button
      type="button"
      className="model-action-icon"
      onClick={() => onDownload(model)}
      disabled={disabled}
      aria-label={`${t('common.download')} ${model.name}`}
      data-tooltip={t('common.download')}
    >
      <DownloadIcon />
    </button>
  );
}

export function ModelCard({
  models,
  installedModels,
  downloads,
  onDelete,
  onDownload,
  onCancelDownload,
  onDiscardDownload,
  actionsDisabled = false,
  isAsr: isAsrProp,
}: ModelCardProps): React.JSX.Element | null {
  const { t } = useTranslation();

  if (!models || models.length === 0) return null;

  const baseModel = models[0];
  const isMultiVersion = models.length > 1;
  const baseDownload = downloads[baseModel.id];

  const isAsr = isAsrProp ?? isAsrModel(baseModel);
  const brand = isAsr ? resolveModelBrand(baseModel) : null;

  const headerStatus: ModelStatus = (() => {
    if (!isMultiVersion) {
      if (baseDownload) return { kind: 'downloading', progress: baseDownload.progress };
      if (installedModels.has(baseModel.id)) return { kind: 'installed' };
      if (baseModel.interruptedDownload) {
        return {
          kind: 'interrupted',
          progress: baseModel.interruptedDownload.percentage ?? null,
        };
      }
      return { kind: 'not-installed' };
    }
    const downloadingModel = models.find((model) => downloads[model.id]);
    if (downloadingModel) {
      return { kind: 'downloading', progress: downloads[downloadingModel.id].progress };
    }
    const installedCount = models.filter((model) => installedModels.has(model.id)).length;
    if (installedCount === models.length) return { kind: 'installed' };
    if (installedCount > 0)
      return { kind: 'partial', installed: installedCount, total: models.length };
    const interruptedModel = models.find(
      (model) => !installedModels.has(model.id) && model.interruptedDownload
    );
    if (interruptedModel?.interruptedDownload) {
      return {
        kind: 'interrupted',
        progress: interruptedModel.interruptedDownload.percentage ?? null,
      };
    }
    return { kind: 'not-installed' };
  })();

  return (
    <div className="model-card">
      <div className="model-card-header">
        <div className="model-card-identity">
          {brand && (
            <div className="model-card-logo-badge">
              <ModelBrandLogo brand={brand} size={36} />
            </div>
          )}
          <div className="model-card-title">
            <span className="model-name">
              {baseModel.name}
              {!isMultiVersion && baseModel.versionLabel ? ` · ${baseModel.versionLabel}` : ''}
            </span>
            <LanguageBadges languages={baseModel.languages} />
          </div>
        </div>
        <ModelStatusChip status={headerStatus} />
      </div>
      <div className="model-description">{t(baseModel.description)}</div>

      {isMultiVersion && (
        /* One family-level rating set, right-aligned on the group tag row —
           version rows stay rating-free so the family shows it only once. */
        <div className="model-card-tags-row">
          <ModelTags model={baseModel} showEngine={false} />
          <RatingMeters model={baseModel} />
        </div>
      )}

      {!isMultiVersion && (
        <>
          {!!baseDownload && (
            <ModelDownloadProgress
              model={baseModel}
              progress={baseDownload.progress}
              status={baseDownload.status}
            />
          )}
          <div className="model-card-footer">
            <div className="model-card-meta">
              <ModelTags model={baseModel} showEngine />
              <RatingMeters model={baseModel} />
            </div>
            <div className="model-card-side">
              <span className="model-size">{baseModel.size}</span>
              <ModelCardActions
                model={baseModel}
                isInstalled={installedModels.has(baseModel.id)}
                isDownloading={!!baseDownload}
                onDelete={onDelete}
                onDownload={onDownload}
                onCancelDownload={() => onCancelDownload(baseModel.id)}
                onDiscardDownload={onDiscardDownload}
                disabled={actionsDisabled}
              />
            </div>
          </div>
        </>
      )}

      {isMultiVersion && (
        <div className="model-versions">
          {models.map((model) => {
            const isInstalled = installedModels.has(model.id);
            const downloadState = downloads[model.id];
            const isDownloading = !!downloadState;

            return (
              <div key={model.id} className="model-version-row">
                <div className="model-version-main">
                  <div className="model-version-info">
                    <span className="model-version-label">{model.versionLabel || model.name}</span>
                    <span className="model-tag model-tag-engine">{ENGINE_LABEL[model.engine]}</span>
                    {isInstalled && <CheckIcon className="model-version-check" />}
                    {!isInstalled && !isDownloading && model.interruptedDownload && (
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
                    <ModelCardActions
                      model={model}
                      isInstalled={isInstalled}
                      isDownloading={isDownloading}
                      onDelete={onDelete}
                      onDownload={onDownload}
                      onCancelDownload={() => onCancelDownload(model.id)}
                      onDiscardDownload={onDiscardDownload}
                      disabled={actionsDisabled}
                    />
                  </div>
                </div>
                {isDownloading && (
                  <ModelDownloadProgress
                    model={model}
                    progress={downloadState.progress}
                    status={downloadState.status}
                  />
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
