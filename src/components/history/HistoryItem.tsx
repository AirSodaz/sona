import React from 'react';
import { useTranslation } from 'react-i18next';
import { Calendar, Clock } from 'lucide-react';
import { HistoryItem as HistoryItemType } from '../../types/history';
import { TrashIcon, MicIcon, FileTextIcon } from '../Icons';

interface HistoryItemProps {
    item: HistoryItemType;
    onLoad: (item: HistoryItemType) => void;
    onDelete: (e: React.MouseEvent, id: string) => void;
    searchQuery?: string;
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

const HistoryItemComponent = ({ item, onLoad, onDelete, searchQuery = '' }: HistoryItemProps) => {
    const { t } = useTranslation();

    return (
        <div
            className="history-item"
            onClick={() => onLoad(item)}
            onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    onLoad(item);
                }
            }}
            role="button"
            tabIndex={0}
            aria-label={`${t('common.load', { defaultValue: 'Load' })} ${item.title}`}
        >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 'var(--spacing-xs)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)' }}>
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
                <button
                    className="btn btn-icon delete-btn"
                    onClick={(e) => onDelete(e, item.id)}
                    aria-label={t('common.delete_item', { item: item.title, defaultValue: `Delete ${item.title}` })}
                    data-tooltip={t('history.delete_tooltip', { defaultValue: 'Delete' })}
                    data-tooltip-pos="left"
                >
                    <TrashIcon />
                </button>
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
                margin: 0
            }}>
                {item.previewText ? highlightText(item.previewText, searchQuery) : <em>{t('history.no_transcript')}</em>}
            </p>
        </div>
    );
};

export const HistoryItem = React.memo(HistoryItemComponent);
