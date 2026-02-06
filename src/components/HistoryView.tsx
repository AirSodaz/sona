import React, { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from '../services/historyService';
import { Calendar, Clock, Search } from 'lucide-react';
import {
    TrashIcon,
    MicIcon,
    FileTextIcon
} from './Icons';
import { Dropdown } from './Dropdown';
import { useDialogStore } from '../stores/dialogStore';

type FilterType = 'all' | 'recording' | 'batch';
type DateFilter = 'all' | 'today' | 'week' | 'month';

export function HistoryView() {
    const { t } = useTranslation();

    // Store State
    const items = useHistoryStore((state) => state.items);
    const isLoading = useHistoryStore((state) => state.isLoading);
    const deleteItem = useHistoryStore((state) => state.deleteItem);

    // Actions
    const setSegments = useTranscriptStore((state) => state.setSegments);
    const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
    const confirm = useDialogStore((state) => state.confirm);

    // Local UI State
    const [searchQuery, setSearchQuery] = useState('');
    const [filterType, setFilterType] = useState<FilterType>('all');
    const [dateFilter, setDateFilter] = useState<DateFilter>('all');

    useEffect(() => {
        useHistoryStore.getState().loadItems();
    }, []);

    // Filter Logic
    const filteredItems = useMemo(() => {
        return items.filter(item => {
            // 1. Search Query
            if (searchQuery) {
                const query = searchQuery.toLowerCase();
                const titleMatch = item.title.toLowerCase().includes(query);
                const contentMatch = item.searchContent?.toLowerCase().includes(query) ||
                    item.previewText.toLowerCase().includes(query); // Fallback to preview

                if (!titleMatch && !contentMatch) return false;
            }

            // 2. Type Filter
            if (filterType !== 'all') {
                // If item.type is undefined, assume 'recording' for now or handle as 'unknown'
                // Let's assume undefined == 'recording' for legacy items compatibility
                const itemType = item.type || 'recording';
                if (itemType !== filterType) return false;
            }

            // 3. Date Filter
            if (dateFilter !== 'all') {
                const itemDate = new Date(item.timestamp);
                const now = new Date();
                const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

                if (dateFilter === 'today') {
                    if (itemDate < today) return false;
                } else if (dateFilter === 'week') {
                    const weekAgo = new Date(today);
                    weekAgo.setDate(weekAgo.getDate() - 7);
                    if (itemDate < weekAgo) return false;
                } else if (dateFilter === 'month') {
                    const monthAgo = new Date(today);
                    monthAgo.setMonth(monthAgo.getMonth() - 1);
                    if (itemDate < monthAgo) return false;
                }
            }

            return true;
        });
    }, [items, searchQuery, filterType, dateFilter]);

    const handleLoad = async (item: any) => {
        try {
            // Load Transcript
            const segments = await historyService.loadTranscript(item.transcriptPath);
            setSegments(segments);

            // Load Audio
            const url = await historyService.getAudioUrl(item.audioPath);
            setAudioUrl(url);

        } catch (error) {
            console.error('Failed to load item:', error);
        }
    };

    const handleDelete = async (e: React.MouseEvent, id: string) => {
        e.stopPropagation();

        const confirmed = await confirm(t('history.delete_confirm'), {
            title: t('history.delete_title', { defaultValue: 'Delete History' }),
            confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
            variant: 'error'
        });

        if (confirmed) {
            await deleteItem(id);
        }
    };

    function formatDuration(seconds: number): string {
        const mins = Math.floor(seconds / 60);
        const secs = Math.floor(seconds % 60);
        return `${mins}:${secs.toString().padStart(2, '0')}`;
    }

    function formatDate(timestamp: number): string {
        return new Date(timestamp).toLocaleDateString() + ' ' + new Date(timestamp).toLocaleTimeString();
    }

    return (
        <div className="panel-container" style={{ height: '100%', flexDirection: 'column', background: 'var(--color-bg-primary)' }}>
            {/* Search and Filters Header */}
            <div style={{ padding: 'var(--spacing-md)', borderBottom: '1px solid var(--color-border)', background: 'var(--color-bg-secondary)' }}>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-md)' }}>
                    {/* Search Bar */}
                    <div style={{ position: 'relative' }}>
                        <Search size={16} style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', color: 'var(--color-text-muted)' }} />
                        <input
                            type="text"
                            placeholder={t('history.search_placeholder', { defaultValue: 'Search history...' })}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                            style={{ paddingLeft: '32px', width: '100%' }}
                        />
                    </div>

                    {/* Filters */}
                    <div style={{ display: 'flex', gap: 'var(--spacing-md)' }}>
                        <div style={{ flex: 1 }}>
                            <Dropdown
                                value={filterType}
                                onChange={(val) => setFilterType(val as FilterType)}
                                options={[
                                    { value: 'all', label: t('history.filter_all', { defaultValue: 'All Types' }) },
                                    { value: 'recording', label: t('history.filter_recordings', { defaultValue: 'Recordings' }) },
                                    { value: 'batch', label: t('history.filter_batch', { defaultValue: 'Batch Imports' }) }
                                ]}
                                style={{ width: '100%' }}
                            />
                        </div>
                        <div style={{ flex: 1 }}>
                            <Dropdown
                                value={dateFilter}
                                onChange={(val) => setDateFilter(val as DateFilter)}
                                options={[
                                    { value: 'all', label: t('history.date_all', { defaultValue: 'Any Time' }) },
                                    { value: 'today', label: t('history.date_today', { defaultValue: 'Today' }) },
                                    { value: 'week', label: t('history.date_week', { defaultValue: 'Last 7 Days' }) },
                                    { value: 'month', label: t('history.date_month', { defaultValue: 'Last 30 Days' }) }
                                ]}
                                style={{ width: '100%' }}
                            />
                        </div>
                    </div>
                </div>
            </div>

            {/* List */}
            <div style={{ flex: 1, overflowY: 'auto', padding: 'var(--spacing-md)' }}>
                {isLoading && <div style={{ textAlign: 'center', padding: 'var(--spacing-xl)', color: 'var(--color-text-muted)' }}>{t('history.loading')}</div>}

                {!isLoading && filteredItems.length === 0 && (
                    <div style={{ textAlign: 'center', padding: 'var(--spacing-xl)', color: 'var(--color-text-muted)' }}>
                        <p>{items.length === 0 ? t('history.empty') : t('history.no_results', { defaultValue: 'No results found' })}</p>
                    </div>
                )}

                {filteredItems.map((item) => (
                    <div
                        key={item.id}
                        onClick={() => handleLoad(item)}
                        style={{
                            background: 'var(--color-bg-elevated)',
                            border: '1px solid var(--color-border)',
                            borderRadius: 'var(--radius-md)',
                            padding: 'var(--spacing-md)',
                            marginBottom: 'var(--spacing-sm)',
                            cursor: 'pointer',
                            transition: 'all var(--transition-fast)'
                        }}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.borderColor = 'var(--color-border-hover)';
                            e.currentTarget.style.boxShadow = 'var(--shadow-sm)';
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.borderColor = 'var(--color-border)';
                            e.currentTarget.style.boxShadow = 'none';
                        }}
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
                                <span style={{ fontWeight: 600, color: 'var(--color-text-primary)' }}>{item.title}</span>
                            </div>
                            <button
                                className="btn btn-icon"
                                onClick={(e) => handleDelete(e, item.id)}
                                title={t('history.delete_tooltip')}
                                style={{ padding: '4px', height: 'auto', color: 'var(--color-text-muted)' }}
                                onMouseEnter={(e) => e.currentTarget.style.color = 'var(--color-error)'}
                                onMouseLeave={(e) => e.currentTarget.style.color = 'var(--color-text-muted)'}
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
                            {item.previewText || <em>{t('history.no_transcript')}</em>}
                        </p>
                    </div>
                ))}
            </div>
        </div>
    );
}

