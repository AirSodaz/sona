import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment, AppMode, ProcessingStatus, AppConfig } from '../types/transcript';
import { findSegmentAndIndexForTime } from '../utils/segmentUtils';

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
    /** Whether recording is currently paused. */
    isPaused: boolean;
    /** Timestamp of the last user-initiated seek. */
    lastSeekTimestamp: number;
    /** Current seek request. */
    seekRequest: { time: number; timestamp: number } | null;

    // History tracking
    /** ID of the history item the current segments originate from. */
    sourceHistoryId: string | null;

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
    loadTranscript: (segments: TranscriptSegment[], sourceHistoryId: string | null) => void;

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


const DEFAULT_CONFIG: AppConfig = {
    // streamingModelPath removed
    offlineModelPath: '',
    language: 'auto',
    appLanguage: 'auto',
    enabledITNModels: ['itn-zh-number'], // Default to having the number ITN enabled
    itnRulesOrder: ['itn-zh-number'],
    enableITN: true, // Keep for legacy check
    enableTimeline: true,
    punctuationModelPath: '',
    vadModelPath: '',
    theme: 'auto',
    font: 'system',
    vadBufferSize: 5,
    maxConcurrent: 2,
};

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
    isPaused: false,
    lastSeekTimestamp: 0,
    seekRequest: null,
    sourceHistoryId: null,
    config: DEFAULT_CONFIG,

    // History tracking
    setSourceHistoryId: (id) => set({ sourceHistoryId: id }),

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
            return {
                segments: result.segments,
                activeSegmentId: segment.id,
                activeSegmentIndex: result.index
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

        const mergedSegment: TranscriptSegment = {
            id: first.id,
            start: first.start,
            end: second.end,
            text: `${first.text} ${second.text}`.trim(),
            isFinal: first.isFinal && second.isFinal,
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

    loadTranscript: (segments, sourceHistoryId) => {
        set({
            segments: segments.sort((a, b) => a.start - b.start),
            sourceHistoryId,
            activeSegmentIndex: -1,
            activeSegmentId: null,
            editingSegmentId: null
        });
    },

    clearSegments: () => {
        set({
            segments: [],
            activeSegmentId: null,
            activeSegmentIndex: -1,
            editingSegmentId: null,
            sourceHistoryId: null
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

    // Config actions
    setConfig: (config) => {
        set((state) => ({
            config: { ...state.config, ...config },
        }));
    },
}));

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
