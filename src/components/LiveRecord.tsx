import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore } from '../stores/configStore';
import { polishService } from '../services/polishService';
import { Pause, Play, Square, Mic, Monitor } from 'lucide-react';
import { splitByPunctuation } from '../utils/segmentUtils';
import { RecordingTimer } from './RecordingTimer';
import { Dropdown } from './Dropdown';
import { TranscriptionOptions } from './TranscriptionOptions';
import { Switch } from './Switch';
import { captionWindowService } from '../services/captionWindowService';
import { useCaptionSession } from '../hooks/useCaptionSession';
import { useAudioVisualizer } from '../hooks/useAudioVisualizer';
import { useAudioRecorder, type RecordSegmentDeliveryMeta } from '../hooks/useAudioRecorder';
import { useOnboardingStore } from '../stores/onboardingStore';
import { logger } from '../utils/logger';
import { TranscriptSegment } from '../types/transcript';

/** Props for the LiveRecord component. */
interface LiveRecordProps {
    className?: string;
}

function getSourceIcon(source: 'microphone' | 'desktop' | 'file'): React.ReactElement {
    switch (source) {
        case 'microphone': return <Mic size={18} aria-hidden="true" />;
        case 'desktop': return <Monitor size={18} aria-hidden="true" />;
        default: return <Mic size={18} aria-hidden="true" />;
    }
}

/**
 * Component for handling real-time audio recording and visualization.
 *
 * Supports recording from microphone or system audio (desktop).
 * Includes a visualizer and timer.
 *
 * @param props Component props.
 * @return The rendered LiveRecord component.
 */
export function LiveRecord({ className = '' }: LiveRecordProps): React.ReactElement {
    const { t } = useTranslation();
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const polishedIdsRef = useRef<Set<string>>(new Set());
    const startButtonRef = useRef<HTMLButtonElement>(null);

    // State from store
    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const focusStartRecordingToken = useOnboardingStore((state) => state.focusStartRecordingToken);

    // Local State
    const [inputSource, setInputSource] = useState<'microphone' | 'desktop'>('microphone');
    const isRecordingRef = useRef(false);

    // Sync refs with store state
    useEffect(() => {
        isRecordingRef.current = isRecording;
    }, [isRecording]);

    useEffect(() => {
        if (!isRecording) {
            startButtonRef.current?.focus();
        }
    }, [focusStartRecordingToken, isRecording]);

    // Caption Mode
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);
    const setIsCaptionMode = useTranscriptStore((state) => state.setIsCaptionMode);
    const config = useConfigStore((state) => state.config);

    // Initialize dedicated caption session hook
    useCaptionSession(config, isCaptionMode);

    // Config Helpers
    const enableTimeline = config.enableTimeline ?? false;
    const lockWindow = config.lockWindow ?? false;
    const alwaysOnTop = config.alwaysOnTop ?? true;
    const enableTimelineRef = useRef(enableTimeline);

    const upsertSegmentAndSetActive = useTranscriptStore((state) => state.upsertSegmentAndSetActive);

    useEffect(() => {
        enableTimelineRef.current = enableTimeline;
    }, [enableTimeline]);

    useEffect(() => {
        captionWindowService.setClickThrough(lockWindow).catch(logger.error);
        captionWindowService.setAlwaysOnTop(alwaysOnTop).catch(logger.error);
    }, [lockWindow, alwaysOnTop]);

    // Segment Handler
    const onSegment = useCallback((segment: TranscriptSegment, meta: RecordSegmentDeliveryMeta) => {
        const storeState = useTranscriptStore.getState();
        logger.info(
            `[LiveRecord] onSegment ${meta.accepted ? 'accepted' : 'dropped'}. segment=${segment.id} final=${segment.isFinal} session=${meta.sessionId ?? 'none'} phase=${meta.phase} store_is_recording=${storeState.isRecording}`
        );

        if (!meta.accepted) {
            return;
        }

        if (enableTimeline && segment.isFinal) {
            const parts = splitByPunctuation([segment]);
            if (parts.length > 0) {
                storeState.deleteSegment(segment.id);
                parts.forEach(part => storeState.upsertSegment(part));
                storeState.setActiveSegmentId(parts[parts.length - 1].id);
            } else {
                upsertSegmentAndSetActive(segment);
            }
        } else {
            upsertSegmentAndSetActive(segment);
        }

        // Auto-Polish Logic
        const config = useConfigStore.getState().config;
        const autoPolish = config.autoPolish ?? false;
        const frequency = config.autoPolishFrequency ?? 5;

        if (autoPolish && frequency > 0) {
            const allSegments = storeState.segments;
            const unpolished = allSegments.filter(s => s.isFinal && !polishedIdsRef.current.has(s.id));

            if (unpolished.length >= frequency) {
                const toPolish = unpolished.slice(0, frequency);
                toPolish.forEach(s => polishedIdsRef.current.add(s.id));

                polishService.polishSegments(toPolish, (chunk) => {
                    const store = useTranscriptStore.getState();
                    chunk.forEach(p => store.updateSegment(p.id, { text: p.text }));
                }).catch(err => {
                    logger.error('[LiveRecord] Auto-polish failed:', err);
                });
            }
        }
    }, [upsertSegmentAndSetActive, enableTimeline]);

    // Audio Recorder Hook
    const {
        startRecording,
        stopRecording,
        pauseRecording,
        resumeRecording,
        isInitializing,
        isTransitioning,
        recordingElapsedMs,
        peakLevelRef
    } = useAudioRecorder({
        inputSource,
        onSegment
    });

    // Visualizer Hook
    const { startVisualizer, stopVisualizer } = useAudioVisualizer({
        canvasRef,
        peakLevelRef,
        isPaused
    });

    const handleToggleRecording = useCallback(async () => {
        if (isRecording) {
            await stopRecording();
            stopVisualizer();
        } else {
            polishedIdsRef.current.clear();
            const success = await startRecording();
            if (success) {
                startVisualizer();
            }
        }
    }, [isRecording, startRecording, stopRecording, startVisualizer, stopVisualizer]);

    const handleTogglePause = useCallback(async () => {
        if (isPaused) {
            await resumeRecording();
        } else {
            await pauseRecording();
        }
    }, [isPaused, pauseRecording, resumeRecording]);

    const handleCaptionToggle = useCallback((checked: boolean) => {
        setIsCaptionMode(checked);
    }, [setIsCaptionMode]);

    function getRecordingStatusText(): string {
        if (isRecording) {
            return isPaused ? t('live.recording_paused') : t('live.recording_active');
        }
        return t('live.start_hint');
    }

    // Keyboard shortcuts
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const shortcutStr = config.liveRecordShortcut || 'Ctrl + Space';
            const parts = shortcutStr.split(' + ').map(p => p.trim());

            const needsCtrl = parts.includes('Ctrl');
            const needsAlt = parts.includes('Alt');
            const needsShift = parts.includes('Shift');
            const needsMeta = parts.includes('Meta');
            const mainKeyPart = parts[parts.length - 1];

            let eventKey = e.key;
            if (eventKey === ' ') eventKey = 'Space';
            else if (eventKey.length === 1) eventKey = eventKey.toUpperCase();

            const isStartStopMatch =
                e.ctrlKey === needsCtrl &&
                e.altKey === needsAlt &&
                e.shiftKey === needsShift &&
                e.metaKey === needsMeta &&
                (eventKey === mainKeyPart || e.code === mainKeyPart || e.code === `Key${mainKeyPart}`);

            if (isStartStopMatch) {
                e.preventDefault();
                void handleToggleRecording();
            } else if (e.code === 'Space' && !needsCtrl && !needsAlt && !needsShift && !needsMeta && mainKeyPart === 'Space') {
                // If the user mapped start/stop to just "Space", we don't handle pause with "Space"
                // to avoid double triggering. But if they didn't map it to just "Space", we can pause with "Space".
                if (isRecordingRef.current) {
                    e.preventDefault();
                    void handleTogglePause();
                }
            } else if (e.code === 'Space' && isRecordingRef.current) {
                e.preventDefault();
                void handleTogglePause();
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [handleToggleRecording, handleTogglePause, config.liveRecordShortcut]);

    return (
        <div className={`live-record-container ${className}`}>
            <div className="live-record-main-content">
                <div className="visualizer-wrapper">
                    <canvas
                        ref={canvasRef}
                        width={600}
                        height={120}
                        className="visualizer-canvas"
                        role="img"
                        aria-label={t('live.visualizer_label')}
                    />
                </div>

                <RecordingTimer elapsedMs={recordingElapsedMs} isRecording={isRecording} />

                <div className="record-controls">
                    {!isRecording ? (
                        <button
                            ref={startButtonRef}
                            className="control-button start"
                            onClick={handleToggleRecording}
                            disabled={isInitializing || isTransitioning}
                            aria-label={t('live.start_recording')}
                            data-tooltip={isInitializing ? 'Initializing...' : t('live.start_recording')}
                            data-tooltip-pos="bottom"
                            style={isInitializing || isTransitioning ? { opacity: 0.7, cursor: 'wait' } : {}}
                        >
                            <div className="control-button-inner" />
                        </button>
                    ) : (
                        <>
                            <button
                                className="control-button pause"
                                onClick={handleTogglePause}
                                disabled={isInitializing || isTransitioning}
                                aria-label={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip-pos="bottom"
                            >
                                {isPaused ? <Play size={24} fill="currentColor" aria-hidden="true" /> : <Pause size={24} fill="currentColor" aria-hidden="true" />}
                            </button>

                            <button
                                className="control-button stop"
                                onClick={handleToggleRecording}
                                disabled={isInitializing || isTransitioning}
                                aria-label={t('live.stop')}
                                data-tooltip={t('live.stop')}
                                data-tooltip-pos="bottom"
                            >
                                <Square size={28} fill="white" color="white" aria-hidden="true" />
                            </button>
                        </>
                    )}
                </div>

                {!isRecording && (
                    <div className="input-source-selector">
                        <div className="source-select-wrapper">
                            {getSourceIcon(inputSource)}
                            <Dropdown
                                value={inputSource}
                                onChange={(value) => setInputSource(value as 'microphone' | 'desktop')}
                                aria-label={t('live.source_select')}
                                options={[
                                    { value: 'microphone', label: t('live.source_microphone') },
                                    { value: 'desktop', label: t('live.source_desktop') }
                                ]}
                                style={{ minWidth: '180px' }}
                            />
                        </div>
                    </div>
                )}

                <p className="recording-status-text" aria-live="polite">
                    {getRecordingStatusText()}
                </p>
            </div>

            <div className="live-caption-toggle">
                <Switch
                    checked={isCaptionMode}
                    onChange={handleCaptionToggle}
                    label={t('live.caption_mode')}
                    disabled={false}
                />
                <span className="live-caption-hint">{t('live.caption_mode_hint')}</span>
            </div>

            <TranscriptionOptions
                disabled={isRecording}
            />
        </div>
    );
}
