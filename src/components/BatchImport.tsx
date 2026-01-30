import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { modelService } from '../services/modelService';
import { splitByPunctuation } from '../utils/segmentUtils';

// Icons
const UploadIcon = () => (
    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" />
        <path d="M12 12v9" />
        <path d="m16 16-4-4-4 4" />
    </svg>
);



const ACCEPTED_EXTENSIONS = ['.wav', '.mp3', '.m4a', '.ogg', '.webm', '.mp4'];

/** Props for BatchImport. */
interface BatchImportProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Component for batch importing audio files.
 * Handles drag-and-drop, file selection, and initiates transcription.
 *
 * @param props - Component props.
 * @return The batch import UI.
 */
export const BatchImport: React.FC<BatchImportProps> = ({ className = '' }) => {
    // const fileInputRef = useRef<HTMLInputElement>(null);
    const { alert } = useDialogStore();
    const [isDragOver, setIsDragOver] = useState(false);
    const [enableTimeline, setEnableTimeline] = useState(true);
    const [language, setLanguage] = useState<string>('auto');

    const processingStatus = useTranscriptStore((state) => state.processingStatus);
    const processingProgress = useTranscriptStore((state) => state.processingProgress);
    const setProcessingStatus = useTranscriptStore((state) => state.setProcessingStatus);
    const setProcessingProgress = useTranscriptStore((state) => state.setProcessingProgress);
    const config = useTranscriptStore((state) => state.config);
    const { t } = useTranslation();



    const handleDrop = useCallback((e: React.DragEvent) => {
        // Fallback or visual handling only, actual logic handled by Tauri event
        e.preventDefault();
        e.stopPropagation();
        setIsDragOver(false);
    }, []);

    // Tauri File Drop Event Listener
    useEffect(() => {
        const unlistenDrop = listen('tauri://file-drop', (event) => {
            const files = event.payload as string[];
            if (files && files.length > 0) {
                const filePath = files[0];
                // Create a mock File object for validation or validate manually
                const ext = filePath.split('.').pop()?.toLowerCase();
                const isSupported = ACCEPTED_EXTENSIONS.some(e => e.replace('.', '') === ext);

                if (isSupported) {
                    processFile(filePath);
                } else {
                    alert(t('batch.unsupported_format', { formats: ACCEPTED_EXTENSIONS.join(', ') }), { variant: 'error' });
                }
            }
            setIsDragOver(false);
        });

        const unlistenHover = listen('tauri://file-drop-hover', () => {
            setIsDragOver(true);
        });

        const unlistenCancelled = listen('tauri://file-drop-cancelled', () => {
            setIsDragOver(false);
        });

        // Cleanup
        return () => {
            unlistenDrop.then(f => f());
            unlistenHover.then(f => f());
            unlistenCancelled.then(f => f());
        };
    }, []);

    const handleDragOver = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        if (!isDragOver) setIsDragOver(true);
    }, [isDragOver]);

    const handleDragEnter = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setIsDragOver(true);
    }, []);

    const handleDragLeave = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();

        // Only disable drag over if we're actually leaving the drop zone, 
        // not just entering a child element
        if (e.currentTarget.contains(e.relatedTarget as Node)) {
            return;
        }
        setIsDragOver(false);
    }, []);

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            handleClick();
        }
    };

    const handleClick = async () => {
        try {
            const selected = await open({
                multiple: false,
                filters: [{
                    name: 'Audio/Video',
                    extensions: ['wav', 'mp3', 'm4a', 'ogg', 'webm', 'mp4']
                }]
            });

            if (selected && typeof selected === 'string') {
                await processFile(selected);
            }
        } catch (err) {
            console.error('Failed to open dialog:', err);
        }
    };



    // ... (existing code)

    const processFile = async (filePath: string) => {
        if (!config.offlineModelPath) {
            alert(t('batch.no_model_error'), { variant: 'error' });
            return;
        }

        setProcessingStatus('loading');
        setProcessingProgress(0);

        try {
            const assetUrl = convertFileSrc(filePath);

            useTranscriptStore.getState().setAudioUrl(assetUrl);

            transcriptionService.setModelPath(config.offlineModelPath);
            const enabledITNModels = new Set(config.enabledITNModels || []);
            const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];

            transcriptionService.setEnableITN(enabledITNModels.size > 0);

            if (enabledITNModels.size > 0) {
                try {
                    const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                    transcriptionService.setITNModelPaths(paths);
                } catch (e) { }
            }

            if (config.punctuationModelPath) {
                transcriptionService.setPunctuationModelPath(config.punctuationModelPath);
            } else {
                transcriptionService.setPunctuationModelPath('');
            }

            if (config.vadModelPath) {
                transcriptionService.setVadModelPath(config.vadModelPath);
                transcriptionService.setVadBufferSize(config.vadBufferSize || 5);
            }

            // Clear previous segments if we are starting a new import
            // But if we want to APPEND, we shouldn't.
            // Batch import usually replaces current transcript?
            // "BatchImport" implies loading a file. 
            // Existing logic: useTranscriptStore.getState().setSegments(...) at end replaces all.
            // So we should clear at start to show progress.
            useTranscriptStore.getState().clearSegments();



            const segments = await transcriptionService.transcribeFile(filePath, (progress) => {
                setProcessingProgress(progress);
            }, (segment) => {
                // Streaming callback
                if (enableTimeline) {
                    // Attempt to split immediately? 
                    // Or just show raw segment? 
                    // splitByPunctuation might be stateless per segment.
                    const split = splitByPunctuation([segment]);
                    split.forEach(s => useTranscriptStore.getState().upsertSegment(s));
                } else {
                    useTranscriptStore.getState().upsertSegment(segment);
                }
            }, language);

            useTranscriptStore.getState().setSegments(enableTimeline ? splitByPunctuation(segments) : segments);
            setProcessingStatus('complete');
            setProcessingProgress(100);
        } catch (error) {
            console.error('Transcription failed:', error);
            setProcessingStatus('error');
            alert(t('batch.transcription_failed', { error }), { variant: 'error' });
        }
    };

    // No longer used
    // const handleInputChange = ...

    if (processingStatus === 'loading' || processingStatus === 'processing') {
        return (
            <div className={`progress-container ${className}`}>
                <div className="drop-zone-text" style={{ marginBottom: 24, textAlign: 'center' }}>
                    <h3>{t('batch.processing_title')}</h3>
                    <p>{t('batch.processing_desc')}</p>
                </div>
                <div
                    className="progress-bar"
                    role="progressbar"
                    aria-valuenow={Math.round(processingProgress)}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label={t('batch.processing_title')}
                >
                    <div
                        className="progress-fill"
                        style={{ width: `${processingProgress}%` }}
                    />
                </div>
                <div className="progress-text" aria-live="polite">
                    <span>{t('batch.transcribing')}</span>
                    <span>{Math.round(processingProgress)}%</span>
                </div>
            </div>
        );
    }

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
                {/* <input
                    ref={fileInputRef}
                    type="file"
                    accept={ACCEPTED_FORMATS.join(',')}
                    onChange={handleInputChange}
                    style={{ display: 'none' }}
                /> */}

                <div className="drop-zone-icon">
                    <UploadIcon />
                </div>

                <div className="drop-zone-text">
                    <h3>{t('batch.drop_title')}</h3>
                    <p>{t('batch.drop_desc')}</p>
                </div>

                <button className="btn btn-primary" style={{ marginTop: '8px', pointerEvents: 'none' }}>
                    {t('batch.select_file')}
                </button>

                <p className="supported-formats" style={{ marginTop: '8px' }}>
                    {t('batch.supports', { formats: ACCEPTED_EXTENSIONS.join(', ') })}
                </p>
            </div>

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
                    <select
                        className="settings-input"
                        value={language}
                        onChange={(e) => setLanguage(e.target.value)}
                        style={{ maxWidth: '120px' }}
                        aria-label={t('batch.language')}
                    >
                        <option value="auto">Auto</option>
                        <option value="zh">Chinese</option>
                        <option value="en">English</option>
                        <option value="ja">Japanese</option>
                        <option value="ko">Korean</option>
                        <option value="yue">Cantonese</option>
                    </select>
                </div>
            </div>
        </div>
    );
};

export default BatchImport;
