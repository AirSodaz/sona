import React from 'react';
import { useTranslation } from 'react-i18next';
import { ModelInfo } from '../../services/modelService';
import { TrashIcon, DownloadIcon, XIcon } from '../Icons';

interface ModelCardProps {
    models: ModelInfo[];
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

interface ModelCardActionsProps {
    model: ModelInfo;
    isInstalled: boolean;
    isDownloading: boolean;
    // downloadingId: string | null;
    // deletingId: string | null;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: () => void;
}

function ModelCardActions({
    model,
    isInstalled,
    isDownloading,
    // downloadingId,
    // deletingId,
    onDelete,
    onDownload,
    onCancelDownload
}: ModelCardActionsProps): React.JSX.Element {
    const { t } = useTranslation();
    // const isDownloading = downloadingId === model.id;
    // const isDeleting = deletingId === model.id;
    const isDeleting = false; // Simplified for now, we can add isDeleting prop later if needed per model

    if (isInstalled) {
        return (
            <div className="model-actions">
                <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => onDelete(model)}
                    disabled={false} // Allow to delete even if something else is happening
                    aria-label={`${t('common.delete')} ${model.name}`}
                    data-tooltip={t('common.delete')}
                >
                    {isDeleting ? <div className="spinner" /> : <TrashIcon />}
                </button>
            </div>
        );
    }

    return (
        <button
            className="btn btn-secondary btn-sm"
            onClick={isDownloading ? onCancelDownload : () => onDownload(model)}
            // disabled={!!downloadingId && !isDownloading} // Allow parallel downloads
            aria-label={isDownloading ? t('common.cancel') : `${t('common.download')} ${model.name}`}
            data-tooltip={isDownloading ? t('common.cancel') : t('common.download')}
        >
            {isDownloading ? <XIcon /> : <DownloadIcon />}
        </button>
    );
}

export function ModelCard({
    models,
    installedModels,
    downloads,
    onDelete,
    onDownload,
    onCancelDownload
}: ModelCardProps): React.JSX.Element {
    const { t } = useTranslation();

    if (!models || models.length === 0) return <></>;

    const baseModel = models[0];
    const isMultiVersion = models.length > 1;

    return (
        <div className="model-card">
            <div className="model-card-header" style={{ alignItems: 'flex-start', marginBottom: isMultiVersion ? '16px' : '0' }}>
                <div style={{ flex: 1, width: '100%' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <div className="model-name">{baseModel.name}{!isMultiVersion && baseModel.versionLabel ? ` (${baseModel.versionLabel})` : ''}</div>
                        <div className="model-tags" style={{ marginTop: '0' }}>
                            <span className="model-tag">{baseModel.language.toUpperCase()}</span>
                            {baseModel.modes && baseModel.modes.length > 0 && (
                                <span className="model-tag">
                                    {baseModel.modes.map(mode => mode.charAt(0).toUpperCase() + mode.slice(1)).join(',')}
                                </span>
                            )}
                            {baseModel.isRecommended && <span className="model-tag">{t('common.recommended')}</span>}
                        </div>
                    </div>
                    <div className="model-description">{t(baseModel.description)}</div>
                    {!isMultiVersion && (
                        <div style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: '12px', marginTop: '12px' }}>
                            <span style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>{baseModel.size}</span>
                            <ModelCardActions
                                model={baseModel}
                                isInstalled={installedModels.has(baseModel.id)}
                                isDownloading={!!downloads[baseModel.id]}
                                onDelete={onDelete}
                                onDownload={onDownload}
                                onCancelDownload={() => onCancelDownload(baseModel.id)}
                            />
                        </div>
                    )}
                </div>
            </div>

            {!isMultiVersion && !!downloads[baseModel.id] && (
                <div className="progress-container-mini">
                    <div className="progress-info-mini" aria-live="polite">
                        <span style={{ color: 'var(--color-text-muted)', fontSize: '0.75rem' }}>{downloads[baseModel.id].status || t('common.loading')}</span>
                        <span>{Math.round(downloads[baseModel.id].progress)}%</span>
                    </div>
                    <div
                        className="progress-bar-mini"
                        role="progressbar"
                        aria-valuenow={Math.round(downloads[baseModel.id].progress)}
                        aria-valuemin={0}
                        aria-valuemax={100}
                        aria-label={`${t('common.download')} ${baseModel.name}`}
                    >
                        <div className="progress-fill" style={{ width: `${downloads[baseModel.id].progress}%` }} />
                    </div>
                </div>
            )}

            {isMultiVersion && (
                <>
                    <hr style={{ border: 'none', borderTop: '1px solid var(--color-border)', margin: '0 0 16px 0' }} />
                    <div className="model-versions" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                        {models.map(model => {
                            const isInstalled = installedModels.has(model.id);
                            const downloadState = downloads[model.id];
                            const isDownloading = !!downloadState;

                            return (
                                <div key={model.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                                            <span style={{ fontWeight: 500, color: 'var(--color-text)' }}>{model.versionLabel || model.name}</span>
                                        </div>
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                                            <span style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>{model.size}</span>
                                            <ModelCardActions
                                                model={model}
                                                isInstalled={isInstalled}
                                                isDownloading={isDownloading}
                                                onDelete={onDelete}
                                                onDownload={onDownload}
                                                onCancelDownload={() => onCancelDownload(model.id)}
                                            />
                                        </div>
                                    </div>
                                    {isDownloading && (
                                        <div className="progress-container-mini" style={{ marginTop: 0 }}>
                                            <div className="progress-info-mini" aria-live="polite">
                                                <span style={{ color: 'var(--color-text-muted)', fontSize: '0.75rem' }}>{downloadState.status || t('common.loading')}</span>
                                                <span>{Math.round(downloadState.progress)}%</span>
                                            </div>
                                            <div
                                                className="progress-bar-mini"
                                                role="progressbar"
                                                aria-valuenow={Math.round(downloadState.progress)}
                                                aria-valuemin={0}
                                                aria-valuemax={100}
                                                aria-label={`${t('common.download')} ${model.name}`}
                                            >
                                                <div className="progress-fill" style={{ width: `${downloadState.progress}%` }} />
                                            </div>
                                        </div>
                                    )}
                                </div>
                            );
                        })}
                    </div>
                </>
            )}
        </div>
    );
}
