import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StorageUsageSnapshot, WebviewBrowsingDataClearResult } from '../../../types/storage';
import { SettingsStorageTab } from '../SettingsStorageTab';

const mocks = vi.hoisted(() => {
  let retentionDays: number | null | undefined = null;
  let sourceHistoryId: string | null = 'active-history';

  return {
    get retentionDays() {
      return retentionDays;
    },
    set retentionDays(value: number | null | undefined) {
      retentionDays = value;
    },
    get sourceHistoryId() {
      return sourceHistoryId;
    },
    set sourceHistoryId(value: string | null) {
      sourceHistoryId = value;
    },
    cleanupAudio: vi.fn(),
    clearWebviewBrowsingData: vi.fn(),
    confirm: vi.fn(),
    getUsageSnapshot: vi.fn(),
    previewAudioCleanup: vi.fn(),
    refreshHistory: vi.fn(),
    setConfig: vi.fn((patch: { historyAudioRetentionDays?: number | null }) => {
      retentionDays = patch.historyAudioRetentionDays;
    }),
    showError: vi.fn(),
  };
});

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'en' },
    t: (key: string, options?: Record<string, unknown>) => {
      const template = typeof options?.defaultValue === 'string' ? options.defaultValue : key;
      return template.replace(/{{\s*(\w+)\s*}}/g, (_, name) => String(options?.[name] ?? ''));
    },
  }),
}));

vi.mock('../../../services/historyService', () => ({
  historyService: {
    cleanupAudio: (...args: unknown[]) => mocks.cleanupAudio(...args),
    previewAudioCleanup: (...args: unknown[]) => mocks.previewAudioCleanup(...args),
  },
}));

vi.mock('../../../services/storageUsageService', () => ({
  storageUsageService: {
    clearWebviewBrowsingData: (...args: unknown[]) => mocks.clearWebviewBrowsingData(...args),
    getUsageSnapshot: (...args: unknown[]) => mocks.getUsageSnapshot(...args),
  },
}));

vi.mock('../../../stores/configStore', () => ({
  useHistoryStorageConfig: () => ({
    historyAudioRetentionDays: mocks.retentionDays,
  }),
  useSetConfig: () => mocks.setConfig,
}));

vi.mock('../../../stores/dialogStore', () => ({
  useDialogStore: (selector: any) => selector({
    confirm: (...args: unknown[]) => mocks.confirm(...args),
    showError: (...args: unknown[]) => mocks.showError(...args),
  }),
}));

vi.mock('../../../stores/historyStore', () => ({
  useHistoryStore: (selector: any) => selector({
    refresh: (...args: unknown[]) => mocks.refreshHistory(...args),
  }),
}));

vi.mock('../../../stores/transcriptSessionStore', () => ({
  useTranscriptSessionStore: (selector: any) => selector({
    sourceHistoryId: mocks.sourceHistoryId,
  }),
}));

function report(overrides: Partial<{
  eligibleCount: number;
  removedCount: number;
  removedBytes: number;
  missingMarkedCount: number;
  failedCount: number;
  skippedActiveCount: number;
}> = {}) {
  return {
    eligibleCount: 0,
    removedCount: 0,
    removedBytes: 0,
    missingMarkedCount: 0,
    failedCount: 0,
    skippedActiveCount: 0,
    ...overrides,
  };
}

function usageSnapshot(overrides: Partial<{
  totalBytes: number;
  webviewBytes: number | null;
}> = {}) {
  return {
    generatedAt: '2026-07-04T08:00:00.000Z',
    totalBytes: overrides.totalBytes ?? 10_240,
    categories: {
      audio: {
        bytes: 3_072,
        historyAudioBytes: 2_048,
        speakerSampleBytes: 1_024,
        fileCount: 2,
      },
      database: {
        bytes: 2_048,
        sqlite: {
          mainDbBytes: 1_024,
          mainWalBytes: 256,
          mainShmBytes: 128,
          analyticsDbBytes: 512,
          analyticsWalBytes: 64,
          analyticsShmBytes: 64,
          dataBytes: 1_280,
          indexBytes: 512,
          freePageBytes: 256,
          indexEntries: [
            { schema: 'main', name: 'idx_history_items_timestamp', bytes: 512 },
          ],
          dbstatAvailable: true,
        },
      },
      models: { bytes: 1_024, fileCount: 1 },
      temporary: { bytes: 512, fileCount: 1 },
      webviewCache: {
        bytes: overrides.webviewBytes ?? 1_024,
        clearSupported: true,
        path: 'C:/Users/test/AppData/Local/com.asoda.sona/EBWebView',
      },
      other: { bytes: 2_560, fileCount: 2 },
    },
  };
}

describe('SettingsStorageTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.retentionDays = null;
    mocks.sourceHistoryId = 'active-history';
    mocks.confirm.mockResolvedValue(true);
    mocks.clearWebviewBrowsingData.mockResolvedValue({
      beforeBytes: 4096,
      afterBytes: 1024,
      clearRequested: true,
    });
    mocks.cleanupAudio.mockResolvedValue(report());
    mocks.getUsageSnapshot.mockResolvedValue(usageSnapshot());
    mocks.previewAudioCleanup.mockResolvedValue(report());
    mocks.refreshHistory.mockResolvedValue(undefined);
    mocks.showError.mockResolvedValue(undefined);
  });

  it('accepts disabled storage capabilities from the core contract', () => {
    const snapshot: StorageUsageSnapshot = usageSnapshot();
    snapshot.categories.database.sqlite.dbstatAvailable = false;
    const clearResult: WebviewBrowsingDataClearResult = {
      beforeBytes: null,
      afterBytes: null,
      clearRequested: false,
    };

    expect(snapshot.categories.database.sqlite.dbstatAvailable).toBe(false);
    expect(clearResult.clearRequested).toBe(false);
  });

  it('renders the data usage overview from the storage snapshot', async () => {
    render(<SettingsStorageTab />);

    expect(screen.getByText('Data & Storage')).toBeTruthy();
    expect(await screen.findByText('Audio')).toBeTruthy();
    expect(screen.getByText('Database')).toBeTruthy();
    expect(screen.getByText('SQLite indexes')).toBeTruthy();
    expect(screen.getByText('WebView Cache')).toBeTruthy();
    expect(mocks.getUsageSnapshot).toHaveBeenCalledTimes(1);
  });

  it('shows a dbstat capability error when storage usage cannot be collected', async () => {
    mocks.getUsageSnapshot.mockRejectedValue(new Error('SQLite dbstat capability is unavailable: no such table: dbstat'));

    render(<SettingsStorageTab />);

    expect(await screen.findByText('Storage usage unavailable')).toBeTruthy();
    expect(screen.getByText(/SQLite dbstat capability is unavailable/)).toBeTruthy();
  });

  it('refreshes the storage usage snapshot on demand', async () => {
    render(<SettingsStorageTab />);

    const refreshButton = await screen.findByRole('button', { name: 'Refresh' });
    fireEvent.click(refreshButton);

    await waitFor(() => {
      expect(mocks.getUsageSnapshot).toHaveBeenCalledTimes(2);
    });
  });

  it('confirms and clears WebView browsing data, then refreshes usage', async () => {
    render(<SettingsStorageTab />);

    await screen.findByText('WebView Cache');
    fireEvent.click(screen.getByRole('button', { name: 'Clear WebView Data' }));

    await waitFor(() => {
      expect(mocks.confirm).toHaveBeenCalledWith(
        expect.stringContaining('This clears WebView cache'),
        expect.objectContaining({
          title: 'Clear WebView browsing data?',
        }),
      );
    });
    await waitFor(() => {
      expect(mocks.clearWebviewBrowsingData).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(mocks.getUsageSnapshot).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByTestId('settings-storage-webview-result').textContent).toContain('WebView cleanup requested');
  });

  it('shows a command error when WebView cleanup fails', async () => {
    mocks.clearWebviewBrowsingData.mockRejectedValue(new Error('clear failed'));

    render(<SettingsStorageTab />);

    await screen.findByText('WebView Cache');
    fireEvent.click(screen.getByRole('button', { name: 'Clear WebView Data' }));

    await waitFor(() => {
      expect(mocks.showError).toHaveBeenCalledWith(expect.objectContaining({
        code: 'storage.webview_cleanup_failed',
      }));
    });
  });

  it('saves the selected history audio retention preset', async () => {
    render(<SettingsStorageTab />);
    await screen.findByRole('button', { name: 'Refresh' });

    fireEvent.click(screen.getByLabelText('Audio retention'));
    fireEvent.click(await screen.findByRole('option', { name: '30 days' }));

    expect(mocks.setConfig).toHaveBeenCalledWith({
      historyAudioRetentionDays: 30,
    });
  });

  it('previews, confirms, applies, refreshes history, and shows the cleanup result', async () => {
    mocks.retentionDays = 30;
    mocks.previewAudioCleanup.mockResolvedValue(report({
      eligibleCount: 3,
      removedCount: 2,
      removedBytes: 2048,
      missingMarkedCount: 1,
      skippedActiveCount: 1,
    }));
    mocks.cleanupAudio.mockResolvedValue(report({
      eligibleCount: 3,
      removedCount: 2,
      removedBytes: 2048,
      missingMarkedCount: 1,
      skippedActiveCount: 1,
    }));

    render(<SettingsStorageTab />);
    await screen.findByRole('button', { name: 'Refresh' });

    fireEvent.click(screen.getByRole('button', { name: 'Clean Now' }));

    await waitFor(() => {
      expect(mocks.previewAudioCleanup).toHaveBeenCalledWith(30, 'active-history');
    });
    await waitFor(() => {
      expect(mocks.confirm).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(mocks.cleanupAudio).toHaveBeenCalledWith(30, 'active-history');
    });
    expect(mocks.refreshHistory).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('settings-storage-cleanup-result').textContent).toContain('2 files removed');
    expect(screen.getByTestId('settings-storage-cleanup-result').textContent).toContain('1 missing');
  });

  it('handles a zero-file preview without asking for confirmation', async () => {
    mocks.retentionDays = 7;
    mocks.previewAudioCleanup.mockResolvedValue(report());

    render(<SettingsStorageTab />);
    await screen.findByRole('button', { name: 'Refresh' });

    fireEvent.click(screen.getByRole('button', { name: 'Clean Now' }));

    await waitFor(() => {
      expect(mocks.previewAudioCleanup).toHaveBeenCalledWith(7, 'active-history');
    });
    await waitFor(() => {
      expect(screen.getByTestId('settings-storage-cleanup-result').textContent).toContain('No audio files need cleanup');
    });
    expect(mocks.confirm).not.toHaveBeenCalled();
    expect(mocks.cleanupAudio).not.toHaveBeenCalled();
  });

  it('shows a command error when cleanup fails', async () => {
    mocks.retentionDays = 90;
    mocks.previewAudioCleanup.mockResolvedValue(report({ eligibleCount: 1, removedCount: 1, removedBytes: 512 }));
    mocks.cleanupAudio.mockRejectedValue(new Error('delete failed'));

    render(<SettingsStorageTab />);
    await screen.findByRole('button', { name: 'Refresh' });

    fireEvent.click(screen.getByRole('button', { name: 'Clean Now' }));

    await waitFor(() => {
      expect(mocks.showError).toHaveBeenCalledWith(expect.objectContaining({
        code: 'history.audio_cleanup_failed',
      }));
    });
  });
});
