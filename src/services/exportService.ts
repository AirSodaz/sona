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

export interface ExportServicePorts {
  exportTranscriptFile: typeof exportTranscriptFile;
  join: typeof join;
}

export class ExportService {
  constructor(private readonly ports: ExportServicePorts) {}

  sanitizeExportFileName = (fileName: string): string => {
    return fileName.replace(/[\\/:*?"<>|]/g, '_').trim();
  }

  exportTranscriptToDirectory = async (
    options: ExportTranscriptToDirectoryOptions,
  ): Promise<string> => {
    const extension = getFileExtension(options.format);
    const fullPath = await this.ports.join(
      options.directory,
      `${this.sanitizeExportFileName(options.baseFileName)}${extension}`,
    );
    const result = await this.ports.exportTranscriptFile({
      segments: options.segments,
      format: options.format,
      mode: options.mode,
      outputPath: fullPath,
    });
    return result.outputPath;
  }
}

export function createExportService(ports: ExportServicePorts): ExportService {
  return new ExportService(ports);
}

export const exportService = createExportService({
  exportTranscriptFile,
  join,
});

export const {
  sanitizeExportFileName,
  exportTranscriptToDirectory,
} = exportService;
