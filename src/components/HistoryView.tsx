import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from '../services/historyService';
import { Trash2, Calendar, Clock } from 'lucide-react';
import { useDialogStore } from '../stores/dialogStore';

export function HistoryView() {
    const { t } = useTranslation();
    const items = useHistoryStore((state) => state.items);
    const isLoading = useHistoryStore((state) => state.isLoading);
    const deleteItem = useHistoryStore((state) => state.deleteItem);
    const setSegments = useTranscriptStore((state) => state.setSegments);
    const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
    const confirm = useDialogStore((state) => state.confirm);

    useEffect(() => {
        useHistoryStore.getState().loadItems();
    }, []);

    const handleLoad = async (item: any) => {
        try {
            // Load Transcript
            const segments = await historyService.loadTranscript(item.transcriptPath);
            setSegments(segments);

            // Load Audio
            const url = await historyService.getAudioUrl(item.audioPath);
            setAudioUrl(url);

            // Note: In History mode, we might want to stay in 'history' mode but 'active' state?
            // Or maybe we use 'history' mode just for the list, and when playing it acts like batch/loaded file?
            // Actually, let's keep it simple: We load it, and the right panel (TranscriptEditor) shows it.
            // The left panel (HistoryView) stays open.

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
        <div className="history-view">
            <div className="history-list">
                {isLoading && <div className="loading">{t('history.loading')}</div>}

                {!isLoading && items.length === 0 && (
                    <div className="empty-state">
                        <p>{t('history.empty')}</p>
                    </div>
                )}

                {items.map((item) => (
                    <div key={item.id} className="history-item" onClick={() => handleLoad(item)}>
                        <div className="history-item-header">
                            <span className="history-title">{item.title}</span>
                            <button
                                className="delete-btn"
                                onClick={(e) => handleDelete(e, item.id)}
                                title={t('history.delete_tooltip')}
                            >
                                <Trash2 size={16} />
                            </button>
                        </div>

                        <div className="history-meta">
                            <span className="meta-item">
                                <Calendar size={12} />
                                {formatDate(item.timestamp)}
                            </span>
                            <span className="meta-item">
                                <Clock size={12} />
                                {formatDuration(item.duration)}
                            </span>
                        </div>

                        <p className="history-preview">
                            {item.previewText || <em>{t('history.no_transcript')}</em>}
                        </p>
                    </div>
                ))}
            </div>

            <style>{`
                .history-view {
                    display: flex;
                    flex-direction: column;
                    height: 100%;
                    overflow-y: auto;
                    padding: 1rem;
                }
                .history-item {
                    background: var(--bg-secondary);
                    border: 1px solid var(--border-color);
                    border-radius: 8px;
                    padding: 12px;
                    margin-bottom: 12px;
                    cursor: pointer;
                    transition: all 0.2s;
                }
                .history-item:hover {
                    background: var(--bg-hover);
                    border-color: var(--primary-color);
                }
                .history-item-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 8px;
                }
                .history-title {
                    font-weight: 600;
                    font-size: 0.95rem;
                }
                .history-meta {
                    display: flex;
                    gap: 12px;
                    font-size: 0.8rem;
                    color: var(--text-secondary);
                    margin-bottom: 8px;
                }
                .meta-item {
                    display: flex;
                    align-items: center;
                    gap: 4px;
                }
                .history-preview {
                    font-size: 0.85rem;
                    color: var(--text-secondary);
                    display: -webkit-box;
                    -webkit-line-clamp: 2;
                    -webkit-box-orient: vertical;
                    overflow: hidden;
                    margin: 0;
                }
                .delete-btn {
                    background: none;
                    border: none;
                    color: var(--text-secondary);
                    cursor: pointer;
                    padding: 4px;
                    border-radius: 4px;
                }
                .delete-btn:hover {
                    color: var(--error-color);
                    background: var(--bg-active);
                }
            `}</style>
        </div>
    );
}
