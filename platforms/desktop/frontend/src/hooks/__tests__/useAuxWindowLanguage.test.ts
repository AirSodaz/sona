import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { settingsStore } from '../../services/storageService';
import { useAuxWindowLanguage } from '../useAuxWindowLanguage';

const mockChangeLanguage = vi.fn();
let currentLanguage = 'en';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: {
      get language() {
        return currentLanguage;
      },
      changeLanguage: mockChangeLanguage.mockImplementation((lang: string) => {
        currentLanguage = lang;
        return Promise.resolve();
      }),
    },
  }),
}));

vi.mock('../../services/storageService', () => ({
  STORE_KEY_CONFIG: 'sona-config',
  settingsStore: {
    get: vi.fn(),
    onKeyChange: vi.fn(),
  },
}));

describe('useAuxWindowLanguage', () => {
  let onKeyChangeCallback: ((config: any) => void) | null = null;
  const mockUnlisten = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    currentLanguage = 'en';
    onKeyChangeCallback = null;
    vi.mocked(settingsStore.onKeyChange).mockImplementation((_key, callback) => {
      onKeyChangeCallback = callback;
      return Promise.resolve(mockUnlisten);
    });
  });

  it('synchronizes language on mount from stored config', async () => {
    vi.mocked(settingsStore.get).mockResolvedValue({
      appLanguage: 'zh-CN',
    } as any);

    renderHook(() => useAuxWindowLanguage());

    await waitFor(() => {
      expect(mockChangeLanguage).toHaveBeenCalledWith('zh');
    });
  });

  it('listens for config updates and updates i18n language', async () => {
    vi.mocked(settingsStore.get).mockResolvedValue({
      appLanguage: 'en',
    } as any);

    renderHook(() => useAuxWindowLanguage());

    await waitFor(() => {
      expect(settingsStore.onKeyChange).toHaveBeenCalled();
    });

    onKeyChangeCallback?.({
      appLanguage: 'ja',
    });

    await waitFor(() => {
      expect(mockChangeLanguage).toHaveBeenCalledWith('ja');
    });
  });

  it('unsubscribes from onKeyChange on unmount', async () => {
    vi.mocked(settingsStore.get).mockResolvedValue(null);

    const { unmount } = renderHook(() => useAuxWindowLanguage());

    await waitFor(() => {
      expect(settingsStore.onKeyChange).toHaveBeenCalled();
    });

    unmount();
    expect(mockUnlisten).toHaveBeenCalled();
  });

  it('cleans up immediately if unmounted while onKeyChange is in-flight', async () => {
    const getDeferred = Promise.withResolvers<any>();
    const onKeyChangeDeferred = Promise.withResolvers<any>();

    vi.mocked(settingsStore.get).mockImplementation(() => getDeferred.promise);
    vi.mocked(settingsStore.onKeyChange).mockImplementation(() => onKeyChangeDeferred.promise);

    const { unmount } = renderHook(() => useAuxWindowLanguage());

    // Resolve get, triggering onKeyChange call
    getDeferred.resolve({ appLanguage: 'zh-CN' });
    await waitFor(() => {
      expect(settingsStore.onKeyChange).toHaveBeenCalled();
    });

    // Unmount before onKeyChange completes
    unmount();

    // When onKeyChange resolves after unmount, it should immediately unlisten
    onKeyChangeDeferred.resolve(mockUnlisten);
    await waitFor(() => {
      expect(mockUnlisten).toHaveBeenCalled();
    });
  });

  it('handles errors when settingsStore fails gracefully', async () => {
    vi.mocked(settingsStore.get).mockRejectedValue(new Error('Storage failure'));

    renderHook(() => useAuxWindowLanguage());

    // Should not throw or crash
    await waitFor(() => {
      expect(settingsStore.get).toHaveBeenCalled();
    });
  });
});
