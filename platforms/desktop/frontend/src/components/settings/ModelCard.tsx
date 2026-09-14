import type React from 'react';
import { useTranslation } from 'react-i18next';
import { type ModelInfo, resolveModelLabels, resolveModelRatings } from '../../types/modelCatalog';
import { CheckIcon, DownloadIcon, TrashIcon, XIcon } from '../Icons';
import { isAsrModel, ModelBrandLogo, resolveModelBrand } from '../icons/ModelLogos';
import { LanguageBadges } from '../LanguageBadges';

const ENGINE_LABEL: Record<ModelInfo['engine'], string> = {
  'sherpa-onnx': 'ONNX',
  'llama-cpp': 'GGUF',
};

type ModelStatus =
  | { kind: 'not-installed' }
  | { kind: 'downloading'; progress: number }
  | { kind: 'installed' }
  | { kind: 'partial'; installed: number; total: number };

interface ModelCardProps {
  models: ModelInfo[];
  installedModels: Set<string>;
  downloads: Record<string, { progress: number; status: string }>;
  onDelete: (model: ModelInfo) => void;
  onDownload: (model: ModelInfo) => void;
  onCancelDownload: (modelId: string) => void;
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
        label={t('settings.model_rating_accuracy', { defaultValue: '准确度' })}
        value={ratings.accuracy}
      />
      <RatingMeter
        label={t('settings.model_rating_speed', { defaultValue: '速度' })}
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
            defaultValue: `正在下载 (${Math.round(status.progress)}%)`,
          })}
        </span>
      );
    case 'installed':
      return (
        <span className="model-status-chip model-status-installed">
          <CheckIcon />
          {t('settings.model_status_installed', { defaultValue: '已安装' })}
        </span>
      );
    case 'partial':
      return (
        <span className="model-status-chip model-status-installed">
          <CheckIcon />
          {t('settings.model_status_partial_installed', {
            installed: status.installed,
            total: status.total,
            defaultValue: '{{installed}}/{{total}} 已安装',
          })}
        </span>
      );
    default:
      return (
        <span className="model-status-chip model-status-not-installed">
          {t('settings.not_installed', { defaultValue: '未安装' })}
        </span>
      );
  }
}

/** Semantic tag cluster: scenario modes, engine, curated labels, recommendation. */
function ModelTags({ model, showEngine }: { model: ModelInfo; showEngine: boolean }) {
  const { t } = useTranslation();
  const labels = resolveModelLabels(model) ?? [];
  const hasModes = (model.modes?.length ?? 0) > 0;
  if (!hasModes && !showEngine && labels.length === 0 && !model.isRecommended) return null;
  return (
    <div className="model-tags">
      {model.modes?.map((mode) => (
        <span key={mode} className="model-tag model-tag-mode">
          {mode.charAt(0).toUpperCase() + mode.slice(1)}
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
        className="model-action-icon"
        onClick={onCancelDownload}
        aria-label={t('common.cancel')}
        data-tooltip={t('common.cancel')}
      >
        <XIcon />
      </button>
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
  actionsDisabled = false,
  isAsr: isAsrProp,
}: ModelCardProps): React.JSX.Element {
  const { t } = useTranslation();

  if (!models || models.length === 0) return <></>;

  const baseModel = models[0];
  const isMultiVersion = models.length > 1;
  const baseDownload = downloads[baseModel.id];

  const isAsr = isAsrProp ?? isAsrModel(baseModel);
  const brand = isAsr ? resolveModelBrand(baseModel) : null;

  const headerStatus: ModelStatus = (() => {
    if (!isMultiVersion) {
      if (baseDownload) return { kind: 'downloading', progress: baseDownload.progress };
      return installedModels.has(baseModel.id) ? { kind: 'installed' } : { kind: 'not-installed' };
    }
    const downloadingModel = models.find((model) => downloads[model.id]);
    if (downloadingModel) {
      return { kind: 'downloading', progress: downloads[downloadingModel.id].progress };
    }
    const installedCount = models.filter((model) => installedModels.has(model.id)).length;
    if (installedCount === 0) return { kind: 'not-installed' };
    if (installedCount === models.length) return { kind: 'installed' };
    return { kind: 'partial', installed: installedCount, total: models.length };
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
              {!isMultiVersion && baseModel.versionLabel ? ` (${baseModel.versionLabel})` : ''}
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
