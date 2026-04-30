import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useTrayHandling } from '../useTrayHandling';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

const invokeMock = vi.fn();
const listenMock = vi.fn();
const forceExitWithGuardMock = vi.fn();
const listeners = new Map<string, (...args: unknown[]) => unknown>();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock('../../services/quitGuard', () => ({
  forceExitWithGuard: (...args: unknown[]) => forceExitWithGuardMock(...args),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: {
      language: 'en',
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('useTrayHandling', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    useTranscriptStore.setState({
      isCaptionMode: false,
    });
    invokeMock.mockResolvedValue(undefined);
    forceExitWithGuardMock.mockResolvedValue(true);
    listenMock.mockImplementation(async (eventName: string, callback: (...args: unknown[]) => unknown) => {
      listeners.set(eventName, callback);
      return vi.fn();
    });
  });

  it('routes request-quit through the shared quit guard', async () => {
    renderHook(() => useTrayHandling(vi.fn(), vi.fn()));

    await waitFor(() => {
      expect(listeners.has('request-quit')).toBe(true);
    });

    await act(async () => {
      await listeners.get('request-quit')?.();
    });

    expect(forceExitWithGuardMock).toHaveBeenCalledTimes(1);
  });
});
