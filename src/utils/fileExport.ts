import { save } from '@tauri-apps/plugin-dialog';
import type { TranscriptSegment } from '../types/transcript';
import { getFileExtension } from './exportFormats';
import type { ExportFormat, ExportMode } from './exportFormats';
import { logger } from './logger';
import { exportTranscriptFile } from '../services/tauri/export';

interface ExportOptions {
    /** The transcript segments to export. */
    segments: TranscriptSegment[];
    /** The format to export to. */
    format: ExportFormat;
    /** The export mode (original, translation, bilingual). Defaults to 'original'. */
    mode?: ExportMode;
    /** The default file name (without extension). Defaults to 'transcript'. */
    defaultFileName?: string;
}

/**
 * Opens a save dialog and exports the transcript segments to the selected file.
 *
 * @param options The export options containing segments, format, mode, and optional filename.
 * @return A promise that resolves to true if the file was saved, false if cancelled.
 * @throws {Error} If writing the file fails.
 */
export async function saveTranscript(options: ExportOptions): Promise<boolean> {
    const { segments, format, mode = 'original', defaultFileName = 'transcript' } = options;

    try {
        const extension = getFileExtension(format);

        // Open save dialog
        const filePath = await save({
            defaultPath: `${defaultFileName}${extension}`,
            filters: [
                {
                    name: format.toUpperCase(),
                    extensions: [format],
                },
                {
                    name: 'All Files',
                    extensions: ['*'],
                },
            ],
        });

        if (!filePath) {
            // User cancelled the dialog
            return false;
        }

        await exportTranscriptFile({
            segments,
            format,
            mode,
            outputPath: filePath,
        });

        return true;
    } catch (error) {
        logger.error('Failed to export transcript:', error);
        throw error;
    }
}
