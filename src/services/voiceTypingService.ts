import { register, unregister, isRegistered } from '@tauri-apps/plugin-global-shortcut';
import { invoke } from '@tauri-apps/api/core';
import i18next from 'i18next';
import { TranscriptionService } from './transcriptionService';
import { TranscriptSegment } from '../types/transcript';
import {
    VoiceTypingOverlayPayload,
    voiceTypingWindowService,
} from './voiceTypingWindowService';
import { useConfigStore } from '../stores/configStore';
import { logger } from '../utils/logger';

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;
const LISTENING_RESET_VISIBILITY_MS = 350;
const HOLD_FINAL_SEGMENT_VISIBILITY_MS = 700;
const FLUSH_EVENT_SETTLE_MS = 80;

function delay(ms: number) {
    return new Promise<void>((resolve) => {
        setTimeout(resolve, ms);
    });
}

class VoiceTypingService {
    private isListening = false;
    private sessionState: 'idle' | 'starting' | 'listening' | 'stopping' = 'idle';
    private isShortcutRegistered = false;
    private transcriptionService: TranscriptionService;
    private currentShortcut: string | null = null;
    private captureStarted = false;
    private startRequestId = 0;
    private activeSessionId: string | null = null;
    private activeSegmentId: string | null = null;
    private overlayVisible = false;
    private lastOverlayPosition: [number, number] | null = null;
    private lastOverlayPayload: VoiceTypingOverlayPayload | null = null;
    private listeningResetTimer: ReturnType<typeof setTimeout> | null = null;

    private initialized = false;
    private lastEnabled = false;
    private lastShortcut = '';
    private lastModelPath = '';
    private lastLanguage = '';
    private lastEnableITN = true;

    constructor() {
        this.transcriptionService = new TranscriptionService('voice-typing');
    }

    public init() {
        if (this.initialized) {
            logger.info('[VoiceTypingService] Already initialized.');
            return;
        }
        this.initialized = true;
        logger.info('[VoiceTypingService] Initializing...');

        const initialConfig = useConfigStore.getState().config;
        this.lastEnabled = initialConfig.voiceTypingEnabled || false;
        this.lastShortcut = initialConfig.voiceTypingShortcut || 'Alt+V';
        this.lastModelPath = initialConfig.streamingModelPath || '';
        this.lastLanguage = initialConfig.language || 'auto';
        this.lastEnableITN = initialConfig.enableITN ?? true;

        logger.info(`[VoiceTypingService] Initial config - Enabled: ${this.lastEnabled}, Shortcut: ${this.lastShortcut}`);

        // Listen to config changes to update shortcuts and pre-warm model
        useConfigStore.subscribe((state) => {
            const newConfig = state.config;
            const newEnabled = newConfig.voiceTypingEnabled || false;
            const newShortcut = newConfig.voiceTypingShortcut || 'Alt+V';
            const newModelPath = newConfig.streamingModelPath || '';
            const newLanguage = newConfig.language || 'auto';
            const newEnableITN = newConfig.enableITN ?? true;

            const configChanged = newModelPath !== this.lastModelPath || 
                                newLanguage !== this.lastLanguage || 
                                newEnableITN !== this.lastEnableITN;

            if (newEnabled !== this.lastEnabled || newShortcut !== this.lastShortcut) {
                logger.info(`[VoiceTypingService] Shortcut config changed. New Enabled: ${newEnabled}, New Shortcut: ${newShortcut}`);
                this.lastEnabled = newEnabled;
                this.lastShortcut = newShortcut;
                this.updateShortcutRegistration(newEnabled, newShortcut);
                
                if (!newEnabled) {
                    this.stopMicrophoneCapture();
                }
            }

            if (newEnabled && (configChanged || newEnabled !== this.lastEnabled)) {
                this.lastModelPath = newModelPath;
                this.lastLanguage = newLanguage;
                this.lastEnableITN = newEnableITN;
                this.syncAndPrepare();
            }
        });

        // Initial setup
        this.updateShortcutRegistration(this.lastEnabled, this.lastShortcut);
        if (this.lastEnabled) {
            this.syncAndPrepare();
        }
    }

    private async syncAndPrepare() {
        const config = useConfigStore.getState().config;
        if (!config.voiceTypingEnabled || !config.streamingModelPath) return;

        try {
            logger.info('[VoiceTypingService] Pre-warming transcription model and microphone...');
            this.transcriptionService.setModelPath(config.streamingModelPath);
            this.transcriptionService.setLanguage(config.language);
            this.transcriptionService.setEnableITN(config.enableITN ?? true);
            await this.transcriptionService.prepare();
            await voiceTypingWindowService.prepare(this.lastOverlayPosition ?? [0, 0]);
            
            // Warm up microphone
            await this.ensureMicrophoneStarted();
            
            logger.info('[VoiceTypingService] Model and Mic pre-warmed and ready.');
        } catch (e) {
            logger.error('[VoiceTypingService] Failed to pre-warm:', e);
        }
    }

    private async ensureMicrophoneStarted() {
        if (this.captureStarted) return;
        
        try {
            const config = useConfigStore.getState().config;
            logger.info('[VoiceTypingService] Starting microphone capture for pre-warming...');
            await invoke('start_microphone_capture', {
                deviceName: config.microphoneId === 'default' ? null : config.microphoneId,
                instanceId: 'voice-typing'
            });
            this.captureStarted = true;
        } catch (e) {
            logger.error('[VoiceTypingService] Failed to start microphone capture:', e);
        }
    }

    private async stopMicrophoneCapture() {
        if (!this.captureStarted) return;
        
        try {
            logger.info('[VoiceTypingService] Stopping persistent microphone capture...');
            await invoke('stop_microphone_capture', { instanceId: 'voice-typing' });
            this.captureStarted = false;
        } catch (e) {
            logger.error('[VoiceTypingService] Failed to stop microphone capture:', e);
        }
    }

    private async updateShortcutRegistration(enabled: boolean, shortcut: string) {
        const normalizedShortcut = shortcut.replace(/\s+/g, '');
        logger.info(`[VoiceTypingService] updateShortcutRegistration called. Enabled: ${enabled}, Shortcut: ${shortcut}, Normalized: ${normalizedShortcut}`);
        
        try {
            if (this.isShortcutRegistered && this.currentShortcut) {
                logger.info(`[VoiceTypingService] Checking if current shortcut ${this.currentShortcut} is registered...`);
                const registered = await isRegistered(this.currentShortcut);
                if (registered) {
                    logger.info(`[VoiceTypingService] Unregistering old shortcut ${this.currentShortcut}...`);
                    await unregister(this.currentShortcut);
                }
                this.isShortcutRegistered = false;
            }

            if (enabled && normalizedShortcut) {
                logger.info(`[VoiceTypingService] Attempting to register new shortcut ${normalizedShortcut}...`);
                await register(normalizedShortcut, (event) => {
                    logger.info(`[VoiceTypingService] Shortcut Event Triggered: ${event.shortcut}, State: ${event.state}`);
                    const mode = useConfigStore.getState().config.voiceTypingMode || 'hold';
                    logger.info(`[VoiceTypingService] Current Mode: ${mode}, isListening: ${this.isListening}`);
                    
                    if (mode === 'hold') {
                        if (event.state === 'Pressed' && !this.isListening) {
                            logger.info('[VoiceTypingService] Hold Mode -> Pressed -> Starting listen');
                            this.startListening();
                        } else if (event.state === 'Released' && this.isListening) {
                            logger.info('[VoiceTypingService] Hold Mode -> Released -> Stopping listen');
                            this.stopListening();
                        }
                    } else if (mode === 'toggle') {
                        if (event.state === 'Released') {
                            if (this.isListening) {
                                logger.info('[VoiceTypingService] Toggle Mode -> Released -> Stopping listen');
                                this.stopListening();
                            } else {
                                logger.info('[VoiceTypingService] Toggle Mode -> Released -> Starting listen');
                                this.startListening();
                            }
                        }
                    }
                });
                this.isShortcutRegistered = true;
                this.currentShortcut = normalizedShortcut;
                logger.info(`[VoiceTypingService] Successfully registered voice typing shortcut: ${normalizedShortcut}`);
            } else {
                logger.info(`[VoiceTypingService] Not registering shortcut (enabled: ${enabled}, shortcut: ${normalizedShortcut})`);
            }
        } catch (e) {
            logger.error('[VoiceTypingService] Failed to update voice typing shortcut:', e);
        }
    }

    private async startListening() {
        if (this.isListening) return;

        const requestId = ++this.startRequestId;
        const sessionId = `voice-typing-${requestId}`;
        this.clearListeningResetTimer();
        this.isListening = true;
        this.sessionState = 'starting';
        this.activeSessionId = sessionId;
        this.activeSegmentId = null;
        
        try {
            // 1. Prepare backend in parallel (non-blocking)
            const config = useConfigStore.getState().config;
            this.transcriptionService.setModelPath(config.streamingModelPath);
            this.transcriptionService.setLanguage(config.language);
            this.transcriptionService.setEnableITN(config.enableITN ?? true);

            const startPromise = this.transcriptionService.start(
                async (segment) => {
                    await this.handleSegmentUpdate(sessionId, requestId, segment);
                },
                (error) => {
                    if (!this.isCurrentSession(sessionId, requestId)) {
                        return;
                    }

                    logger.error('Voice typing transcription error:', error);
                    void this.handleSessionError(sessionId, requestId, error);
                }
            );

            // 2. Detect position and open window in parallel
            const positionPromise = this.sendOverlayState(
                { sessionId, phase: 'preparing', text: '' },
                { revealIfHidden: true, reposition: true }
            );

            // 3. Wait for basic readiness
            await Promise.all([startPromise, positionPromise]);

            if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
                return;
            }

            // Ensure mic is started (should already be warmed up)
            await this.ensureMicrophoneStarted();

            if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
                return;
            }

            // Successfully started recognition
            this.sessionState = 'listening';
            await this.sendOverlayState({ sessionId, phase: 'listening', text: '' });

        } catch (e) {
            logger.error('Failed to start voice typing:', e);
            // Don't stop capture here as we want to keep it warmed up
            await this.transcriptionService.softStop().catch((stopError) => {
                logger.error('Failed to roll back voice typing recognizer after start failure:', stopError);
            });

            if (this.activeSessionId === sessionId) {
                await this.hideOverlay(sessionId);
                this.finishSession(sessionId);
            }
        }
    }

    private async stopListening() {
        if (!this.isListening || !this.activeSessionId || this.sessionState === 'stopping') return;

        const sessionId = this.activeSessionId;
        const mode = this.getVoiceTypingMode();
        this.sessionState = 'stopping';
        this.clearListeningResetTimer();

        // We still await softStop to ensure the flush_recognizer completes 
        // and the final segments are emitted and injected.
        // But we DO NOT stop the microphone capture to keep it warmed up.
        await this.transcriptionService.softStop().catch((stopError) => {
            logger.error('Failed to flush voice typing recognizer while stopping:', stopError);
        });

        if (!this.isCurrentSession(sessionId)) {
            return;
        }

        // `flush_recognizer` waits for backend inference, but the emitted Tauri event can still
        // arrive a beat later on the frontend. Give the final sentence a tiny settle window
        // before deciding whether we can close immediately.
        if (!this.isFinalSegmentVisible(sessionId)) {
            await delay(FLUSH_EVENT_SETTLE_MS);
        }

        if (!this.isCurrentSession(sessionId)) {
            return;
        }

        if (this.isFinalSegmentVisible(sessionId)) {
            await delay(mode === 'hold' ? HOLD_FINAL_SEGMENT_VISIBILITY_MS : LISTENING_RESET_VISIBILITY_MS);
        }

        await this.hideOverlay(sessionId).catch(e => logger.error('[VoiceTypingService] Failed to close window:', e));
        this.finishSession(sessionId);
    }

    private async handleSegmentUpdate(sessionId: string, requestId: number, segment: TranscriptSegment) {
        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        const text = segment.text.trim();
        const isNewSegment = this.activeSegmentId !== segment.id;
        this.clearListeningResetTimer();

        if (!text) {
            if (!segment.isFinal && this.overlayVisible && this.sessionState === 'listening') {
                this.activeSegmentId = null;
                await this.sendOverlayState({ sessionId, phase: 'listening', text: '' });
            }
            return;
        }

        this.activeSegmentId = segment.id;
        await this.sendOverlayState(
            {
                sessionId,
                phase: 'segment',
                text,
                segmentId: segment.id,
                isFinal: segment.isFinal,
            },
            {
                revealIfHidden: !this.overlayVisible,
                reposition: this.overlayVisible && isNewSegment,
            }
        );

        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        if (segment.isFinal) {
            await invoke('inject_text', { text });

            if (!this.isCurrentSession(sessionId, requestId)) {
                return;
            }

            if (this.sessionState === 'listening') {
                this.scheduleListeningReset(sessionId, segment.id);
            }
        }
    }

    private async handleSessionError(sessionId: string, requestId: number, error: string) {
        this.clearListeningResetTimer();
        this.sessionState = 'stopping';
        await this.sendOverlayState({
            sessionId,
            phase: 'error',
            text: i18next.t('errors.common.operation_failed') + ': ' + error,
        }, { revealIfHidden: true });

        await this.transcriptionService.softStop().catch((stopError) => {
            logger.error('Failed to stop voice typing recognizer after error:', stopError);
        });

        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        await delay(HOLD_FINAL_SEGMENT_VISIBILITY_MS);
        await this.hideOverlay(sessionId);
        this.finishSession(sessionId);
    }

    private async sendOverlayState(
        payload: VoiceTypingOverlayPayload,
        options?: { revealIfHidden?: boolean; reposition?: boolean }
    ) {
        this.lastOverlayPayload = payload;
        const shouldReveal = options?.revealIfHidden ?? false;
        const shouldReposition = options?.reposition ?? false;
        const wasOverlayVisible = this.overlayVisible;

        if (shouldReveal || (shouldReposition && this.overlayVisible)) {
            const [x, y] = await this.getOverlayPosition();
            this.lastOverlayPosition = [x, y];
            await voiceTypingWindowService.open(x, y);
            this.overlayVisible = true;
        }

        logger.info('[VoiceTypingService] Dispatching overlay state', {
            sessionId: payload.sessionId,
            phase: payload.phase,
            segmentId: payload.segmentId ?? null,
            isFinal: payload.isFinal ?? null,
            textLength: payload.text.length,
            overlayVisible: this.overlayVisible,
            wasOverlayVisible,
            revealIfHidden: shouldReveal,
            reposition: shouldReposition,
        });
        await voiceTypingWindowService.sendState(payload);
    }

    private async hideOverlay(sessionId: string) {
        this.overlayVisible = false;
        await voiceTypingWindowService.close();
        await voiceTypingWindowService.sendState({ sessionId, phase: 'listening', text: '' });
    }

    private clearListeningResetTimer() {
        if (this.listeningResetTimer) {
            clearTimeout(this.listeningResetTimer);
            this.listeningResetTimer = null;
        }
    }

    private scheduleListeningReset(sessionId: string, segmentId: string) {
        this.clearListeningResetTimer();
        this.listeningResetTimer = setTimeout(() => {
            if (!this.isCurrentSession(sessionId) || this.sessionState !== 'listening' || this.activeSegmentId !== segmentId) {
                return;
            }

            this.activeSegmentId = null;
            void this.sendOverlayState({ sessionId, phase: 'listening', text: '' });
        }, LISTENING_RESET_VISIBILITY_MS);
    }

    private isCurrentSession(sessionId: string, requestId?: number) {
        return this.activeSessionId === sessionId &&
            (requestId === undefined || requestId === this.startRequestId);
    }

    private finishSession(sessionId: string) {
        if (this.activeSessionId !== sessionId) {
            return;
        }

        this.clearListeningResetTimer();
        this.activeSessionId = null;
        this.activeSegmentId = null;
        this.lastOverlayPayload = null;
        this.sessionState = 'idle';
        this.isListening = false;
    }

    private isSessionStopping() {
        return this.sessionState === 'stopping';
    }

    private getVoiceTypingMode() {
        return useConfigStore.getState().config.voiceTypingMode || 'hold';
    }

    private isFinalSegmentVisible(sessionId: string) {
        return this.lastOverlayPayload?.sessionId === sessionId &&
            this.lastOverlayPayload.phase === 'segment' &&
            this.lastOverlayPayload.isFinal === true;
    }

    private async getOverlayPosition(): Promise<[number, number]> {
        const MARGIN_COMPENSATION = 4;
        try {
            const cursorPosition = await invoke<[number, number] | null>('get_text_cursor_position');
            if (cursorPosition) {
                return [cursorPosition[0] - MARGIN_COMPENSATION, cursorPosition[1] + CURSOR_POSITION_OFFSET - MARGIN_COMPENSATION];
            }
        } catch (error) {
            logger.debug('[VoiceTypingService] Failed to get text cursor position, falling back to mouse.', error);
        }

        if (this.lastOverlayPosition) {
            return this.lastOverlayPosition;
        }

        const [x, y] = await invoke<[number, number]>('get_mouse_position');
        return [x - MARGIN_COMPENSATION, y + MOUSE_POSITION_OFFSET - MARGIN_COMPENSATION];
    }
}

export const voiceTypingService = new VoiceTypingService();
