import { TranscriptSegment } from '../types/transcript';
import { stripHtmlTags } from './segmentUtils';

export type ExportMode = 'original' | 'translation' | 'bilingual';

function decodeHtmlEntities(text: string): string {
    return text
        .replace(/&nbsp;/g, ' ')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&quot;/g, '"')
        .replace(/&#39;/g, "'");
}

/**
 * Processes HTML content for export, optionally preserving formatting tags.
 *
 * @param html The HTML string to process.
 * @param stripTags Whether to strip all HTML tags (true) or preserve simple formatting (false).
 * @returns The processed plain text or formatted text.
 */
function processText(html: string, stripTags: boolean): string {
    if (!html) return '';

    // Convert structural tags to newlines
    let text = html
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<\/div>/gi, '\n')
        .replace(/<div>/gi, '')
        .replace(/<\/p>/gi, '\n')
        .replace(/<p>/gi, '');

    if (stripTags) {
        text = stripHtmlTags(text);
    }

    // Decode entities
    return decodeHtmlEntities(text);
}

/**
 * Formats seconds to timestamp format (HH:MM:SS<separator>mmm).
 *
 * @param seconds The time in seconds.
 * @param separator The decimal separator (defaults to comma for SRT).
 * @return The formatted timestamp string.
 */
function formatTimestamp(seconds: number, separator: string = ','): string {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    const millis = Math.floor((seconds % 1) * 1000);

    return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}${separator}${String(millis).padStart(3, '0')}`;
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
 * Helper to prepare segments for text-based exports.
 *
 * @param segments The source segments.
 * @param mode The export mode.
 * @param stripTags Whether to strip all tags (for TXT) or preserve formatting (for SRT/VTT).
 * @param reverseBilingual If true, puts original first (TXT style). If false, puts translation first (Subtitle style).
 */
function prepareExportData(
    segments: TranscriptSegment[],
    mode: ExportMode,
    stripTags: boolean,
    reverseBilingual: boolean = false
): { text: string; start: number; end: number }[] {
    return segments
        .filter((seg) => seg.isFinal)
        .map((segment) => {
            const original = processText(segment.text.trim(), stripTags);
            const translation = (segment.translation || '').trim();
            let text = original;

            if (mode === 'translation') {
                text = translation;
            } else if (mode === 'bilingual') {
                text = reverseBilingual
                    ? `${original}\n${translation}`
                    : `${translation}\n${original}`;
            }

            return { start: segment.start, end: segment.end, text };
        })
        .filter((seg) => seg.text.trim().length > 0);
}

/**
 * Converts TranscriptSegment array to SRT (SubRip Subtitle) format.
 *
 * @param segments The array of transcript segments to convert.
 * @param mode The export mode.
 * @return The SRT formatted string.
 */
export function toSRT(segments: TranscriptSegment[], mode: ExportMode = 'original'): string {
    return prepareExportData(segments, mode, false, false)
        .map((segment, index) => {
            const startTime = formatTimestamp(segment.start);
            const endTime = formatTimestamp(segment.end);
            return `${index + 1}\n${startTime} --> ${endTime}\n${segment.text}\n`;
        })
        .join('\n');
}

/**
 * Converts TranscriptSegment array to JSON format.
 *
 * @param segments The array of transcript segments to convert.
 * @param mode The export mode.
 * @return The JSON formatted string.
 */
export function toJSON(segments: TranscriptSegment[], mode: ExportMode = 'original'): string {
    const exportData = segments
        .filter((seg) => seg.isFinal)
        .map((segment) => {
            const original = processText(segment.text.trim(), false);
            const translation = (segment.translation || '').trim();

            if (mode === 'translation') {
                return {
                    start: segment.start,
                    end: segment.end,
                    text: translation,
                };
            } else if (mode === 'bilingual') {
                return {
                    start: segment.start,
                    end: segment.end,
                    text: original,
                    translation: translation || undefined,
                };
            }

            return {
                start: segment.start,
                end: segment.end,
                text: original,
            };
        });

    return JSON.stringify(exportData, null, 2);
}

/**
 * Converts TranscriptSegment array to plain text format.
 *
 * @param segments The array of transcript segments to convert.
 * @param mode The export mode.
 * @return The plain text string.
 */
export function toTXT(segments: TranscriptSegment[], mode: ExportMode = 'original'): string {
    return prepareExportData(segments, mode, true, true)
        .map((segment) => segment.text)
        .join('\n\n');
}

/**
 * Converts TranscriptSegment array to VTT (WebVTT) format.
 *
 * @param segments The array of transcript segments to convert.
 * @param mode The export mode.
 * @return The VTT formatted string.
 */
export function toVTT(segments: TranscriptSegment[], mode: ExportMode = 'original'): string {
    const header = 'WEBVTT\n\n';
    const content = prepareExportData(segments, mode, false, false)
        .map((segment) => {
            const startTime = formatTimestamp(segment.start, '.');
            const endTime = formatTimestamp(segment.end, '.');
            return `${startTime} --> ${endTime}\n${segment.text}\n`;
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
 * @param mode The export mode (original, translation, bilingual).
 * @return The formatted string content.
 * @throws {Error} If the format is unknown.
 */
export function exportSegments(segments: TranscriptSegment[], format: ExportFormat, mode: ExportMode = 'original'): string {
    switch (format) {
        case 'srt':
            return toSRT(segments, mode);
        case 'json':
            return toJSON(segments, mode);
        case 'txt':
            return toTXT(segments, mode);
        case 'vtt':
            return toVTT(segments, mode);
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
