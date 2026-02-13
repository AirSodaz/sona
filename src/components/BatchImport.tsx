import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Event } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useDialogStore } from '../stores/dialogStore';
import { FileQueueSidebar } from './FileQueueSidebar';
import { UploadIcon } from './Icons';
import { Dropdown } from './Dropdown';



const SUPPORTED_EXTENSIONS = [
    // Audio
    '.wav', '.mp3', '.m4a', '.aiff', '.flac', '.ogg', '.wma', '.aac', '.opus', '.amr',
    // Video
    '.mp4', '.webm', '.mov', '.mkv', '.avi', '.wmv', '.flv', '.3gp'
];

/** Props for BatchImportOptions component. */
interface BatchImportOptionsProps {
    enableTimeline: boolean;
    setEnableTimeline: (value: boolean) => void;
    language: string;
    setLanguage: (value: string) => void;
}

/**
 * Component for configuring batch import options (timeline, language).
 *
 * @param props Component props.
 * @return The rendered options component.
 */
const BatchImportOptions = React.memo(function BatchImportOptions({ enableTimeline, setEnableTimeline, language, setLanguage }: BatchImportOptionsProps): React.JSX.Element {
    const { t } = useTranslation();
    return (
        <div className="options-container">
            <div className="options-row">
                <div className="options-label">
                    <span>{t('batch.timeline_mode')}</span>
                    <span className="options-hint">{t('batch.timeline_hint')}</span>
                </div>
                <button
                    className="toggle-switch"
                    onClick={() => setEnableTimeline(!enableTimeline)}
                    role="switch"
                    aria-checked={enableTimeline}
                    aria-label={t('batch.timeline_mode')}
                    data-tooltip={t('batch.timeline_mode_tooltip')}
                    data-tooltip-pos="left"
                >
                    <div className="toggle-switch-handle" />
                </button>
            </div>

            <div className="options-row">
                <div className="options-label">
                    <span>{t('batch.language')}</span>
                    <span className="options-hint">{t('batch.language_hint')}</span>
                </div>
                <Dropdown
                    value={language}
                    onChange={setLanguage}
                    options={[
                        { value: 'auto', label: 'Auto' },
                        { value: 'zh', label: 'Chinese' },
                        { value: 'en', label: 'English' },
                        { value: 'ja', label: 'Japanese' },
                        { value: 'ko', label: 'Korean' },
                        { value: 'yue', label: 'Cantonese' }
                    ]}
                    style={{ width: '180px' }}
                />
            </div>
        </div>
    );
});

/**
 * Displays the status of the currently processing or selected item in the queue.
 *
 * Connected component that subscribes to the batch queue store directly.
 * Optimized with React.memo to prevent re-renders when other items update.
 *
 * @return The status display component.
 */
function ActiveItemStatusComponent(): React.JSX.Element | null {
    const { t } = useTranslation();
    const item = useBatchQueueStore((state) => state.queueItems.find((i) => i.id === state.activeItemId) || null);

    if (!item) {
        return (
            <div className="batch-queue-empty">
                <p>{t('batch.queue_empty')}</p>
            </div>
        );
    }

    switch (item.status) {
        case 'processing':
            return (
                <div className="batch-queue-processing">
                    <div className="drop-zone-text" style={{ marginBottom: 24, textAlign: 'center' }}>
                        <h3>{t('batch.processing_title')}</h3>
                        <p>{item.filename}</p>
                    </div>
                    <div
                        className="progress-bar"
                        role="progressbar"
                        aria-valuenow={Math.round(item.progress)}
                        aria-valuemin={0}
                        aria-valuemax={100}
                        aria-label={t('batch.processing_title')}
                    >
                        <div
                            className="progress-fill"
                            style={{ width: `${item.progress}%` }}
                        />
                    </div>
                    <div className="progress-text" aria-live="polite">
                        <span>{t('batch.transcribing')}</span>
                        <span>{Math.round(item.progress)}%</span>
                    </div>
                </div>
            );
        case 'error':
            return (
                <div className="batch-queue-error">
                    <div className="drop-zone-text" style={{ textAlign: 'center' }}>
                        <h3>{t('batch.file_failed')}</h3>
                        <p>{item.errorMessage || t('common.error')}</p>
                    </div>
                </div>
            );
        case 'pending':
            return (
                <div className="batch-queue-pending">
                    <div className="drop-zone-text" style={{ textAlign: 'center' }}>
                        <h3>{t('batch.queue_waiting')}</h3>
                        <p>{item.filename}</p>
                    </div>
                </div>
            );
        default:
            // Complete - show nothing here, TranscriptEditor will show the content
            return (
                <div className="batch-queue-complete">
                    <div className="drop-zone-text" style={{ textAlign: 'center' }}>
                        <h3>{t('batch.file_complete')}</h3>
                        <p>{item.filename}</p>
                    </div>
                </div>
            );
    }
}

// Optimization: Memoize to prevent re-renders unless the active item changes
const ActiveItemStatus = React.memo(ActiveItemStatusComponent);

/** Props for BatchImport. */
interface BatchImportProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Component for batch importing audio files with multi-file queue support.
 *
 * Handles drag-and-drop, file selection, and displays queue sidebar.
 *
 * @param props Component props.
 * @return The batch import UI.
 */
export function BatchImport({ className = '' }: BatchImportProps): React.JSX.Element {
    const { alert } = useDialogStore();
    const [isDragOver, setIsDragOver] = useState(false);
    const { t } = useTranslation();

    // Queue store
    // Optimization: Only subscribe to queue length to avoid re-renders on progress updates
    const hasQueueItems = useBatchQueueStore((state) => state.queueItems.length > 0);
    const isQueueProcessing = useBatchQueueStore((state) => state.isQueueProcessing);
    const addFiles = useBatchQueueStore((state) => state.addFiles);
    const enableTimeline = useBatchQueueStore((state) => state.enableTimeline);
    const setEnableTimeline = useBatchQueueStore((state) => state.setEnableTimeline);
    const language = useBatchQueueStore((state) => state.language);
    const setLanguage = useBatchQueueStore((state) => state.setLanguage);

    // Transcript store
    const config = useTranscriptStore((state) => state.config);

    const handleDrop = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        setIsDragOver(false);
    }, []);

    // Tauri File Drop Event Listener
    useEffect(() => {
        let mounted = true;
        const unlisteners: Array<() => void> = [];

        const setupListeners = async () => {
            const appWindow = getCurrentWindow();

            // Only listen to tauri://drag-drop (Tauri v2)
            const unlistenDrop = await appWindow.listen('tauri://drag-drop', (event: Event<unknown>) => {
                if (mounted) {
                    handleTauriDrop(event.payload);
                }
            });
            if (mounted) unlisteners.push(unlistenDrop);

            const unlistenHover = await appWindow.listen('tauri://drag-enter', () => {
                if (mounted) setIsDragOver(true);
            });
            if (mounted) unlisteners.push(unlistenHover);

            const unlistenCancelled = await appWindow.listen('tauri://drag-leave', () => {
                if (mounted) setIsDragOver(false);
            });
            if (mounted) unlisteners.push(unlistenCancelled);
        };

        setupListeners();

        return () => {
            mounted = false;
            unlisteners.forEach((unlisten) => unlisten());
        };
    }, []);

    const handleTauriDrop = (payload: unknown): void => {
        let files: string[] = [];

        if (Array.isArray(payload)) {
            files = payload as string[];
        } else if (payload && typeof payload === 'object' && 'paths' in payload && Array.isArray((payload as { paths: unknown }).paths)) {
            files = (payload as { paths: string[] }).paths;
        }

        if (files && files.length > 0) {
            // Validate all files
            const validFiles: string[] = [];
            const invalidFiles: string[] = [];

            files.forEach((filePath) => {
                const ext = filePath.split('.').pop()?.toLowerCase();
                const isSupported = SUPPORTED_EXTENSIONS.some(e => e.replace('.', '') === ext);
                if (isSupported) {
                    validFiles.push(filePath);
                } else {
                    invalidFiles.push(filePath);
                }
            });

            if (invalidFiles.length > 0) {
                alert(t('batch.unsupported_format', { formats: SUPPORTED_EXTENSIONS.join(', ') }), { variant: 'error' });
            }

            if (validFiles.length > 0) {
                if (!config.offlineModelPath) {
                    alert(t('batch.no_model_error'), { variant: 'error' });
                    return;
                }
                addFiles(validFiles);
            }
        } else {
            console.warn('File drop event received but payload is empty or invalid.');
        }
        setIsDragOver(false);
    };

    const handleDragOver = useCallback((e: React.DragEvent): void => {
        e.preventDefault();
        if (!isDragOver) setIsDragOver(true);
    }, [isDragOver]);

    const handleDragEnter = useCallback((e: React.DragEvent): void => {
        e.preventDefault();
        setIsDragOver(true);
    }, []);

    const handleDragLeave = useCallback((e: React.DragEvent): void => {
        e.preventDefault();
        setIsDragOver(false);
    }, []);

    const handleKeyDown = useCallback((e: React.KeyboardEvent): void => {
        if (e.key === 'Enter' || e.key === ' ') {
            handleClick();
        }
    }, []);

    const handleClick = async (): Promise<void> => {
        try {
            const selected = await open({
                multiple: true,
                filters: [{
                    name: 'Audio',
                    extensions: SUPPORTED_EXTENSIONS.map(ext => ext.replace('.', ''))
                }]
            });

            if (selected) {
                const files = Array.isArray(selected) ? selected : [selected];
                if (files.length > 0) {
                    if (!config.offlineModelPath) {
                        alert(t('batch.no_model_error'), { variant: 'error' });
                        return;
                    }
                    addFiles(files);
                }
            }
        } catch (err) {
            console.error('Failed to open dialog:', err);
        }
    };



    // Render the queue view when we have items
    if (hasQueueItems) {
        return (
            <div className={`batch-import-container batch-import-queue-view ${className}`}>
                <FileQueueSidebar />

                <div className="batch-queue-content">
                    <ActiveItemStatus />

                    {/* Add more files button */}
                    <div className="batch-add-more">
                        <div
                            data-tooltip={isQueueProcessing ? t('batch.processing_wait') : undefined}
                            data-tooltip-pos="top"
                            style={{ display: 'inline-block' }}
                        >
                            <button
                                className="btn btn-secondary"
                                onClick={handleClick}
                                disabled={isQueueProcessing}
                                style={isQueueProcessing ? { pointerEvents: 'none' } : undefined}
                            >
                                {t('batch.add_more_files')}
                            </button>
                        </div>
                    </div>
                </div>

                {/* Options */}
                <BatchImportOptions
                    enableTimeline={enableTimeline}
                    setEnableTimeline={setEnableTimeline}
                    language={language}
                    setLanguage={setLanguage}
                />
            </div>
        );
    }

    // Initial drop zone view (no queue items)
    return (
        <div className={`batch-import-container ${className}`}>
            <div
                className={`drop-zone drop-zone-wrapper ${isDragOver ? 'drag-over' : ''}`}
                onDrop={handleDrop}
                onDragOver={handleDragOver}
                onDragEnter={handleDragEnter}
                onDragLeave={handleDragLeave}
                onClick={handleClick}
                onKeyDown={handleKeyDown}
                role="button"
                tabIndex={0}
                aria-label={t('batch.drop_desc')}
            >
                <div className="drop-zone-icon">
                    <UploadIcon />
                </div>

                <div className="drop-zone-text">
                    <h3>{t('batch.drop_title')}</h3>
                    <p>{t('batch.drop_desc')}</p>
                </div>

                <div className="btn btn-primary" style={{ marginTop: '8px', pointerEvents: 'none' }} aria-hidden="true">
                    {t('batch.select_file')}
                </div>

                <p className="supported-formats" style={{ marginTop: '8px' }}>
                    {t('batch.supports', { formats: SUPPORTED_EXTENSIONS.join(', ') })}
                </p>
            </div>

            <BatchImportOptions
                enableTimeline={enableTimeline}
                setEnableTimeline={setEnableTimeline}
                language={language}
                setLanguage={setLanguage}
            />
        </div>
    );
}

export default BatchImport;
