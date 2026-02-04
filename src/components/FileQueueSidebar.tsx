import React from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { BatchQueueItemStatus } from '../types/batchQueue';
import { PendingIcon, ProcessingIcon, CompleteIcon, ErrorIcon, TrashIcon } from './Icons';



/**
 * Returns the appropriate icon for a queue item status.
 */
function getStatusIcon(status: BatchQueueItemStatus): React.JSX.Element {
    switch (status) {
        case 'pending':
            return <PendingIcon />;
        case 'processing':
            return <ProcessingIcon />;
        case 'complete':
            return <CompleteIcon />;
        case 'error':
            return <ErrorIcon />;
        default:
            return <PendingIcon />;
    }
}

/** Props for FileQueueSidebar. */
interface FileQueueSidebarProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Sidebar component displaying the batch transcription queue.
 * Shows file list with status, progress, and allows selection.
 *
 * @param props - Component props.
 * @return The file queue sidebar element.
 */
export function FileQueueSidebar({ className = '' }: FileQueueSidebarProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const queueItems = useBatchQueueStore((state) => state.queueItems);
    const activeItemId = useBatchQueueStore((state) => state.activeItemId);
    const setActiveItem = useBatchQueueStore((state) => state.setActiveItem);
    const removeItem = useBatchQueueStore((state) => state.removeItem);
    const clearQueue = useBatchQueueStore((state) => state.clearQueue);

    const handleItemClick = (id: string) => {
        setActiveItem(id);
    };

    const handleRemoveItem = (e: React.MouseEvent, id: string) => {
        e.stopPropagation();
        removeItem(id);
    };

    const handleKeyDown = (e: React.KeyboardEvent, id: string) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setActiveItem(id);
        }
    };

    if (queueItems.length === 0) {
        return null;
    }

    return (
        <div className={`file-queue-sidebar ${className}`}>
            <div className="queue-header">
                <span className="queue-title">{t('batch.queue_title', { count: queueItems.length })}</span>
                <button
                    className="btn btn-icon queue-clear-btn"
                    onClick={clearQueue}
                    aria-label={t('batch.clear_queue')}
                    data-tooltip={t('batch.clear_queue')}
                    data-tooltip-pos="left"
                >
                    <TrashIcon />
                </button>
            </div>

            <div className="queue-list" role="list" aria-label={t('batch.queue_title', { count: queueItems.length })}>
                {queueItems.map((item) => (
                    <div
                        key={item.id}
                        className={`queue-item queue-item-${item.status} ${activeItemId === item.id ? 'queue-item-active' : ''}`}
                        onClick={() => handleItemClick(item.id)}
                        onKeyDown={(e) => handleKeyDown(e, item.id)}
                        role="listitem"
                        tabIndex={0}
                        aria-current={activeItemId === item.id ? 'true' : undefined}
                        aria-label={`${item.filename} - ${t(`batch.status_${item.status}`)}`}
                    >
                        <div className="queue-item-icon" aria-hidden="true">
                            {getStatusIcon(item.status)}
                        </div>

                        <div className="queue-item-content">
                            <div className="queue-item-filename" title={item.filename}>
                                {item.filename}
                            </div>

                            {item.status === 'processing' && (
                                <div className="queue-item-progress">
                                    <div
                                        className="queue-item-progress-fill"
                                        style={{ width: `${item.progress}%` }}
                                        role="progressbar"
                                        aria-valuenow={Math.round(item.progress)}
                                        aria-valuemin={0}
                                        aria-valuemax={100}
                                    />
                                </div>
                            )}

                            {item.status === 'error' && item.errorMessage && (
                                <div className="queue-item-error" title={item.errorMessage}>
                                    {t('batch.file_failed')}
                                </div>
                            )}
                        </div>

                        <button
                            className="btn btn-icon queue-item-remove"
                            onClick={(e) => handleRemoveItem(e, item.id)}
                            aria-label={t('common.delete_item', { item: item.filename })}
                        >
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <line x1="18" y1="6" x2="6" y2="18" />
                                <line x1="6" y1="6" x2="18" y2="18" />
                            </svg>
                        </button>
                    </div>
                ))}
            </div>
        </div>
    );
};

export default FileQueueSidebar;
