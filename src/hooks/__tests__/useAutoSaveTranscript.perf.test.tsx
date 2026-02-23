import { renderHook } from '@testing-library/react';
import { useAutoSaveTranscript } from '../useAutoSaveTranscript';
import { useTranscriptStore } from '../../stores/transcriptStore';
import * as segmentUtils from '../../utils/segmentUtils';
import { vi, describe, it, expect, beforeEach } from 'vitest';

// Mock the segmentUtils module to spy on computeSegmentsFingerprint
vi.mock('../../utils/segmentUtils', async (importOriginal) => {
    const actual = await importOriginal<typeof segmentUtils>();
    return {
        ...actual,
        computeSegmentsFingerprint: vi.fn((segments) => actual.computeSegmentsFingerprint(segments)),
    };
});

describe('useAutoSaveTranscript Performance', () => {
    beforeEach(() => {
        // Reset store state relevant to the hook
        useTranscriptStore.setState({
            segments: [],
            sourceHistoryId: 'test-id',
            currentTime: 0,
            isPlaying: false,
        });
        vi.clearAllMocks();
    });

    it('should NOT call computeSegmentsFingerprint on unrelated store updates (optimized)', () => {
        // 1. Mount the hook
        renderHook(() => useAutoSaveTranscript());

        // Initial call happens on mount to set lastFingerprintRef
        // And possibly one more if subscription fires immediately (it shouldn't usually, but depends on implementation)
        // Let's establish the count after mount.
        const callsAfterMount = vi.mocked(segmentUtils.computeSegmentsFingerprint).mock.calls.length;
        expect(callsAfterMount).toBeGreaterThanOrEqual(1);

        // 2. Trigger unrelated state changes (currentTime, isPlaying)
        // These do NOT change 'segments' or 'sourceHistoryId'
        useTranscriptStore.setState({ currentTime: 1.5 });
        useTranscriptStore.setState({ currentTime: 2.0 });
        useTranscriptStore.setState({ isPlaying: true });
        useTranscriptStore.setState({ isPlaying: false });

        // 3. Assert that computeSegmentsFingerprint was NOT called for unrelated updates
        // With optimization, it should not be called.
        const callsAfterUpdates = vi.mocked(segmentUtils.computeSegmentsFingerprint).mock.calls.length;
        expect(callsAfterUpdates).toBe(callsAfterMount);
    });

    it('should call computeSegmentsFingerprint when segments change', () => {
        renderHook(() => useAutoSaveTranscript());
        const callsAfterMount = vi.mocked(segmentUtils.computeSegmentsFingerprint).mock.calls.length;

        // Act - Change segments
        const newSegments = [{ id: '1', text: 'test', start: 0, end: 1, isFinal: true }];
        // @ts-ignore - Partial segment mock is fine for this test
        useTranscriptStore.setState({ segments: newSegments });

        // Assert - Should be called once for the update
        const callsAfterUpdates = vi.mocked(segmentUtils.computeSegmentsFingerprint).mock.calls.length;
        expect(callsAfterUpdates).toBe(callsAfterMount + 1);
    });
});
