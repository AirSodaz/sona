import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import {
    TranscriptSegment,
    AppMode,
    ProcessingStatus,
    AppConfig,
    DEFAULT_SUMMARY_TEMPLATE_ID,
    HistorySummaryPayload,
    SummaryTemplateId,
    TranscriptSummaryState
} from '../types/transcript';
import { useConfigStore, DEFAULT_CONFIG } from './configStore';
import { useProjectStore } from './projectStore';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { findSegmentAndIndexForTime } from '../utils/segmentUtils';
import { coerceSummaryTemplateId } from '../utils/summaryTemplates';
import { areSpeakerTagsEqual } from '../types/speaker';
// createLlmSettings is now used in configStore

/** State interface for the transcript store. */
interface TranscriptState {
    // Segment data (source of truth)
    /** List of transcript segments. */
    segments: TranscriptSegment[];

    // UI state
    /** ID of the currently active segment (during playback). */
    activeSegmentId: string | null;
    /** Index of the active segment (optimization for sequential playback). */
    activeSegmentIndex: number;
    /** ID of the segment currently being edited. */
    editingSegmentId: string | null;
    /** Current application mode. */
    mode: AppMode;
    /** Status of batch processing. */
    processingStatus: ProcessingStatus;
    /** Progress of processing (0-100). */
    processingProgress: number;
    /** Set of segment IDs currently being re-aligned. */
    aligningSegmentIds: Set<string>;

    // Audio state
    /** The loaded audio file object. */
    audioFile: File | null;
    /** URL of the loaded audio. */
    audioUrl: string | null;
    /** Current playback time in seconds. */
    currentTime: number;
    /** Whether audio is currently playing. */
    isPlaying: boolean;
    /** Whether recording is currently active. */
    isRecording: boolean;
    /** Whether caption mode is active. */
    isCaptionMode: boolean;
    /** Whether recording is currently paused. */
    isPaused: boolean;
    /** Timestamp of the last user-initiated seek. */
    lastSeekTimestamp: number;
    /** Current seek request. */
    seekRequest: { time: number; timestamp: number } | null;

    // History tracking
    /** ID of the history item the current segments originate from. */
    sourceHistoryId: string | null;
    /** Title of the current transcription. */
    title: string | null;
    /** Icon of the current transcription (emoji or system:icon). */
    icon: string | null;

    // LLM states mapped by historyId
    /**
     * Record of LLM states (translation, polishing) mapped by sourceHistoryId.
     * Use 'current' for the active unsaved recording.
     */
    llmStates: Record<string, LlmState>;

    /**
     * Record of transcript summary states mapped by sourceHistoryId.
     * Use 'current' for the active unsaved recording.
     */
    summaryStates: Record<string, TranscriptSummaryState>;

    /**
     * UI-only auto-save status mapped by historyId.
     */
    autoSaveStates: Record<string, AutoSaveState>;

    // Config
    /** Application configuration. */
    config: AppConfig;

    // History tracking actions
    /**
     * Sets the source history item ID for the current segments.
     *
     * @param id The history item ID or null.
     */
    setSourceHistoryId: (id: string | null) => void;

    /**
     * Sets the title for the current transcription.
     *
     * @param title The new title.
     */
    setTitle: (title: string | null) => void;

    /**
     * Sets the icon for the current transcription.
     *
     * @param icon The new icon.
     */
    setIcon: (icon: string | null) => void;

    // Segment CRUD operations
    /**
     * Adds a new segment.
     *
     * @param segment The segment data (excluding ID).
     * @return The ID of the newly created segment.
     */
    addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;

    /**
     * Updates an existing segment or adds it if it doesn't exist.
     *
     * Optimized for streaming usage.
     *
     * @param segment The segment to upsert.
     */
    upsertSegment: (segment: TranscriptSegment) => void;

    /**
     * Updates an existing segment or adds it, AND sets it as active.
     * Optimized to perform both operations in a single store update.
     *
     * @param segment The segment to upsert and set active.
     */
    upsertSegmentAndSetActive: (segment: TranscriptSegment) => void;

    /**
     * Updates specific fields of a segment.
     *
     * @param id The ID of the segment to update.
     * @param updates Partial segment data.
     */
    updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;

    /**
     * Deletes a segment by ID.
     *
     * @param id The ID of the segment to delete.
     */
    deleteSegment: (id: string) => void;

    /**
     * Merges two segments into one.
     *
     * @param id1 The ID of the first segment.
     * @param id2 The ID of the second segment.
     */
    mergeSegments: (id1: string, id2: string) => void;

    /**
     * Replaces all segments with a new list.
     *
     * @param segments The new list of segments.
     */
    setSegments: (segments: TranscriptSegment[]) => void;

    /**
     * Atomically loads segments and sets the source history ID.
     * Prevents auto-save race conditions when switching items.
     */
    loadTranscript: (segments: TranscriptSegment[], sourceHistoryId: string | null, title?: string | null, icon?: string | null) => void;

    /**
     * Mark the last segment as final if it isn't already.
     * Useful when stopping a recording.
     */
    finalizeLastSegment: () => void;

    /** Clears all segments and resets segment-related state. */
    clearSegments: () => void;

    // UI actions
    /** Sets the active segment ID. */
    setActiveSegmentId: (id: string | null, index?: number) => void;
    /** Sets the editing segment ID. */
    setEditingSegmentId: (id: string | null) => void;
    /** Sets the application mode. */
    setMode: (mode: AppMode) => void;
    /** Sets the processing status. */
    setProcessingStatus: (status: ProcessingStatus) => void;
    /** Sets the processing progress. */
    setProcessingProgress: (progress: number) => void;
    /**
     * Adds a segment ID to the aligning set.
     *
     * @param id The segment ID being aligned.
     */
    addAligningSegmentId: (id: string) => void;
    /**
     * Removes a segment ID from the aligning set.
     *
     * @param id The segment ID that finished aligning.
     */
    removeAligningSegmentId: (id: string) => void;

    // LLM state actions
    /**
     * Gets the LLM state for a specific history ID.
     * If no ID is provided, uses the current sourceHistoryId or 'current'.
     * Returns a default empty state if not found.
     */
    getLlmState: (historyId?: string) => LlmState;

    /**
     * Updates the LLM state for a specific history ID.
     * If no ID is provided, updates the current sourceHistoryId or 'current'.
     */
    updateLlmState: (updates: Partial<LlmState>, historyId?: string) => void;

    /**
     * Gets the summary state for a specific history ID.
     * If no ID is provided, uses the current sourceHistoryId or 'current'.
     */
    getSummaryState: (historyId?: string) => TranscriptSummaryState;

    /**
     * Replaces the summary state for a specific history ID.
     * If no ID is provided, updates the current sourceHistoryId or 'current'.
     */
    setSummaryState: (summaryState: Partial<TranscriptSummaryState>, historyId?: string) => void;

    /**
     * Updates the summary state for a specific history ID.
     * If no ID is provided, updates the current sourceHistoryId or 'current'.
     */
    updateSummaryState: (updates: Partial<TranscriptSummaryState>, historyId?: string) => void;

    /**
     * Updates the active summary template for the current transcript or a specific history item.
     */
    setActiveSummaryTemplate: (templateId: SummaryTemplateId, historyId?: string) => void;

    /**
     * Hydrates persisted summary payload into store state for a specific history ID.
     */
    hydrateSummaryState: (payload: HistorySummaryPayload, historyId?: string) => void;

    /**
     * Clears a summary state entry.
     */
    clearSummaryState: (historyId?: string) => void;

    /**
     * Updates the auto-save status for a specific history item.
     */
    setAutoSaveState: (historyId: string, status: AutoSaveStatus) => void;

    /**
     * Clears a specific auto-save state entry.
     */
    clearAutoSaveState: (historyId?: string) => void;

    // Legacy actions for backward compatibility (updates current active state)
    setIsTranslationVisible: (visible: boolean) => void;
    setIsTranslating: (translating: boolean) => void;
    setTranslationProgress: (progress: number) => void;
    setIsPolishing: (polishing: boolean) => void;
    setPolishProgress: (progress: number) => void;

    // Audio actions
    /**
     * Sets the current audio file and generates a URL.
     *
     * @param file The file object or null.
     */
    setAudioFile: (file: File | null) => void;
    /**
     * Sets the audio URL directly.
     *
     * @param url The audio URL or null.
     */
    setAudioUrl: (url: string | null) => void;
    /**
     * Sets the current playback time and updates active segment.
     *
     * @param time Current time in seconds.
     */
    setCurrentTime: (time: number) => void;
    /** Sets the playing state. */
    setIsPlaying: (isPlaying: boolean) => void;
    /** Sets the recording state. */
    setIsRecording: (isRecording: boolean) => void;
    /** Sets the caption mode state. */
    setIsCaptionMode: (isCaptionMode: boolean) => void;
    /** Sets the recording paused state. */
    setIsPaused: (isPaused: boolean) => void;
    /**
     * Requests a seek to a specific time.
     * Updates current time and triggers a seek request for the audio player.
     *
     * @param time The time to seek to in seconds.
     */
    requestSeek: (time: number) => void;

    // Config actions
    /**
     * Updates the application configuration.
     *
     * @param config Partial configuration updates.
     */
    setConfig: (config: Partial<AppConfig>) => void;
}


export interface LlmState {
    isTranslating: boolean;
    translationProgress: number;
    isTranslationVisible: boolean;
    isPolishing: boolean;
    polishProgress: number;
    isRetranscribing: boolean;
    retranscribeProgress: number;
}

export type AutoSaveStatus = 'saving' | 'saved' | 'error';

export interface AutoSaveState {
    status: AutoSaveStatus;
    updatedAt: number;
}

const DEFAULT_LLM_STATE: LlmState = {
    isTranslating: false,
    translationProgress: 0,
    isTranslationVisible: false,
    isPolishing: false,
    polishProgress: 0,
    isRetranscribing: false,
    retranscribeProgress: 0,
};

const DEFAULT_SUMMARY_STATE: TranscriptSummaryState = {
    activeTemplateId: DEFAULT_SUMMARY_TEMPLATE_ID,
    record: undefined,
    streamingContent: undefined,
    isGenerating: false,
    generationProgress: 0,
};

function createDefaultSummaryState(): TranscriptSummaryState {
    return {
        ...DEFAULT_SUMMARY_STATE,
    };
}


/**
 * Zustand store for managing transcript data, audio state, and application configuration.
 */
export const useTranscriptStore = create<TranscriptState>((set, get) => ({
    // Initial state
    segments: [],
    activeSegmentId: null,
    activeSegmentIndex: -1,
    editingSegmentId: null,
    mode: 'live',
    processingStatus: 'idle',
    processingProgress: 0,
    aligningSegmentIds: new Set<string>(),
    audioFile: null,
    audioUrl: null,
    currentTime: 0,
    isPlaying: false,
    isRecording: false,
    isCaptionMode: false,
    isPaused: false,
    lastSeekTimestamp: 0,
    seekRequest: null,
    sourceHistoryId: null,
    title: null,
    icon: null,
    llmStates: {},
    summaryStates: {},
    autoSaveStates: {},
    config: DEFAULT_CONFIG,

    // History tracking
    setSourceHistoryId: (id) => set((state) => {
        if (!id || !state.summaryStates.current) {
            return { sourceHistoryId: id };
        }

        const currentSummaryState = state.summaryStates.current;
        const existingTargetState = state.summaryStates[id];
        const summaryStates = { ...state.summaryStates };

        summaryStates[id] = existingTargetState
            ? {
                ...existingTargetState,
                ...currentSummaryState,
                record: currentSummaryState.record || existingTargetState.record,
            }
            : currentSummaryState;

        delete summaryStates.current;

        return {
            sourceHistoryId: id,
            summaryStates,
        };
    }),

    setTitle: (title) => set({ title }),

    setIcon: (icon) => set({ icon }),

    // Segment CRUD
    addSegment: (segment) => {
        const id = uuidv4();
        const newSegment: TranscriptSegment = { ...segment, id };
        set((state) => ({
            segments: [...state.segments, newSegment].sort((a, b) => a.start - b.start),
        }));
        return id;
    },

    upsertSegment: (segment) => {
        set((state) => {
            const result = calculateSegmentUpdate(state.segments, segment);
            return { segments: result.segments };
        });
    },

    upsertSegmentAndSetActive: (segment) => {
        set((state) => {
            const result = calculateSegmentUpdate(state.segments, segment);
            // In caption mode, trim old segments to prevent unbounded growth
            const MAX_CAPTION_SEGMENTS = 50;
            const needsTrim = state.isCaptionMode && result.segments.length > MAX_CAPTION_SEGMENTS;
            const segments = needsTrim
                ? result.segments.slice(-MAX_CAPTION_SEGMENTS)
                : result.segments;

            return {
                segments,
                activeSegmentId: segment.id,
                activeSegmentIndex: needsTrim ? segments.length - 1 : result.index
            };
        });
    },

    updateSegment: (id, updates) => {
        set((state) => ({
            segments: state.segments.map((seg) =>
                seg.id === id ? { ...seg, ...updates } : seg
            ),
        }));
    },

    deleteSegment: (id) => {
        set((state) => ({
            segments: state.segments.filter((seg) => seg.id !== id),
            activeSegmentId: state.activeSegmentId === id ? null : state.activeSegmentId,
            editingSegmentId: state.editingSegmentId === id ? null : state.editingSegmentId,
        }));
    },

    mergeSegments: (id1, id2) => {
        const state = get();
        const seg1 = state.segments.find((s) => s.id === id1);
        const seg2 = state.segments.find((s) => s.id === id2);

        if (!seg1 || !seg2) return;

        // Ensure seg1 comes before seg2
        const [first, second] = seg1.start <= seg2.start ? [seg1, seg2] : [seg2, seg1];
        if (!areSpeakerTagsEqual(first.speaker, second.speaker)) {
            return;
        }

        const mergedSegment: TranscriptSegment = {
            id: first.id,
            start: first.start,
            end: second.end,
            text: `${first.text} ${second.text}`.trim(),
            isFinal: first.isFinal && second.isFinal,
            speaker: first.speaker,
        };

        set((state) => ({
            segments: state.segments
                .filter((s) => s.id !== second.id)
                .map((s) => (s.id === first.id ? mergedSegment : s)),
        }));
    },

    setSegments: (segments) => {
        set({
            segments: segments.sort((a, b) => a.start - b.start),
            activeSegmentIndex: -1 // Reset index on bulk update
        });
    },

    loadTranscript: (segments: TranscriptSegment[], sourceHistoryId: string | null, title?: string | null, icon?: string | null) => {
        set({
            segments: segments.sort((a, b) => a.start - b.start),
            sourceHistoryId,
            title: title || '',
            icon: icon || null,
            activeSegmentId: null,
            editingSegmentId: null,
        });
    },

    finalizeLastSegment: () => {
        set((state) => {
            if (state.segments.length === 0) return {};
            const lastIndex = state.segments.length - 1;
            const lastSegment = state.segments[lastIndex];
            if (lastSegment.isFinal) return {};

            const newSegments = [...state.segments];
            newSegments[lastIndex] = { ...lastSegment, isFinal: true };
            return { segments: newSegments };
        });
    },

    clearSegments: () => {
        set((state) => {
            const summaryStates = { ...state.summaryStates };
            delete summaryStates.current;

            return {
            segments: [],
            activeSegmentId: null,
            activeSegmentIndex: -1,
            editingSegmentId: null,
            sourceHistoryId: null,
            title: null,
            icon: null,
            summaryStates,
            };
        });
    },

    // UI actions
    setActiveSegmentId: (id, index = -1) => set({
        activeSegmentId: id,
        activeSegmentIndex: index
    }),
    setEditingSegmentId: (id) => set({ editingSegmentId: id }),
    setMode: (mode) => set({ mode }),
    setProcessingStatus: (status) => set({ processingStatus: status }),
    setProcessingProgress: (progress) => set({ processingProgress: progress }),
    addAligningSegmentId: (id) => set((state) => {
        const next = new Set(state.aligningSegmentIds);
        next.add(id);
        return { aligningSegmentIds: next };
    }),
    removeAligningSegmentId: (id) => set((state) => {
        const next = new Set(state.aligningSegmentIds);
        next.delete(id);
        return { aligningSegmentIds: next };
    }),

    // LLM state actions
    getLlmState: (historyId) => {
        const state = get();
        const id = historyId || state.sourceHistoryId || 'current';
        return state.llmStates[id] || { ...DEFAULT_LLM_STATE };
    },

    updateLlmState: (updates, historyId) => {
        set((state) => {
            const id = historyId || state.sourceHistoryId || 'current';
            const currentState = state.llmStates[id] || { ...DEFAULT_LLM_STATE };
            return {
                llmStates: {
                    ...state.llmStates,
                    [id]: { ...currentState, ...updates }
                }
            };
        });
    },

    getSummaryState: (historyId) => {
        const state = get();
        const id = historyId || state.sourceHistoryId || 'current';
        return state.summaryStates[id] || createDefaultSummaryState();
    },

    setSummaryState: (summaryState, historyId) => {
        set((state) => {
            const id = historyId || state.sourceHistoryId || 'current';
            return {
                summaryStates: {
                    ...state.summaryStates,
                    [id]: {
                        ...createDefaultSummaryState(),
                        ...summaryState,
                        record: summaryState.record,
                    },
                },
            };
        });
    },

    updateSummaryState: (updates, historyId) => {
        set((state) => {
            const id = historyId || state.sourceHistoryId || 'current';
            const currentState = state.summaryStates[id] || createDefaultSummaryState();
            return {
                summaryStates: {
                    ...state.summaryStates,
                    [id]: {
                        ...currentState,
                        ...updates,
                    },
                },
            };
        });
    },

    setActiveSummaryTemplate: (templateId, historyId) => {
        get().updateSummaryState({ activeTemplateId: templateId }, historyId);
    },

    hydrateSummaryState: (payload, historyId) => {
        const customTemplates = get().config.summaryCustomTemplates;
        const payloadWithLegacyFields = payload as HistorySummaryPayload & {
            activeTemplate?: string;
            records?: Record<string, any>;
        };
        const activeTemplateId = coerceSummaryTemplateId(
            payloadWithLegacyFields.activeTemplateId || payloadWithLegacyFields.activeTemplate,
            customTemplates,
        );

        let record = payload.record as any;
        if (!record && payloadWithLegacyFields.records) {
            const records = payloadWithLegacyFields.records;
            record = records[
                payloadWithLegacyFields.activeTemplateId
                || payloadWithLegacyFields.activeTemplate
                || activeTemplateId
            ] || Object.values(records)[0] as any;
        }

        if (record) {
            record = {
                ...record,
                templateId: coerceSummaryTemplateId(
                    record.templateId || record.template || activeTemplateId,
                    customTemplates,
                ),
            };
        }

        get().setSummaryState({
            activeTemplateId,
            record,
            streamingContent: undefined,
            isGenerating: false,
            generationProgress: 0,
        }, historyId);
    },

    clearSummaryState: (historyId) => {
        set((state) => {
            const id = historyId || state.sourceHistoryId || 'current';
            if (!state.summaryStates[id]) {
                return state;
            }

            const summaryStates = { ...state.summaryStates };
            delete summaryStates[id];
            return { summaryStates };
        });
    },

    setAutoSaveState: (historyId, status) => {
        if (!historyId || historyId === 'current') {
            return;
        }

        set((state) => ({
            autoSaveStates: {
                ...state.autoSaveStates,
                [historyId]: {
                    status,
                    updatedAt: Date.now(),
                },
            },
        }));
    },

    clearAutoSaveState: (historyId) => {
        set((state) => {
            const id = historyId || state.sourceHistoryId || 'current';
            if (!id || id === 'current' || !state.autoSaveStates[id]) {
                return state;
            }

            const autoSaveStates = { ...state.autoSaveStates };
            delete autoSaveStates[id];
            return { autoSaveStates };
        });
    },

    // Legacy actions mapping to current LLM state
    setIsTranslationVisible: (visible) => get().updateLlmState({ isTranslationVisible: visible }),
    setIsTranslating: (translating) => get().updateLlmState({ isTranslating: translating }),
    setTranslationProgress: (progress) => get().updateLlmState({ translationProgress: progress }),
    setIsPolishing: (polishing) => get().updateLlmState({ isPolishing: polishing }),
    setPolishProgress: (progress) => get().updateLlmState({ polishProgress: progress }),

    // Audio actions
    setAudioFile: (file) => {
        const state = get();
        // Revoke previous URL to prevent memory leaks
        if (state.audioUrl) {
            URL.revokeObjectURL(state.audioUrl);
        }
        const url = file ? URL.createObjectURL(file) : null;
        set({
            audioFile: file,
            audioUrl: url,
            isPlaying: false,
            currentTime: 0
        });
    },
    setAudioUrl: (url) => set({
        audioUrl: url,
        isPlaying: false,
        currentTime: 0
    }),
    setCurrentTime: (time) => {
        const state = get();
        // Find active segment based on current time, using previous index as hint
        const { segment, index } = findSegmentAndIndexForTime(state.segments, time, state.activeSegmentIndex);

        if (segment?.id !== state.activeSegmentId) {
            set({
                currentTime: time,
                activeSegmentId: segment?.id || null,
                activeSegmentIndex: index
            });
        } else {
            set({ currentTime: time });
        }
    },
    setIsPlaying: (isPlaying) => set({ isPlaying }),
    setIsRecording: (isRecording) => set({ isRecording }),
    setIsCaptionMode: (isCaptionMode) => set({ isCaptionMode }),
    setIsPaused: (isPaused) => set({ isPaused }),
    requestSeek: (time) => {
        const state = get();
        // Optimistically update current time and active segment
        state.setCurrentTime(time);

        set({
            seekRequest: { time, timestamp: Date.now() },
            lastSeekTimestamp: Date.now()
        });
    },

    // Config actions (delegated to configStore, synced back for reactivity)
    setConfig: (patch) => {
        useConfigStore.getState().setConfig(patch);
        const effectiveConfig = resolveEffectiveConfig(
            useConfigStore.getState().config,
            useProjectStore.getState().getActiveProject(),
        );
        set({ config: effectiveConfig });
    },
}));

function syncEffectiveConfigToTranscriptStore() {
    const projectState = useProjectStore.getState();
    const activeProject = typeof projectState.getActiveProject === 'function'
        ? projectState.getActiveProject()
        : null;
    const effectiveConfig = resolveEffectiveConfig(
        useConfigStore.getState().config,
        activeProject,
    );
    useTranscriptStore.setState({ config: effectiveConfig });
}

// Keep transcriptStore.config in sync with the active project-aware config.
// New code should prefer transcriptStore.config for runtime workflow behavior.
useConfigStore.subscribe((state) => {
    const projectState = useProjectStore.getState();
    const activeProject = typeof projectState.getActiveProject === 'function'
        ? projectState.getActiveProject()
        : null;
    useTranscriptStore.setState({
        config: resolveEffectiveConfig(state.config, activeProject),
    });
});

if (typeof useProjectStore.subscribe === 'function') {
    useProjectStore.subscribe(() => {
        syncEffectiveConfigToTranscriptStore();
    });
}

/**
 * Calculates the new segments array and the index of the updated/inserted segment.
 *
 * @param segments The current list of segments.
 * @param segment The segment to update or insert.
 * @return An object containing the new segments array and the index of the segment.
 */
function calculateSegmentUpdate(segments: TranscriptSegment[], segment: TranscriptSegment): { segments: TranscriptSegment[], index: number } {
    const len = segments.length;

    // 1. Update last segment (Most common case in streaming)
    if (len > 0) {
        const lastIndex = len - 1;
        const lastSegment = segments[lastIndex];
        if (lastSegment.id === segment.id) {
            const newSegments = [...segments];
            newSegments[lastIndex] = segment;
            return { segments: newSegments, index: lastIndex };
        }
    }

    // 2. Update existing segment (middle)
    const index = segments.findIndex((s) => s.id === segment.id);
    if (index !== -1) {
        const newSegments = [...segments];
        newSegments[index] = segment;
        return { segments: newSegments, index };
    }

    // 3. Append (Next most common)
    if (len === 0 || segments[len - 1].start <= segment.start) {
        return {
            segments: [...segments, segment],
            index: len // New index is at the end (old length)
        };
    }

    // 4. Insert/Sort (Rare)
    const newSegments = [...segments, segment].sort((a, b) => a.start - b.start);
    const newIndex = newSegments.findIndex(s => s.id === segment.id);

    return {
        segments: newSegments,
        index: newIndex
    };
}
