import type { TranscriptSegment } from '../types/transcript';

export interface HistoryTranscriptMetadata {
    previewText: string;
    searchContent: string;
}

export function buildHistoryTranscriptMetadata(segments: TranscriptSegment[]): HistoryTranscriptMetadata {
    const searchContent = segments.map((segment) => segment.text).join(' ');
    const previewText = searchContent.substring(0, 100) + (segments.length > 0 ? '...' : '');

    return {
        previewText,
        searchContent,
    };
}
