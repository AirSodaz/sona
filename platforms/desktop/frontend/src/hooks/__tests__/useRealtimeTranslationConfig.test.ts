import { act, renderHook } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type * as ModelServiceModule from '../../services/modelService';
import { modelService } from '../../services/modelService';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { useRealtimeTranslationConfig } from '../useRealtimeTranslationConfig';

vi.mock('../../services/modelService', async (importOriginal) => {
  const actual = await importOriginal<typeof ModelServiceModule>();
  return {
    ...actual,
    modelService: {
      ...actual.modelService,
      isModelInstalled: vi.fn(),
    },
  };
});

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    }),
  };
});

describe('useRealtimeTranslationConfig', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(true);
    useConfigStore.setState({
      config: buildTestConfig({
        realtimeTranslationEnabled: false,
        realtimeTranslationSourceLanguage: 'en',
        realtimeTranslationTargetLanguage: 'zh',
        captionDisplayMode: 'bilingual',
      }),
    });
  });

  it('computes modelId and valid target languages based on source language', () => {
    const { result, rerender } = renderHook(() => useRealtimeTranslationConfig());

    expect(result.current.modelId).toBe('opus-mt-en-zh');
    expect(result.current.targetLanguages).toEqual([{ value: 'zh', label: '中文 (Chinese)' }]);

    act(() => {
      result.current.handleSourceLanguageChange('zh');
    });

    rerender();
    expect(useConfigStore.getState().config.realtimeTranslationSourceLanguage).toBe('zh');
    expect(result.current.targetLanguages).toEqual([
      { value: 'en', label: 'English' },
      { value: 'ja', label: '日本語 (Japanese)' },
    ]);
  });

  it('swaps languages safely', () => {
    const { result, rerender } = renderHook(() => useRealtimeTranslationConfig());

    act(() => {
      result.current.handleSwapLanguages();
    });

    rerender();
    expect(useConfigStore.getState().config.realtimeTranslationSourceLanguage).toBe('zh');
    expect(useConfigStore.getState().config.realtimeTranslationTargetLanguage).toBe('en');

    act(() => {
      result.current.handleSwapLanguages();
    });

    rerender();
    expect(useConfigStore.getState().config.realtimeTranslationSourceLanguage).toBe('en');
    expect(useConfigStore.getState().config.realtimeTranslationTargetLanguage).toBe('zh');
  });

  it('enables realtime translation when model is installed', async () => {
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(true);
    const { result } = renderHook(() => useRealtimeTranslationConfig());

    await act(async () => {
      await result.current.handleToggleRealtimeTranslation(true);
    });

    expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(true);
  });

  it('blocks enabling and shows error dialog when model is not installed', async () => {
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);
    const showError = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ showError });

    const { result } = renderHook(() => useRealtimeTranslationConfig());

    await act(async () => {
      await result.current.handleToggleRealtimeTranslation(true);
    });

    expect(showError).toHaveBeenCalledWith(
      expect.objectContaining({
        code: 'model.not_installed',
      })
    );
    expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(false);
  });

  it('resets captionDisplayMode to bilingual when disabling translation in translation mode', async () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        realtimeTranslationEnabled: true,
        captionDisplayMode: 'translation',
      },
    }));

    const { result } = renderHook(() => useRealtimeTranslationConfig());

    await act(async () => {
      await result.current.handleToggleRealtimeTranslation(false);
    });

    expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(false);
    expect(useConfigStore.getState().config.captionDisplayMode).toBe('bilingual');
  });
});
