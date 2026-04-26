import React from 'react';
import { useTranslation } from 'react-i18next';
import { Calendar, Clock } from 'lucide-react';
import { HistoryItem as HistoryItemType } from '../../types/history';
import { useProjectStore } from '../../stores/projectStore';
import { TrashIcon, MicIcon, FileTextIcon, EditIcon, FolderIcon, CodeIcon } from '../Icons';
import { Checkbox } from '../Checkbox';
import type { WorkspaceSearchRange, WorkspaceSearchSnippet } from '../../utils/workspaceSearch';

interface HistoryItemProps {
    item: HistoryItemType;
    onLoad: (item: HistoryItemType) => void;
    onDelete: (e: React.MouseEvent, id: string) => void;
    onRename?: (e: React.MouseEvent, id: string) => void;
    searchQuery?: string;
    searchTitleMatch?: WorkspaceSearchRange | null;
    searchSnippet?: WorkspaceSearchSnippet | null;
    isSelectionMode?: boolean;
    isSelected?: boolean;
    isKeyboardActive?: boolean;
    onToggleSelection?: (id: string) => void;
    layout?: 'list' | 'grid' | 'table';
}

/**
 * Renders an icon based on the icon string or fallback to type default
 */
function renderIcon(icon: string | undefined, type: string | undefined): React.ReactNode {
    if (icon) {
        if (icon.startsWith('system:')) {
            const iconName = icon.replace('system:', '');
            switch (iconName) {
                case 'mic': return <MicIcon />;
                case 'file': return <FileTextIcon />;
                case 'folder': return <FolderIcon />;
                case 'code': return <CodeIcon />;
                default: break;
            }
        } else {
            // Assume it's an emoji
            return <span className="emoji-icon">{icon}</span>;
        }
    }

    // Fallback
    return type === 'batch' ? <FileTextIcon /> : <MicIcon />;
}

function highlightRange(text: string, range?: WorkspaceSearchRange | null): React.ReactNode {
    if (!range) {
        return text;
    }

    const safeStart = Math.max(0, Math.min(range.start, text.length));
    const safeEnd = Math.max(safeStart, Math.min(range.end, text.length));

    if (safeStart === safeEnd) {
        return text;
    }

    return (
        <>
            {text.slice(0, safeStart)}
            <mark className="search-highlight">{text.slice(safeStart, safeEnd)}</mark>
            {text.slice(safeEnd)}
        </>
    );
}

function renderSnippet(snippet?: WorkspaceSearchSnippet | null): React.ReactNode {
    if (!snippet) {
        return null;
    }

    return highlightRange(snippet.text, {
        start: snippet.highlightStart,
        end: snippet.highlightEnd,
    });
}

function formatDuration(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
}

function formatRelativeDate(timestamp: number, locale: string): string {
    const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
    const now = Date.now();
    const diffInSeconds = Math.round((timestamp - now) / 1000);
    const absDiff = Math.abs(diffInSeconds);

    if (absDiff < 60) {
        return rtf.format(diffInSeconds, 'second');
    } else if (absDiff < 3600) {
        return rtf.format(Math.round(diffInSeconds / 60), 'minute');
    } else if (absDiff < 86400) {
        return rtf.format(Math.round(diffInSeconds / 3600), 'hour');
    } else if (absDiff < 604800) {
        return rtf.format(Math.round(diffInSeconds / 86400), 'day');
    } else {
        const date = new Date(timestamp);
        return new Intl.DateTimeFormat(locale, {
            month: 'short',
            day: 'numeric',
            year: date.getFullYear() !== new Date().getFullYear() ? 'numeric' : undefined,
        }).format(date);
    }
}

function HistoryItemComponent({
    item,
    onLoad,
    onDelete,
    onRename,
    searchQuery = '',
    searchTitleMatch = null,
    searchSnippet = null,
    isSelectionMode = false,
    isSelected = false,
    isKeyboardActive = false,
    onToggleSelection,
    layout = 'list',
}: HistoryItemProps): React.JSX.Element {
    const { t, i18n } = useTranslation();
    const projectName = useProjectStore((state) => {
        if (!item.projectId) {
            return t('projects.inbox', { defaultValue: 'Inbox' });
        }
        return state.projects.find((project) => project.id === item.projectId)?.name || t('projects.unknown_project', { defaultValue: 'Unknown Project' });
    });
    const itemTypeLabel = item.type === 'batch'
        ? t('projects.filter_batch', { defaultValue: 'Batch imports' })
        : t('projects.filter_recordings', { defaultValue: 'Recordings' });

    const handleClick = (e: React.MouseEvent) => {
        if (isSelectionMode && onToggleSelection) {
            e.preventDefault();
            e.stopPropagation();
            onToggleSelection(item.id);
        } else {
            onLoad(item);
        }
    };

    return (
        <div
            id={`workspace-search-result-${item.id}`}
            data-history-item-id={item.id}
            className={`history-item history-item--${layout} ${isSelected ? 'selected' : ''} ${isSelectionMode ? 'is-selection-mode' : ''} ${isKeyboardActive ? 'keyboard-active' : ''}`}
            onClick={isSelectionMode ? () => onToggleSelection?.(item.id) : undefined}
            role={layout === 'table' ? 'row' : 'listitem'}
        >
            {isSelectionMode && (
                <div className="history-item-checkbox" role={layout === 'table' ? 'cell' : undefined}>
                    <Checkbox
                        checked={isSelected}
                        onChange={() => onToggleSelection?.(item.id)}
                        aria-label={t('history.select_item', { item: item.title, defaultValue: `Select ${item.title}` })}
                    />
                </div>
            )}

            <button
                type="button"
                className="history-item-content"
                onClick={handleClick}
                aria-label={`${t('common.load', { defaultValue: 'Load' })} ${item.title}`}
                role={layout === 'table' ? 'cell' : undefined}
            >
                <div className="history-item-header">
                    <div className="history-item-title-row">
                        <span className="history-item-type-icon" title={itemTypeLabel}>
                            {renderIcon(item.icon, item.type)}
                        </span>
                        <span className="history-item-title">{highlightRange(item.title, searchTitleMatch)}</span>
                    </div>

                    {layout !== 'table' && (
                        <span className="history-item-project-badge">
                            {projectName}
                        </span>
                    )}
                </div>

                {layout === 'table' && (
                    <div className="history-item-table-cell history-item-table-project" role="cell">
                        <span className="history-item-project-badge">{projectName}</span>
                    </div>
                )}

                {layout === 'table' && searchQuery.trim() && searchSnippet && (
                    <p className="history-item-preview history-item-preview--table">
                        {renderSnippet(searchSnippet)}
                    </p>
                )}

                {layout !== 'table' && (
                    <p className="history-item-preview">
                        {searchQuery.trim() && searchSnippet
                            ? renderSnippet(searchSnippet)
                            : item.previewText
                            ? item.previewText
                            : <em>{t('history.no_transcript')}</em>}
                    </p>
                )}

                <div className={`history-item-meta ${layout === 'table' ? 'history-item-table-cells' : ''}`}>
                    <span className="history-item-meta-chip" role={layout === 'table' ? 'cell' : undefined}>
                        <Calendar size={12} />
                        {formatRelativeDate(item.timestamp, i18n.language)}
                    </span>
                    <span className="history-item-meta-chip" role={layout === 'table' ? 'cell' : undefined}>
                        <Clock size={12} />
                        {formatDuration(item.duration)}
                    </span>
                </div>
            </button>

            {!isSelectionMode && (
                <div className="history-item-actions" role={layout === 'table' ? 'cell' : undefined}>
                    {onRename && (
                        <button
                            type="button"
                            className="btn btn-icon history-item-rename"
                            onClick={(e) => onRename(e, item.id)}
                            aria-label={t('common.rename_item', { item: item.title, defaultValue: `Rename ${item.title}` })}
                            data-tooltip={t('common.rename', { defaultValue: 'Rename' })}
                            data-tooltip-pos="left"
                        >
                            <EditIcon />
                        </button>
                    )}
                    <button
                        type="button"
                        className="btn btn-icon delete-btn history-item-delete"
                        onClick={(e) => onDelete(e, item.id)}
                        aria-label={t('common.delete_item', { item: item.title, defaultValue: `Delete ${item.title}` })}
                        data-tooltip={t('history.delete_tooltip', { defaultValue: 'Delete' })}
                        data-tooltip-pos="left"
                    >
                        <TrashIcon />
                    </button>
                </div>
            )}
        </div>
    );
}

export const HistoryItem = React.memo(HistoryItemComponent);
