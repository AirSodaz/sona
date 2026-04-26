import { join } from '@tauri-apps/api/path';
import type { TranscriptSegment } from '../types/transcript';
import type { ExportFormat, ExportMode } from '../utils/exportFormats';
import { exportSegments, getFileExtension } from '../utils/exportFormats';
import { exportToPath } from '../utils/fileExport';

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
  const content = exportSegments(options.segments, options.format, options.mode);
  const extension = getFileExtension(options.format);
  const fullPath = await join(options.directory, `${sanitizeExportFileName(options.baseFileName)}${extension}`);
  await exportToPath(content, fullPath);
  return fullPath;
}
