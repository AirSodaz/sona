import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { TranscriptSegment } from '../types/transcript';
import { exportSegments, getFileExtension, ExportFormat } from './exportFormats';

interface ExportOptions {
    /** The transcript segments to export. */
    segments: TranscriptSegment[];
    /** The format to export to. */
    format: ExportFormat;
    /** The default file name (without extension). Defaults to 'transcript'. */
    defaultFileName?: string;
}

/**
 * Opens a save dialog and exports the transcript segments to the selected file.
 *
 * @param options The export options containing segments, format, and optional filename.
 * @return A promise that resolves to true if the file was saved, false if cancelled.
 * @throws {Error} If writing the file fails.
 */
export async function saveTranscript(options: ExportOptions): Promise<boolean> {
    const { segments, format, defaultFileName = 'transcript' } = options;

    try {
        const content = exportSegments(segments, format);
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

        // Write the file
        await writeTextFile(filePath, content);

        return true;
    } catch (error) {
        console.error('Failed to export transcript:', error);
        throw error;
    }
}
