import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment, AppMode, ProcessingStatus, AppConfig } from '../types/transcript';
import { findSegmentForTime } from '../utils/segmentUtils';

interface TranscriptState {
    // Segment data (source of truth)
    segments: TranscriptSegment[];

    // UI state
    activeSegmentId: string | null;
    editingSegmentId: string | null;
    mode: AppMode;
    processingStatus: ProcessingStatus;
    processingProgress: number; // 0-100

    // Audio state
    audioFile: File | null;
    audioUrl: string | null;
    currentTime: number;
    isPlaying: boolean;

    // Config
    config: AppConfig;

    // Segment CRUD operations
    addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;
    upsertSegment: (segment: TranscriptSegment) => void;
    updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;
    deleteSegment: (id: string) => void;
    mergeSegments: (id1: string, id2: string) => void;
    setSegments: (segments: TranscriptSegment[]) => void;
    clearSegments: () => void;

    // UI actions
    setActiveSegmentId: (id: string | null) => void;
    setEditingSegmentId: (id: string | null) => void;
    setMode: (mode: AppMode) => void;
    setProcessingStatus: (status: ProcessingStatus) => void;
    setProcessingProgress: (progress: number) => void;

    // Audio actions
    setAudioFile: (file: File | null) => void;
    setAudioUrl: (url: string | null) => void;
    setCurrentTime: (time: number) => void;
    setIsPlaying: (isPlaying: boolean) => void;

    // Config actions
    setConfig: (config: Partial<AppConfig>) => void;
}


const DEFAULT_CONFIG: AppConfig = {
    streamingModelPath: '',
    offlineModelPath: '',
    language: 'en',
    appLanguage: 'auto',
    enabledITNModels: ['itn-zh-number'], // Default to having the number ITN enabled
    itnRulesOrder: ['itn-zh-number', 'itn-new-heteronym', 'itn-phone'],
    enableITN: true, // Keep for legacy check
    punctuationModelPath: '',
    theme: 'auto',
    font: 'system',
};

export const useTranscriptStore = create<TranscriptState>((set, get) => ({
    // Initial state
    segments: [],
    activeSegmentId: null,
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
        set({ segments: segments.sort((a, b) => a.start - b.start) });
    },

    clearSegments: () => {
        set({ segments: [], activeSegmentId: null, editingSegmentId: null });
    },

    // UI actions
    setActiveSegmentId: (id) => set({ activeSegmentId: id }),
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
        // Find active segment based on current time
        const activeSegment = findSegmentForTime(state.segments, time);
        set({
            currentTime: time,
            activeSegmentId: activeSegment?.id || null,
        });
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
export const useSegments = () => useTranscriptStore((state) => state.segments);
export const useActiveSegmentId = () => useTranscriptStore((state) => state.activeSegmentId);
export const useMode = () => useTranscriptStore((state) => state.mode);
export const useProcessingStatus = () => useTranscriptStore((state) => state.processingStatus);
export const useConfig = () => useTranscriptStore((state) => state.config);
