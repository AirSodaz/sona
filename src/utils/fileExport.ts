import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { TranscriptSegment } from '../types/transcript';
import { exportSegments, getFileExtension, ExportFormat } from './exportFormats';

interface ExportOptions {
    segments: TranscriptSegment[];
    format: ExportFormat;
    defaultFileName?: string;
}

/**
 * Opens a save dialog and exports the transcript segments to the selected file
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
