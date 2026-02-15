import React, { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from '../services/historyService';
import { Search } from 'lucide-react';
import { Dropdown } from './Dropdown';
import { CloseIcon } from './Icons';
import { Virtuoso } from 'react-virtuoso';
import { HistoryItem } from './history/HistoryItem';
import { useDialogStore } from '../stores/dialogStore';

type FilterType = 'all' | 'recording' | 'batch';
type DateFilter = 'all' | 'today' | 'week' | 'month';

export function HistoryView() {
    const { t } = useTranslation();

    // Store State
    const items = useHistoryStore((state) => state.items);
    const isLoading = useHistoryStore((state) => state.isLoading);
    const deleteItem = useHistoryStore((state) => state.deleteItem);
    const deleteItems = useHistoryStore((state) => state.deleteItems);

    // Actions
    const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
    const confirm = useDialogStore((state) => state.confirm);

    // Local UI State
    const [searchQuery, setSearchQuery] = useState('');
    const [filterType, setFilterType] = useState<FilterType>('all');
    const [dateFilter, setDateFilter] = useState<DateFilter>('all');
    const [isSelectionMode, setIsSelectionMode] = useState(false);
    const [selectedIds, setSelectedIds] = useState<string[]>([]);

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
            // Use atomic load to prevent auto-save from seeing mixed state (new segments + old ID)
            useTranscriptStore.getState().loadTranscript(segments, item.id);

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

    // Selection Logic
    const toggleSelectionMode = () => {
        setIsSelectionMode(!isSelectionMode);
        setSelectedIds([]);
    };

    const toggleSelection = (id: string) => {
        setSelectedIds(prev =>
            prev.includes(id)
                ? prev.filter(i => i !== id)
                : [...prev, id]
        );
    };

    const handleSelectAll = () => {
        if (selectedIds.length === filteredItems.length) {
            setSelectedIds([]);
        } else {
            setSelectedIds(filteredItems.map(i => i.id));
        }
    };

    const handleBulkDelete = async () => {
        if (selectedIds.length === 0) return;

        const confirmed = await confirm(
            t('history.delete_bulk_confirm', { count: selectedIds.length, defaultValue: `Are you sure you want to delete ${selectedIds.length} items?` }),
            {
                title: t('history.delete_title', { defaultValue: 'Delete History' }),
                confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
                variant: 'error'
            }
        );

        if (confirmed) {
            await deleteItems(selectedIds);
            setSelectedIds([]);
            setIsSelectionMode(false);
        }
    };

    return (
        <div className="panel-container" style={{ height: '100%', flexDirection: 'column', background: 'var(--color-bg-primary)' }}>
            {/* Search and Filters Header */}
            <div style={{ padding: 'var(--spacing-md)', borderBottom: '1px solid var(--color-border)', background: 'var(--color-bg-secondary)' }}>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-md)' }}>
                    {/* Search Bar & Actions */}
                    <div style={{ display: 'flex', gap: 'var(--spacing-md)', alignItems: 'center' }}>
                        <div style={{ position: 'relative', flex: 1 }}>
                            <Search size={16} style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', color: 'var(--color-text-muted)' }} />
                            <input
                                type="text"
                                placeholder={t('history.search_placeholder', { defaultValue: 'Search history...' })}
                                aria-label={t('history.search_placeholder', { defaultValue: 'Search history...' })}
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === 'Escape') {
                                        setSearchQuery('');
                                    }
                                }}
                                style={{
                                    paddingLeft: '32px',
                                    paddingRight: searchQuery ? '32px' : 'var(--spacing-sm)',
                                    width: '100%'
                                }}
                            />
                            {searchQuery && (
                                <button
                                    className="btn btn-icon btn-text"
                                    onClick={() => setSearchQuery('')}
                                    aria-label={t('common.clear_search', { defaultValue: 'Clear search' })}
                                    style={{
                                        position: 'absolute',
                                        right: '4px',
                                        top: '50%',
                                        transform: 'translateY(-50%)',
                                        height: '24px',
                                        width: '24px',
                                        padding: 0,
                                        minWidth: 0,
                                        color: 'var(--color-text-muted)',
                                        display: 'flex',
                                        alignItems: 'center',
                                        justifyContent: 'center'
                                    }}
                                >
                                    <CloseIcon width={14} height={14} />
                                </button>
                            )}
                        </div>
                        <button
                            className={`btn ${isSelectionMode ? 'btn-primary' : 'btn-secondary'}`}
                            onClick={toggleSelectionMode}
                            style={{ whiteSpace: 'nowrap' }}
                        >
                            {isSelectionMode ? t('common.cancel', { defaultValue: 'Cancel' }) : t('common.select', { defaultValue: 'Select' })}
                        </button>
                    </div>

                    {/* Filters or Selection Actions */}
                    {!isSelectionMode ? (
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
                    ) : (
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', background: 'var(--color-bg-elevated)', padding: 'var(--spacing-sm)', borderRadius: 'var(--radius-sm)', border: '1px solid var(--color-border)' }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-md)' }}>
                                <button
                                    className="btn btn-secondary"
                                    onClick={handleSelectAll}
                                    style={{ padding: '4px 8px', fontSize: '0.8rem' }}
                                >
                                    {selectedIds.length === filteredItems.length ? t('common.deselect_all', { defaultValue: 'Deselect All' }) : t('common.select_all', { defaultValue: 'Select All' })}
                                </button>
                                <span style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>
                                    {t('history.selected_count', { count: selectedIds.length, defaultValue: `${selectedIds.length} selected` })}
                                </span>
                            </div>

                            {selectedIds.length > 0 && (
                                <button
                                    className="btn btn-danger"
                                    onClick={handleBulkDelete}
                                    style={{ padding: '4px 12px', fontSize: '0.8rem' }}
                                >
                                    {t('common.delete', { defaultValue: 'Delete' })}
                                </button>
                            )}
                        </div>
                    )}
                </div>
            </div>

            {/* List */}
            <div style={{ flex: 1, overflow: 'hidden' }}>
                {isLoading && <div style={{ textAlign: 'center', padding: 'var(--spacing-xl)', color: 'var(--color-text-muted)' }}>{t('history.loading')}</div>}

                {!isLoading && filteredItems.length === 0 && (
                    <div style={{ textAlign: 'center', padding: 'var(--spacing-xl)', color: 'var(--color-text-muted)' }}>
                        <p>{items.length === 0 ? t('history.empty') : t('history.no_results', { defaultValue: 'No results found' })}</p>
                    </div>
                )}

                {!isLoading && filteredItems.length > 0 && (
                    <Virtuoso
                        style={{ height: '100%' }}
                        data={filteredItems}
                        itemContent={(_index, item) => (
                            <HistoryItem
                                item={item}
                                onLoad={handleLoad}
                                onDelete={handleDelete}
                                searchQuery={searchQuery}
                                isSelectionMode={isSelectionMode}
                                isSelected={selectedIds.includes(item.id)}
                                onToggleSelection={toggleSelection}
                            />
                        )}
                        components={{
                            List: React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>((props, ref) => (
                                <div
                                    {...props}
                                    ref={ref}
                                    style={{
                                        ...props.style,
                                        paddingLeft: 'var(--spacing-md)',
                                        paddingRight: 'var(--spacing-md)',
                                        // paddingTop removed to avoid breaking virtualization
                                    }}
                                />
                            )),
                            Header: () => <div style={{ height: 'var(--spacing-md)' }} />,
                            Footer: () => <div style={{ height: 'var(--spacing-md)' }} />
                        }}
                    />
                )}
            </div>
        </div>
    );
}

