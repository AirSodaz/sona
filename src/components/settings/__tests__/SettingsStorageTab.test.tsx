import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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
    confirm: vi.fn(),
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

describe('SettingsStorageTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.retentionDays = null;
    mocks.sourceHistoryId = 'active-history';
    mocks.confirm.mockResolvedValue(true);
    mocks.cleanupAudio.mockResolvedValue(report());
    mocks.previewAudioCleanup.mockResolvedValue(report());
    mocks.refreshHistory.mockResolvedValue(undefined);
    mocks.showError.mockResolvedValue(undefined);
  });

  it('saves the selected history audio retention preset', async () => {
    render(<SettingsStorageTab />);

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

    fireEvent.click(screen.getByRole('button', { name: 'Clean Now' }));

    await waitFor(() => {
      expect(mocks.showError).toHaveBeenCalledWith(expect.objectContaining({
        code: 'history.audio_cleanup_failed',
      }));
    });
  });
});
