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
    getDirectories: vi.fn(),
    selectDirectory: vi.fn(),
    migrateDataDirectory: vi.fn(),
    resetDataDirectory: vi.fn(),
    setModelsDirectory: vi.fn(),
    resetModelsDirectory: vi.fn(),
    openPath: vi.fn(),
    relaunchApp: vi.fn(),
    getModelCatalogSnapshot: vi.fn(),
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

vi.mock('../../../services/storageLocationService', () => ({
  storageLocationService: {
    getDirectories: (...args: unknown[]) => mocks.getDirectories(...args),
    selectDirectory: (...args: unknown[]) => mocks.selectDirectory(...args),
    migrateDataDirectory: (...args: unknown[]) => mocks.migrateDataDirectory(...args),
    resetDataDirectory: (...args: unknown[]) => mocks.resetDataDirectory(...args),
    setModelsDirectory: (...args: unknown[]) => mocks.setModelsDirectory(...args),
    resetModelsDirectory: (...args: unknown[]) => mocks.resetModelsDirectory(...args),
    openPath: (...args: unknown[]) => mocks.openPath(...args),
    relaunchApp: (...args: unknown[]) => mocks.relaunchApp(...args),
  },
}));

vi.mock('../../../services/modelService', () => ({
  modelService: {
    getModelCatalogSnapshot: (...args: unknown[]) => mocks.getModelCatalogSnapshot(...args),
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
    mocks.getDirectories.mockResolvedValue({
      dataDir: '/default/data',
      defaultDataDir: '/default/data',
      isCustomDataDir: false,
      modelsDir: '/default/data/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: false,
    });
    mocks.selectDirectory.mockResolvedValue('/new/path');
    mocks.migrateDataDirectory.mockResolvedValue({
      dataDir: '/new/path',
      defaultDataDir: '/default/data',
      isCustomDataDir: true,
      modelsDir: '/default/data/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: false,
    });
    mocks.resetDataDirectory.mockResolvedValue({
      dataDir: '/default/data',
      defaultDataDir: '/default/data',
      isCustomDataDir: false,
      modelsDir: '/default/data/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: false,
    });
    mocks.setModelsDirectory.mockResolvedValue({
      dataDir: '/default/data',
      defaultDataDir: '/default/data',
      isCustomDataDir: false,
      modelsDir: '/new/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: true,
    });
    mocks.resetModelsDirectory.mockResolvedValue({
      dataDir: '/default/data',
      defaultDataDir: '/default/data',
      isCustomDataDir: false,
      modelsDir: '/default/data/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: false,
    });
    mocks.openPath.mockResolvedValue(undefined);
    mocks.relaunchApp.mockResolvedValue(undefined);
    mocks.getModelCatalogSnapshot.mockResolvedValue({});
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

    screen.getByText('Data & Storage');
    expect(await screen.findByText('Audio')).toBeTruthy();
    screen.getByText('Database');
    screen.getByText('SQLite indexes');
    screen.getByText('WebView Cache');
    expect(mocks.getUsageSnapshot).toHaveBeenCalledTimes(1);
  });

  it('shows a dbstat capability error when storage usage cannot be collected', async () => {
    mocks.getUsageSnapshot.mockRejectedValue(new Error('SQLite dbstat capability is unavailable: no such table: dbstat'));

    render(<SettingsStorageTab />);

    expect(await screen.findByText('Storage usage unavailable')).toBeTruthy();
    screen.getByText(/SQLite dbstat capability is unavailable/);
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

  it('renders storage locations with data directory and models directory', async () => {
    render(<SettingsStorageTab />);

    expect(await screen.findByText('/default/data')).toBeTruthy();
    const dataCard = screen.getByTestId('settings-storage-data-dir-card');
    const modelsCard = screen.getByTestId('settings-storage-models-dir-card');

    expect(dataCard).toBeTruthy();
    expect(modelsCard).toBeTruthy();
    expect(screen.getByText('/default/data')).toBeTruthy();
    expect(screen.getByText('/default/data/models')).toBeTruthy();

    const dataPathBox = dataCard.querySelector('.settings-storage-path-box');
    expect(dataPathBox?.getAttribute('title')).toBeNull();
    expect(dataPathBox?.getAttribute('data-tooltip')).toBe('/default/data');
    expect(dataPathBox?.getAttribute('data-tooltip-pos')).toBe('top');
    expect(dataPathBox?.hasAttribute('data-tooltip-multiline')).toBe(true);

    const modelsPathBox = modelsCard.querySelector('.settings-storage-path-box');
    expect(modelsPathBox?.getAttribute('title')).toBeNull();
    expect(modelsPathBox?.getAttribute('data-tooltip')).toBe('/default/data/models');
    expect(modelsPathBox?.getAttribute('data-tooltip-pos')).toBe('top');
    expect(modelsPathBox?.hasAttribute('data-tooltip-multiline')).toBe(true);

    const dataOpenBtn = dataCard.querySelectorAll('button')[1];
    expect(dataOpenBtn?.getAttribute('title')).toBeNull();
    expect(dataOpenBtn?.getAttribute('data-tooltip')).toBeNull();

    const modelsOpenBtn = modelsCard.querySelectorAll('button')[1];
    expect(modelsOpenBtn?.getAttribute('title')).toBeNull();
    expect(modelsOpenBtn?.getAttribute('data-tooltip')).toBeNull();
  });

  it('allows changing data directory and confirms before relaunch', async () => {
    render(<SettingsStorageTab />);
    await screen.findByText('/default/data');

    const dataCard = screen.getByTestId('settings-storage-data-dir-card');
    const changeBtn = dataCard.querySelector('button') as HTMLButtonElement;
    fireEvent.click(changeBtn);

    await waitFor(() => {
      expect(mocks.selectDirectory).toHaveBeenCalledWith('/default/data');
    });

    await waitFor(() => {
      expect(mocks.confirm).toHaveBeenCalledWith(
        expect.stringContaining('/new/path'),
        expect.objectContaining({
          title: 'Change Data Directory?',
        }),
      );
    });

    await waitFor(() => {
      expect(mocks.migrateDataDirectory).toHaveBeenCalledWith('/new/path', true);
    });

    await waitFor(() => {
      expect(mocks.relaunchApp).toHaveBeenCalledTimes(1);
    });
  });

  it('allows changing models directory and refreshes snapshot without relaunch', async () => {
    render(<SettingsStorageTab />);
    await screen.findByText('/default/data/models');

    const modelsCard = screen.getByTestId('settings-storage-models-dir-card');
    const changeBtn = modelsCard.querySelector('button') as HTMLButtonElement;
    fireEvent.click(changeBtn);

    await waitFor(() => {
      expect(mocks.selectDirectory).toHaveBeenCalledWith('/default/data/models');
    });

    await waitFor(() => {
      expect(mocks.confirm).toHaveBeenCalledWith(
        expect.stringContaining('/new/path'),
        expect.objectContaining({
          title: 'Change Models Directory?',
        }),
      );
    });

    await waitFor(() => {
      expect(mocks.setModelsDirectory).toHaveBeenCalledWith('/new/path', true);
    });

    await waitFor(() => {
      expect(mocks.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
    });
    expect(mocks.relaunchApp).not.toHaveBeenCalled();
  });

  it('opens storage directory in system file explorer', async () => {
    render(<SettingsStorageTab />);
    await screen.findByText('/default/data');

    const dataCard = screen.getByTestId('settings-storage-data-dir-card');
    const openBtn = dataCard.querySelectorAll('button')[1] as HTMLButtonElement;
    fireEvent.click(openBtn);

    await waitFor(() => {
      expect(mocks.openPath).toHaveBeenCalledWith('/default/data');
    });
  });

  it('resets custom data directory to default and relaunches', async () => {
    mocks.getDirectories.mockResolvedValue({
      dataDir: '/custom/data',
      defaultDataDir: '/default/data',
      isCustomDataDir: true,
      modelsDir: '/default/data/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: false,
    });

    render(<SettingsStorageTab />);
    await screen.findByText('/custom/data');

    const dataCard = screen.getByTestId('settings-storage-data-dir-card');
    const resetBtn = dataCard.querySelectorAll('button')[2] as HTMLButtonElement;
    fireEvent.click(resetBtn);

    await waitFor(() => {
      expect(mocks.confirm).toHaveBeenCalledWith(
        expect.stringContaining('/default/data'),
        expect.objectContaining({
          title: 'Restore Default Data Directory?',
        }),
      );
    });

    await waitFor(() => {
      expect(mocks.resetDataDirectory).toHaveBeenCalledTimes(1);
    });

    await waitFor(() => {
      expect(mocks.relaunchApp).toHaveBeenCalledTimes(1);
    });
  });

  it('resets custom models directory to default', async () => {
    mocks.getDirectories.mockResolvedValue({
      dataDir: '/default/data',
      defaultDataDir: '/default/data',
      isCustomDataDir: false,
      modelsDir: '/custom/models',
      defaultModelsDir: '/default/data/models',
      isCustomModelsDir: true,
    });

    render(<SettingsStorageTab />);
    await screen.findByText('/custom/models');

    const modelsCard = screen.getByTestId('settings-storage-models-dir-card');
    const resetBtn = modelsCard.querySelectorAll('button')[2] as HTMLButtonElement;
    fireEvent.click(resetBtn);

    await waitFor(() => {
      expect(mocks.confirm).toHaveBeenCalledWith(
        expect.stringContaining('/default/data/models'),
        expect.objectContaining({
          title: 'Restore Default Models Directory?',
        }),
      );
    });

    await waitFor(() => {
      expect(mocks.resetModelsDirectory).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(mocks.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
    });
  });
});
