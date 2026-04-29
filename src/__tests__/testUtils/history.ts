import { vi } from 'vitest';
import type { HistoryItem } from '../../types/history';

export function buildImportedHistoryItem(
  filePath = '/path/to/test.wav',
  overrides: Partial<HistoryItem> = {},
): HistoryItem {
  const id = overrides.id ?? 'mock-history-id';
  const filename = filePath.split(/[/\\]/).pop() || 'test.wav';

  return {
    id,
    timestamp: 1,
    duration: 1,
    audioPath: `${id}.wav`,
    transcriptPath: `${id}.json`,
    title: `Batch ${filename}`,
    previewText: 'Test...',
    searchContent: 'Test',
    type: 'batch',
    projectId: null,
    status: 'complete',
    ...overrides,
  };
}

export function createHistoryServiceMockModule(options?: {
  filePath?: string;
  importedItem?: HistoryItem | null;
}) {
  const importedItem = options?.importedItem === undefined
    ? buildImportedHistoryItem(options?.filePath)
    : options.importedItem;

  return {
    historyService: {
      saveImportedFile: vi.fn().mockResolvedValue(importedItem),
      updateTranscript: vi.fn().mockResolvedValue(undefined),
    },
  };
}
