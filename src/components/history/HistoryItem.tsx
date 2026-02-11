import React from 'react';
import { useTranslation } from 'react-i18next';
import { Calendar, Clock } from 'lucide-react';
import { HistoryItem as HistoryItemType } from '../../types/history';
import { TrashIcon, MicIcon, FileTextIcon } from '../Icons';

import { Checkbox } from '../Checkbox';

interface HistoryItemProps {
    item: HistoryItemType;
    onLoad: (item: HistoryItemType) => void;
    onDelete: (e: React.MouseEvent, id: string) => void;
    searchQuery?: string;
    isSelectionMode?: boolean;
    isSelected?: boolean;
    onToggleSelection?: (id: string) => void;
}

/**
 * Highlights matching text by wrapping matches in <mark> tags
 */
function highlightText(text: string, query: string): React.ReactNode {
    if (!query || !query.trim()) {
        return text;
    }

    const escapedQuery = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const regex = new RegExp(`(${escapedQuery})`, 'gi');
    const parts = text.split(regex);

    return parts.map((part, index) =>
        regex.test(part) ? (
            <mark key={index} className="search-highlight">{part}</mark>
        ) : (
            part
        )
    );
}

function formatDuration(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
}

function formatDate(timestamp: number): string {
    return new Date(timestamp).toLocaleDateString() + ' ' + new Date(timestamp).toLocaleTimeString();
}

const HistoryItemComponent = ({
    item,
    onLoad,
    onDelete,
    searchQuery = '',
    isSelectionMode = false,
    isSelected = false,
    onToggleSelection
}: HistoryItemProps) => {
    const { t } = useTranslation();

    const handleClick = (e: React.MouseEvent) => {
        if (isSelectionMode && onToggleSelection) {
            e.preventDefault();
            onToggleSelection(item.id);
        } else {
            onLoad(item);
        }
    };

    return (
        <div
            className={`history-item ${isSelected ? 'selected' : ''}`}
            style={{
                position: 'relative',
                display: 'flex',
                gap: 'var(--spacing-md)',
                alignItems: 'flex-start',
                paddingLeft: isSelectionMode ? 'var(--spacing-sm)' : undefined
            }}
            onClick={isSelectionMode ? handleClick : undefined}
        >
            {isSelectionMode && (
                <div style={{ paddingTop: 'var(--spacing-sm)' }}>
                    <Checkbox
                        checked={isSelected}
                        onChange={() => onToggleSelection?.(item.id)}
                    />
                </div>
            )}

            <button
                className="history-item-content"
                onClick={handleClick}
                aria-label={`${t('common.load', { defaultValue: 'Load' })} ${item.title}`}
                style={{
                    flex: 1,
                    textAlign: 'left',
                    background: 'transparent',
                    border: 'none',
                    padding: 0,
                    margin: 0,
                    cursor: 'pointer',
                    display: 'flex',
                    flexDirection: 'column',
                    fontFamily: 'inherit',
                    color: 'inherit'
                }}
            >
                <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', marginBottom: 'var(--spacing-xs)', paddingRight: isSelectionMode ? 0 : '40px' }}>
                    {item.type === 'batch' ? (
                        <span title="Batch Import" style={{ color: 'var(--color-text-tertiary)' }}>
                            <FileTextIcon />
                        </span>
                    ) : (
                        <span title="Recording" style={{ color: 'var(--color-text-tertiary)' }}>
                            <MicIcon />
                        </span>
                    )}
                    <span style={{ fontWeight: 600, color: 'var(--color-text-primary)' }}>{highlightText(item.title, searchQuery)}</span>
                </div>

                <div style={{ display: 'flex', gap: 'var(--spacing-md)', fontSize: '0.8rem', color: 'var(--color-text-tertiary)', marginBottom: 'var(--spacing-sm)' }}>
                    <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <Calendar size={12} />
                        {formatDate(item.timestamp)}
                    </span>
                    <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <Clock size={12} />
                        {formatDuration(item.duration)}
                    </span>
                </div>

                <p style={{
                    fontSize: '0.875rem',
                    color: 'var(--color-text-secondary)',
                    lineHeight: 1.5,
                    display: '-webkit-box',
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: 'vertical',
                    overflow: 'hidden',
                    margin: 0,
                    textAlign: 'left',
                    width: '100%'
                }}>
                    {item.previewText ? highlightText(item.previewText, searchQuery) : <em>{t('history.no_transcript')}</em>}
                </p>
            </button>

            {!isSelectionMode && (
                <button
                    className="btn btn-icon delete-btn"
                    onClick={(e) => onDelete(e, item.id)}
                    aria-label={t('common.delete_item', { item: item.title, defaultValue: `Delete ${item.title}` })}
                    data-tooltip={t('history.delete_tooltip', { defaultValue: 'Delete' })}
                    data-tooltip-pos="left"
                    style={{
                        position: 'absolute',
                        top: 'var(--spacing-md)',
                        right: 'var(--spacing-md)'
                    }}
                >
                    <TrashIcon />
                </button>
            )}
        </div>
    );
};

export const HistoryItem = React.memo(HistoryItemComponent);
