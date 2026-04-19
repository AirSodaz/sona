import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { TranscriptSegment } from '../types/transcript';
import { exportSegments, getFileExtension, ExportFormat, ExportMode } from './exportFormats';

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
 * Writes the content to the specified file path directly.
 *
 * @param content The content to write.
 * @param filePath The destination file path.
 * @throws {Error} If writing the file fails.
 */
export async function exportToPath(content: string, filePath: string): Promise<void> {
    try {
        await writeTextFile(filePath, content);
    } catch (error) {
        console.error('Failed to write file to path:', filePath, error);
        throw error;
    }
}

/**
 * Opens a save dialog and exports the transcript segments to the selected file.
...

 * @param options The export options containing segments, format, mode, and optional filename.
 * @return A promise that resolves to true if the file was saved, false if cancelled.
 * @throws {Error} If writing the file fails.
 */
export async function saveTranscript(options: ExportOptions): Promise<boolean> {
    const { segments, format, mode = 'original', defaultFileName = 'transcript' } = options;

    try {
        const content = exportSegments(segments, format, mode);
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
