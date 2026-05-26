import type { TranscriptSegment } from '../types/transcript';
import type { ExportFormat, ExportMode } from '../utils/exportFormats';
import { getFileExtension } from '../utils/exportFormats';
import { exportTranscriptFile } from './tauri/export';
import { join } from './tauri/platform/path';

export interface ExportTranscriptToDirectoryOptions {
  segments: TranscriptSegment[];
  directory: string;
  baseFileName: string;
  format: ExportFormat;
  mode: ExportMode;
}

export function sanitizeExportFileName(fileName: string): string {
  return fileName.replace(/[\\/:*?"<>|]/g, '_').trim();
}

export async function exportTranscriptToDirectory(
  options: ExportTranscriptToDirectoryOptions,
): Promise<string> {
  const extension = getFileExtension(options.format);
  const fullPath = await join(options.directory, `${sanitizeExportFileName(options.baseFileName)}${extension}`);
  const result = await exportTranscriptFile({
    segments: options.segments,
    format: options.format,
    mode: options.mode,
    outputPath: fullPath,
  });
  return result.outputPath;
}
