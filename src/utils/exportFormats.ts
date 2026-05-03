export type ExportMode = 'original' | 'translation' | 'bilingual';

export type ExportFormat = 'srt' | 'json' | 'txt' | 'vtt';

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
