import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TranscriptSegment } from '../../types/transcript';
import { exportTranscriptToDirectory } from '../exportService';
import { exportTranscriptFile } from '../tauri/export';

vi.mock('@tauri-apps/api/path', () => ({
  join: vi.fn(async (...parts: string[]) => parts.join('/')),
}));

vi.mock('../tauri/export', () => ({
  exportTranscriptFile: vi.fn(
    async ({ outputPath }: { outputPath: string }) => ({
      outputPath,
      bytesWritten: 42,
    }),
  ),
}));

describe('exportService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('exports transcript files through the Rust command wrapper', async () => {
    const segments: TranscriptSegment[] = [
      {
        id: 'seg-1',
        start: 0,
        end: 1,
        text: 'Hello',
        isFinal: true,
        translation: 'Bonjour',
      },
    ];

    const path = await exportTranscriptToDirectory({
      segments,
      directory: 'C:/exports',
      baseFileName: 'bad:name',
      format: 'srt',
      mode: 'bilingual',
    });

    expect(path).toBe('C:/exports/bad_name.srt');
    expect(exportTranscriptFile).toHaveBeenCalledWith({
      segments,
      format: 'srt',
      mode: 'bilingual',
      outputPath: 'C:/exports/bad_name.srt',
    });
  });
});
