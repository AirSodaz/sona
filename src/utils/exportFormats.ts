import { TranscriptSegment } from '../types/transcript';
import { stripHtmlTags } from './segmentUtils';

export type ExportMode = 'original' | 'translation' | 'bilingual';

function prefixSpeakerLabel(segment: TranscriptSegment, text: string): string {
    const trimmed = text.trim();
    if (!trimmed) return '';
    return segment.speaker?.label ? `${segment.speaker.label}: ${trimmed}` : trimmed;
}

function decodeHtmlEntities(text: string): string {
    return text
        .replace(/&nbsp;/g, ' ')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&quot;/g, '"')
        .replace(/&#39;/g, "'");
}

function htmlToPlainText(html: string): string {
    if (!html) return '';
    // Convert structural tags to newlines
    const text = html
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<\/div>/gi, '\n')
        .replace(/<div>/gi, '')
        .replace(/<\/p>/gi, '\n')
        .replace(/<p>/gi, '');

    return decodeHtmlEntities(stripHtmlTags(text));
}

function htmlToFormattedText(html: string): string {
    if (!html) return '';
    // Convert structural tags to newlines, but preserve formatting tags (b, i, u)
    const text = html
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<\/div>/gi, '\n')
        .replace(/<div>/gi, '')
        .replace(/<\/p>/gi, '\n')
        .replace(/<p>/gi, '');

    // Decode entities (converts &lt; to <, etc.)
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
 * Formats the text based on the desired export mode.
 */
function getSegmentExportText(segment: TranscriptSegment, mode: ExportMode, isHtmlFormatted: boolean, isSubtitle: boolean = false): string {
    const original = isHtmlFormatted ? htmlToFormattedText(segment.text.trim()) : htmlToPlainText(segment.text.trim());
    const translation = (segment.translation || '').trim();

    if (mode === 'translation') {
        return prefixSpeakerLabel(segment, translation);
    }

    if (mode === 'bilingual') {
        if (isSubtitle) {
            return prefixSpeakerLabel(segment, `${translation}\n${original}`);
        }
        return prefixSpeakerLabel(segment, `${original}\n${translation}`);
    }

    return prefixSpeakerLabel(segment, original);
}

/**
 * Converts TranscriptSegment array to SRT (SubRip Subtitle) format.
 *
 * @param segments The array of transcript segments to convert.
 * @param mode The export mode.
 * @return The SRT formatted string.
 */
export function toSRT(segments: TranscriptSegment[], mode: ExportMode = 'original'): string {
    return segments
        .filter((seg) => seg.isFinal)
        .map((segment) => ({
            ...segment,
            text: getSegmentExportText(segment, mode, true, true)
        }))
        .filter((seg) => seg.text.trim().length > 0)
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
            const original = htmlToFormattedText(segment.text.trim());
            const translation = (segment.translation || '').trim();

            if (mode === 'translation') {
                return {
                    start: segment.start,
                    end: segment.end,
                    text: translation,
                    speaker: segment.speaker,
                };
            }
            if (mode === 'bilingual') {
                return {
                    start: segment.start,
                    end: segment.end,
                    text: original,
                    translation: translation || undefined,
                    speaker: segment.speaker,
                };
            }
            return { start: segment.start, end: segment.end, text: original, speaker: segment.speaker };
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
    return segments
        .filter((seg) => seg.isFinal)
        .map((segment) => getSegmentExportText(segment, mode, false))
        .filter((text) => text.trim().length > 0)
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
    const content = segments
        .filter((seg) => seg.isFinal)
        .map((segment) => ({
            ...segment,
            text: getSegmentExportText(segment, mode, true, true)
        }))
        .filter((seg) => seg.text.trim().length > 0)
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
