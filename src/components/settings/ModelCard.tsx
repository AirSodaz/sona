import React from 'react';
import { useTranslation } from 'react-i18next';
import { ModelInfo } from '../../services/modelService';
import { CheckIcon, PlayIcon, TrashIcon, DownloadIcon, XIcon } from '../Icons';

interface ModelCardProps {
    model: ModelInfo;
    isInstalled: boolean;
    isSelected: boolean;
    isDownloading: boolean;
    // downloadingId: string | null;
    // deletingId: string | null;
    progress: number;
    statusMessage: string;
    onLoad: (model: ModelInfo) => void;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: () => void;
}

interface ModelCardActionsProps {
    model: ModelInfo;
    isInstalled: boolean;
    isSelected: boolean;
    isDownloading: boolean;
    // downloadingId: string | null;
    // deletingId: string | null;
    onLoad: (model: ModelInfo) => void;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: () => void;
}

function ModelCardActions({
    model,
    isInstalled,
    isSelected,
    isDownloading,
    // downloadingId,
    // deletingId,
    onLoad,
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
                    className={`btn ${isSelected ? 'btn-success' : 'btn-primary'}`}
                    onClick={() => onLoad(model)}
                    disabled={isSelected}
                    aria-label={`${t('settings.load')} ${model.name}`}
                >
                    {isSelected ? <CheckIcon /> : <PlayIcon />}
                </button>
                <button
                    className="btn btn-secondary"
                    onClick={() => onDelete(model)}
                    disabled={false} // Allow delete even if something else is happening
                    aria-label={`${t('common.delete')} ${model.name}`}
                >
                    {isDeleting ? <div className="spinner" /> : <TrashIcon />}
                </button>
            </div>
        );
    }

    return (
        <button
            className="btn btn-secondary"
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
    model,
    isInstalled,
    isSelected,
    isDownloading,
    // downloadingId,
    // deletingId,
    progress,
    statusMessage,
    onLoad,
    onDelete,
    onDownload,
    onCancelDownload
}: ModelCardProps): React.JSX.Element {
    const { t } = useTranslation();
    // const isDownloading = downloadingId === model.id;

    return (
        <div className="model-card">
            <div className="model-card-header">
                <div>
                    <div className="model-name">{model.name}</div>
                    <div className="model-description">{model.description}</div>
                    <div className="model-tags">
                        <span className="model-tag">{model.language.toUpperCase()}</span>
                        <span className="model-tag">{model.engine.toUpperCase()}</span>
                        <span className="model-tag">{model.size}</span>
                    </div>
                </div>
                <ModelCardActions
                    model={model}
                    isInstalled={isInstalled}
                    isSelected={isSelected}
                    isDownloading={isDownloading}
                    // downloadingId={downloadingId}
                    // deletingId={deletingId}
                    onLoad={onLoad}
                    onDelete={onDelete}
                    onDownload={onDownload}
                    onCancelDownload={onCancelDownload}
                />
            </div>
            {isDownloading && (
                <div className="progress-container-mini">
                    <div className="progress-info-mini" aria-live="polite">
                        <span style={{ color: 'var(--color-text-muted)', fontSize: '0.75rem' }}>{statusMessage || t('common.loading')}</span>
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
            )}
        </div>
    );
}
