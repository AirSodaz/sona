import React from 'react';
import { useTranslation } from 'react-i18next';
import { TFunction } from 'i18next';
import { useShallow } from 'zustand/react/shallow';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { BatchQueueItem, BatchQueueItemStatus } from '../types/batchQueue';
import { PendingIcon, ProcessingIcon, CompleteIcon, ErrorIcon, TrashIcon } from './Icons';



/**
 * Returns the appropriate icon for a queue item status.
 */
const getStatusIcon = (status: BatchQueueItemStatus): React.JSX.Element => {
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
};

/** Props for QueueItem component. */
interface QueueItemProps {
    item: BatchQueueItem;
    isActive: boolean;
    onActivate: (id: string) => void;
    onRemove: (id: string) => void;
    t: TFunction;
}

/**
 * Individual queue item component.
 * Memoized to prevent re-renders of the entire list when only one item updates.
 */
function QueueItemComponent({ item, isActive, onActivate, onRemove, t }: QueueItemProps): React.JSX.Element {
    const handleClick = () => {
        onActivate(item.id);
    };

    const handleRemove = (e: React.MouseEvent) => {
        e.stopPropagation();
        onRemove(item.id);
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onActivate(item.id);
        }
    };

    return (
        <div
            className={`queue-item queue-item-${item.status} ${isActive ? 'queue-item-active' : ''}`}
            onClick={handleClick}
            onKeyDown={handleKeyDown}
            role="listitem"
            tabIndex={0}
            aria-current={isActive ? 'true' : undefined}
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
                onClick={handleRemove}
                aria-label={t('common.delete_item', { item: item.filename })}
            >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
            </button>
        </div>
    );
}

const QueueItem = React.memo(QueueItemComponent);

/**
 * Container for QueueItem that handles subscription to specific item updates.
 * This prevents the parent list from re-rendering when a single item updates.
 */
function QueueItemContainer({ id, t }: { id: string; t: TFunction }): React.JSX.Element | null {
    const item = useBatchQueueStore((state) => state.queueItems.find((i) => i.id === id));
    // Subscribe to active state for this item
    const isActive = useBatchQueueStore((state) => state.activeItemId === id);
    const setActiveItem = useBatchQueueStore((state) => state.setActiveItem);
    const removeItem = useBatchQueueStore((state) => state.removeItem);

    if (!item) return null;

    return (
        <QueueItem
            item={item}
            isActive={isActive}
            onActivate={setActiveItem}
            onRemove={removeItem}
            t={t}
        />
    );
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

    // OPTIMIZATION: Subscribe only to the list of IDs using useShallow.
    // This ensures the sidebar component ONLY re-renders when items are added, removed, or reordered,
    // but NOT when an item's progress/status updates (which happens frequently).
    const itemIds = useBatchQueueStore(
        useShallow((state) => state.queueItems.map((i) => i.id))
    );

    const clearQueue = useBatchQueueStore((state) => state.clearQueue);

    if (itemIds.length === 0) {
        return null;
    }

    return (
        <div className={`file-queue-sidebar ${className}`}>
            <div className="queue-header">
                <span className="queue-title">{t('batch.queue_title', { count: itemIds.length })}</span>
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

            <div className="queue-list" role="list" aria-label={t('batch.queue_title', { count: itemIds.length })}>
                {itemIds.map((id) => (
                    <QueueItemContainer
                        key={id}
                        id={id}
                        t={t}
                    />
                ))}
            </div>
        </div>
    );
};

export default FileQueueSidebar;
