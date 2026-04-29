import { describe, it, expect, beforeEach } from 'vitest';
import { useTranscriptStore } from '../transcriptStore';
import { TranscriptSegment } from '../../types/transcript';
import { v4 as uuidv4 } from 'uuid';

describe('TranscriptStore', () => {
    beforeEach(() => {
        useTranscriptStore.getState().clearSegments();
        useTranscriptStore.setState({
            autoSaveStates: {},
            sourceHistoryId: null,
        });
    });

    describe('setActiveSegmentId', () => {
        it('should reset activeSegmentIndex to -1 when index is not provided', () => {
            const id1 = uuidv4();
            const id2 = uuidv4();
            const segments: TranscriptSegment[] = [
                { id: id1, text: 'First', start: 0, end: 1, isFinal: true },
                { id: id2, text: 'Second', start: 1, end: 2, isFinal: true }
            ];

            useTranscriptStore.getState().setSegments(segments);

            // Set active segment without index
            useTranscriptStore.getState().setActiveSegmentId(id2);

            expect(useTranscriptStore.getState().activeSegmentId).toBe(id2);
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(-1);
        });

        it('should set activeSegmentIndex when index is provided', () => {
            const id1 = uuidv4();
            const id2 = uuidv4();
            const segments: TranscriptSegment[] = [
                { id: id1, text: 'First', start: 0, end: 1, isFinal: true },
                { id: id2, text: 'Second', start: 1, end: 2, isFinal: true }
            ];

            useTranscriptStore.getState().setSegments(segments);

            // Set active segment with index
            useTranscriptStore.getState().setActiveSegmentId(id2, 1);

            expect(useTranscriptStore.getState().activeSegmentId).toBe(id2);
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(1);
        });
    });

    describe('requestSeek', () => {
        it('should update currentTime and set seekRequest', () => {
            const time = 10.5;
            useTranscriptStore.getState().requestSeek(time);

            expect(useTranscriptStore.getState().currentTime).toBe(time);
            expect(useTranscriptStore.getState().seekRequest).toEqual({
                time,
                timestamp: expect.any(Number)
            });
            expect(useTranscriptStore.getState().lastSeekTimestamp).toEqual(expect.any(Number));
        });

        it('should update activeSegmentId based on seek time', () => {
            const id1 = uuidv4();
            const id2 = uuidv4();
            const segments: TranscriptSegment[] = [
                { id: id1, text: 'First', start: 0, end: 5, isFinal: true },
                { id: id2, text: 'Second', start: 5, end: 10, isFinal: true }
            ];

            useTranscriptStore.getState().setSegments(segments);

            // Seek to middle of second segment
            useTranscriptStore.getState().requestSeek(7.5);

            expect(useTranscriptStore.getState().currentTime).toBe(7.5);
            expect(useTranscriptStore.getState().activeSegmentId).toBe(id2);
            // activeSegmentIndex should be set by findSegmentAndIndexForTime
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(1);
        });
    });

    describe('autoSaveStates', () => {
        it('stores save status per history item with timestamps', () => {
            useTranscriptStore.getState().setAutoSaveState('hist-1', 'saving');

            expect(useTranscriptStore.getState().autoSaveStates['hist-1']).toEqual({
                status: 'saving',
                updatedAt: expect.any(Number),
            });
        });

        it('clears the active history item auto-save state when requested', () => {
            useTranscriptStore.setState({ sourceHistoryId: 'hist-1' });
            useTranscriptStore.getState().setAutoSaveState('hist-1', 'saved');

            useTranscriptStore.getState().clearAutoSaveState();

            expect(useTranscriptStore.getState().autoSaveStates['hist-1']).toBeUndefined();
        });
    });

    describe('applyTranscriptUpdate', () => {
        it('atomically replaces a partial segment with multiple final segments', () => {
            useTranscriptStore.getState().setSegments([
                { id: 'seg-partial', text: 'Hello world.', start: 0, end: 2, isFinal: false },
            ]);

            useTranscriptStore.getState().applyTranscriptUpdate({
                removeIds: ['seg-partial'],
                upsertSegments: [
                    { id: 'seg-final-1', text: 'Hello.', start: 0, end: 1, isFinal: true },
                    { id: 'seg-final-2', text: 'World.', start: 1, end: 2, isFinal: true },
                ],
            }, 'seg-final-2');

            expect(useTranscriptStore.getState().segments.map((segment) => segment.id)).toEqual([
                'seg-final-1',
                'seg-final-2',
            ]);
            expect(useTranscriptStore.getState().activeSegmentId).toBe('seg-final-2');
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(1);
        });
    });
});
