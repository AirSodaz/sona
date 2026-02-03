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

    // Audio state
    /** The loaded audio file object. */
    audioFile: File | null;
    /** URL of the loaded audio. */
    audioUrl: string | null;
    /** Current playback time in seconds. */
    currentTime: number;
    /** Whether audio is currently playing. */
    isPlaying: boolean;

    // Config
    /** Application configuration. */
    config: AppConfig;

    // Segment CRUD operations
    /**
     * Adds a new segment.
     * @param segment - The segment data (excluding ID).
     * @return The ID of the newly created segment.
     */
    addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;

    /**
     * Updates an existing segment or adds it if it doesn't exist.
     * Optimized for streaming usage.
     * @param segment - The segment to upsert.
     */
    upsertSegment: (segment: TranscriptSegment) => void;

    /**
     * Updates specific fields of a segment.
     * @param id - The ID of the segment to update.
     * @param updates - Partial segment data.
     */
    updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;

    /**
     * Deletes a segment by ID.
     * @param id - The ID of the segment to delete.
     */
    deleteSegment: (id: string) => void;

    /**
     * Merges two segments into one.
     * @param id1 - The ID of the first segment.
     * @param id2 - The ID of the second segment.
     */
    mergeSegments: (id1: string, id2: string) => void;

    /**
     * Replaces all segments with a new list.
     * @param segments - The new list of segments.
     */
    setSegments: (segments: TranscriptSegment[]) => void;

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

    // Audio actions
    /**
     * Sets the current audio file and generates a URL.
     * @param file - The file object or null.
     */
    setAudioFile: (file: File | null) => void;
    /**
     * Sets the audio URL directly.
     * @param url - The audio URL or null.
     */
    setAudioUrl: (url: string | null) => void;
    /**
     * Sets the current playback time and updates active segment.
     * @param time - Current time in seconds.
     */
    setCurrentTime: (time: number) => void;
    /** Sets the playing state. */
    setIsPlaying: (isPlaying: boolean) => void;

    // Config actions
    /**
     * Updates the application configuration.
     * @param config - Partial configuration updates.
     */
    setConfig: (config: Partial<AppConfig>) => void;
}


const DEFAULT_CONFIG: AppConfig = {
    streamingModelPath: '',
    offlineModelPath: '',
    language: 'en',
    appLanguage: 'auto',
    enabledITNModels: ['itn-zh-number'], // Default to having the number ITN enabled
    itnRulesOrder: ['itn-zh-number'],
    enableITN: true, // Keep for legacy check
    punctuationModelPath: '',
    vadModelPath: '',
    theme: 'auto',
    font: 'system',
    vadBufferSize: 5,
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
    audioFile: null,
    audioUrl: null,
    currentTime: 0,
    isPlaying: false,
    config: DEFAULT_CONFIG,

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
            // Optimization: Check last segment first (common case for streaming updates)
            const len = state.segments.length;
            if (len > 0) {
                const lastIndex = len - 1;
                const lastSegment = state.segments[lastIndex];
                if (lastSegment.id === segment.id) {
                    const newSegments = [...state.segments];
                    newSegments[lastIndex] = segment;
                    return { segments: newSegments };
                }
            }

            const index = state.segments.findIndex((s) => s.id === segment.id);
            if (index !== -1) {
                const newSegments = [...state.segments];
                newSegments[index] = segment;
                return { segments: newSegments };
            }

            // Optimization: Append at end if chronological (avoid sort)
            if (len === 0 || state.segments[len - 1].start <= segment.start) {
                return {
                    segments: [...state.segments, segment]
                };
            }

            return {
                segments: [...state.segments, segment].sort((a, b) => a.start - b.start),
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

    clearSegments: () => {
        set({
            segments: [],
            activeSegmentId: null,
            activeSegmentIndex: -1,
            editingSegmentId: null
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

    // Config actions
    setConfig: (config) => {
        set((state) => ({
            config: { ...state.config, ...config },
        }));
    },
}));

// Selector hooks for better performance
/** Selector for accessing segments. */
export const useSegments = () => useTranscriptStore((state) => state.segments);
/** Selector for accessing the active segment ID. */
export const useActiveSegmentId = () => useTranscriptStore((state) => state.activeSegmentId);
/** Selector for accessing the current mode. */
export const useMode = () => useTranscriptStore((state) => state.mode);
/** Selector for accessing the processing status. */
export const useProcessingStatus = () => useTranscriptStore((state) => state.processingStatus);
/** Selector for accessing the current configuration. */
export const useConfig = () => useTranscriptStore((state) => state.config);
