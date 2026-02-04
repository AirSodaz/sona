import { TranscriptSegment } from '../types/transcript';

/**
 * Formats seconds to SRT timestamp format (HH:MM:SS,mmm).
 *
 * @param seconds The time in seconds.
 * @return The formatted timestamp string.
 */
function formatSRTTimestamp(seconds: number): string {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    const millis = Math.floor((seconds % 1) * 1000);

    return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')},${String(millis).padStart(3, '0')}`;
}

/**
 * Formats seconds to display format (MM:SS.m).
 *
 * @param seconds The time in seconds.
 * @return The formatted display time string.
 */
export function formatDisplayTime(seconds: number): string {
    const minutes = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    const tenths = Math.floor((seconds % 1) * 10);

    return `${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${tenths}`;
}

/**
 * Converts TranscriptSegment array to SRT (SubRip Subtitle) format.
 *
 * @param segments The array of transcript segments to convert.
 * @return The SRT formatted string.
 */
export function toSRT(segments: TranscriptSegment[]): string {
    return segments
        .filter((seg) => seg.isFinal && seg.text.trim().length > 0)
        .map((segment, index) => {
            const startTime = formatSRTTimestamp(segment.start);
            const endTime = formatSRTTimestamp(segment.end);
            return `${index + 1}\n${startTime} --> ${endTime}\n${segment.text.trim()}\n`;
        })
        .join('\n');
}

/**
 * Converts TranscriptSegment array to JSON format.
 *
 * @param segments The array of transcript segments to convert.
 * @return The JSON formatted string.
 */
export function toJSON(segments: TranscriptSegment[]): string {
    const exportData = segments
        .filter((seg) => seg.isFinal)
        .map((segment) => ({
            start: segment.start,
            end: segment.end,
            text: segment.text.trim(),
        }));

    return JSON.stringify(exportData, null, 2);
}

/**
 * Converts TranscriptSegment array to plain text format.
 *
 * @param segments The array of transcript segments to convert.
 * @return The plain text string.
 */
export function toTXT(segments: TranscriptSegment[]): string {
    return segments
        .filter((seg) => seg.isFinal && seg.text.trim().length > 0)
        .map((segment) => segment.text.trim())
        .join('\n\n');
}

/**
 * Converts TranscriptSegment array to VTT (WebVTT) format.
 *
 * @param segments The array of transcript segments to convert.
 * @return The VTT formatted string.
 */
export function toVTT(segments: TranscriptSegment[]): string {
    const header = 'WEBVTT\n\n';
    const content = segments
        .filter((seg) => seg.isFinal && seg.text.trim().length > 0)
        .map((segment) => {
            // VTT uses different timestamp format (HH:MM:SS.mmm)
            const startTime = formatSRTTimestamp(segment.start).replace(',', '.');
            const endTime = formatSRTTimestamp(segment.end).replace(',', '.');
            return `${startTime} --> ${endTime}\n${segment.text.trim()}\n`;
        })
        .join('\n');

    return header + content;
}

export type ExportFormat = 'srt' | 'json' | 'txt' | 'vtt';

/**
 * Exports segments in the specified format.
 *
 * @param segments The transcript segments to export.
 * @param format The target export format.
 * @return The formatted string content.
 * @throws {Error} If the format is unknown.
 */
export function exportSegments(segments: TranscriptSegment[], format: ExportFormat): string {
    switch (format) {
        case 'srt':
            return toSRT(segments);
        case 'json':
            return toJSON(segments);
        case 'txt':
            return toTXT(segments);
        case 'vtt':
            return toVTT(segments);
        default:
            throw new Error(`Unknown export format: ${format}`);
    }
}

/**
 * Gets the file extension for a given format.
 *
 * @param format The export format.
 * @return The file extension (e.g., ".srt").
 */
export function getFileExtension(format: ExportFormat): string {
    return `.${format}`;
}

/**
 * Gets the MIME type for a given format.
 *
 * @param format The export format.
 * @return The MIME type string.
 */
export function getMimeType(format: ExportFormat): string {
    switch (format) {
        case 'srt':
        case 'vtt':
        case 'txt':
            return 'text/plain';
        case 'json':
            return 'application/json';
        default:
            return 'text/plain';
    }
}
