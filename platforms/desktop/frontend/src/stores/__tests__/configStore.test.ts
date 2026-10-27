// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import {
  CURRENT_CONFIG_VERSION,
  DEFAULT_CONFIG,
  useApiServerConfig,
  useCaptionConfig,
  useConfigStore,
} from '../configStore';

describe('configStore - useApiServerConfig', () => {
  it('includes httpServerEnabled in useApiServerConfig selector', () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        httpServerEnabled: true,
        httpServerHost: '0.0.0.0',
        httpServerPort: 14200,
      },
    }));

    const { result, rerender } = renderHook(() => useApiServerConfig());
    expect(result.current.httpServerEnabled).toBe(true);
    expect(result.current.httpServerHost).toBe('0.0.0.0');
    expect(result.current.httpServerPort).toBe(14200);

    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        httpServerEnabled: false,
      },
    }));

    rerender();
    expect(result.current.httpServerEnabled).toBe(false);
  });

  it('exports CURRENT_CONFIG_VERSION aligned with DEFAULT_CONFIG.configVersion', () => {
    expect(CURRENT_CONFIG_VERSION).toBe(8);
    expect(DEFAULT_CONFIG.configVersion).toBe(8);
  });

  it('includes captionTranslationFontSize and captionTranslationFontColor in DEFAULT_CONFIG', () => {
    expect(DEFAULT_CONFIG.captionTranslationFontSize).toBe(16);
    expect(DEFAULT_CONFIG.captionTranslationFontColor).toBe('#6ee7b7');
  });

  it('includes new caption defaults and returns them from useCaptionConfig', () => {
    expect(DEFAULT_CONFIG.captionStrokeWidth).toBe(2);
    expect(DEFAULT_CONFIG.captionDisplayPriority).toBe('translation_main');
    expect(DEFAULT_CONFIG.captionFontSize).toBe(22);
    expect(DEFAULT_CONFIG.captionBackgroundColor).toBe('#17171a');
    expect(DEFAULT_CONFIG.captionBackgroundOpacity).toBe(0.72);

    const { result } = renderHook(() => useCaptionConfig());
    expect(result.current.captionStrokeWidth).toBe(2);
    expect(result.current.captionDisplayPriority).toBe('translation_main');
    expect(result.current.captionTextStroke).toBe(false);
    expect(result.current.captionStrokeColor).toBe('#000000');
    expect(result.current.captionTextShadow).toBe(true);
    expect(result.current.captionTextAlign).toBe('center');
    expect(result.current.captionBackgroundBlur).toBe(true);
    expect(result.current.captionBackgroundBlurRadius).toBe(20);
    expect(result.current.captionDisappearDelay).toBe(4.0);
    expect(result.current.captionLockShortcut).toBe('Alt+C');
  });
});
