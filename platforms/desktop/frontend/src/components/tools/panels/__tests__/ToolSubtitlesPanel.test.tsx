import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { captionWindowService } from '../../../../services/captionWindowService';
import type * as ModelServiceModule from '../../../../services/modelService';
import { modelService } from '../../../../services/modelService';
import { useConfigStore } from '../../../../stores/configStore';
import { useDialogStore } from '../../../../stores/dialogStore';
import { useTranscriptRuntimeStore } from '../../../../stores/transcriptRuntimeStore';
import { buildTestConfig } from '../../../../test-utils/configTestUtils';
import { ToolSubtitlesPanel } from '../ToolSubtitlesPanel';

vi.mock('../../../../services/captionWindowService', () => ({
  captionWindowService: {
    resetPosition: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock('../../../../services/modelService', async (importOriginal) => {
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
      t: (key: string) => key,
      i18n: { language: 'en' },
    }),
  };
});

describe('ToolSubtitlesPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(true);
    useTranscriptRuntimeStore.setState({ isCaptionMode: false });
    useConfigStore.setState({
      config: buildTestConfig({
        captionWindowWidth: 800,
        captionFontSize: 24,
        captionFontColor: '#ffffff',
        captionTranslationFontSize: 18,
        captionTranslationFontColor: '#6ee7b7',
        captionBackgroundColor: '#000000',
        captionBackgroundOpacity: 0.6,
        realtimeTranslationEnabled: false,
        realtimeTranslationSourceLanguage: 'en',
        realtimeTranslationTargetLanguage: 'zh',
        captionDisplayMode: 'bilingual',
        captionBilingualOrder: 'original_first',
        captionDisplayPriority: 'original_main',
      }),
    });
  });

  afterEach(() => {
    cleanup();
  });

  it('renders all four settings sections', () => {
    const { container } = render(<ToolSubtitlesPanel />);

    expect(container.querySelector('#tools-subtitle-window')).not.toBeNull();
    expect(container.querySelector('#tools-subtitle-translation')).not.toBeNull();
    expect(container.querySelector('#tools-subtitle-appearance')).not.toBeNull();
    expect(container.querySelector('#tools-subtitle-preview-section')).not.toBeNull();
  });

  it('toggles real-time translation switch and updates config when model is installed', async () => {
    render(<ToolSubtitlesPanel />);

    const switchBtn = screen.getByRole('switch', {
      name: 'settings.enable_realtime_translation',
    });
    expect(switchBtn.getAttribute('aria-checked')).toBe('false');

    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(true);
    });
  });

  it('blocks enabling realtime translation and displays error when model is not installed', async () => {
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);
    const showErrorSpy = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ showError: showErrorSpy });

    render(<ToolSubtitlesPanel />);

    const switchBtn = screen.getByRole('switch', {
      name: 'settings.enable_realtime_translation',
    });
    fireEvent.click(switchBtn);

    await waitFor(() => {
      expect(showErrorSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          code: 'model.not_installed',
        })
      );
    });

    expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(false);
  });
  it('falls back to bilingual mode when realtime translation is disabled while in translation mode', () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        realtimeTranslationEnabled: true,
        captionDisplayMode: 'translation',
      },
    }));

    render(<ToolSubtitlesPanel />);

    const switchBtn = screen.getByRole('switch', {
      name: 'settings.enable_realtime_translation',
    });
    fireEvent.click(switchBtn);

    expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(false);
    expect(useConfigStore.getState().config.captionDisplayMode).toBe('bilingual');
  });

  it('renders translation language pairs and swaps them on button click', () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        realtimeTranslationEnabled: true,
        realtimeTranslationSourceLanguage: 'en',
        realtimeTranslationTargetLanguage: 'zh',
      },
    }));

    render(<ToolSubtitlesPanel />);

    const swapBtn = screen.getByRole('button', { name: 'settings.swap_translation_languages' });
    expect(swapBtn).toBeDefined();

    fireEvent.click(swapBtn);

    expect(useConfigStore.getState().config.realtimeTranslationSourceLanguage).toBe('zh');
    expect(useConfigStore.getState().config.realtimeTranslationTargetLanguage).toBe('en');
  });

  it('changes source and target languages using dropdowns', () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        realtimeTranslationEnabled: true,
        realtimeTranslationSourceLanguage: 'en',
        realtimeTranslationTargetLanguage: 'zh',
      },
    }));

    render(<ToolSubtitlesPanel />);

    const sourceDropdownTrigger = screen.getByRole('button', {
      name: 'settings.translation_source_language',
    });
    expect(sourceDropdownTrigger).toBeDefined();

    fireEvent.click(sourceDropdownTrigger);
    const zhOption = screen.getByRole('option', { name: '中文 (Chinese)' });
    fireEvent.click(zhOption);
    expect(useConfigStore.getState().config.realtimeTranslationSourceLanguage).toBe('zh');

    const targetDropdownTrigger = screen.getByRole('button', {
      name: 'settings.translation_target_language',
    });
    expect(targetDropdownTrigger).toBeDefined();

    fireEvent.click(targetDropdownTrigger);
    const enOption = screen.getByRole('option', { name: 'English' });
    fireEvent.click(enOption);
    expect(useConfigStore.getState().config.realtimeTranslationTargetLanguage).toBe('en');
  });

  it('controls bilingual order dropdown visibility based on caption display mode', () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        realtimeTranslationEnabled: true,
        captionDisplayMode: 'bilingual',
      },
    }));

    const { rerender } = render(<ToolSubtitlesPanel />);

    expect(screen.getByText('settings.caption_bilingual_order')).toBeDefined();

    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        captionDisplayMode: 'translation',
      },
    }));

    rerender(<ToolSubtitlesPanel />);
    expect(screen.queryByText('settings.caption_bilingual_order')).toBeNull();

    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        captionDisplayMode: 'original',
      },
    }));

    rerender(<ToolSubtitlesPanel />);
    expect(screen.queryByText('settings.caption_bilingual_order')).toBeNull();
  });

  it('updates translation font size and color in appearance section', () => {
    render(<ToolSubtitlesPanel />);

    const translationFontSizeSlider = screen.getByRole('slider', {
      name: 'live.translation_font_size',
    });
    expect(translationFontSizeSlider).toBeDefined();
    expect(translationFontSizeSlider.getAttribute('value')).toBe('18');

    fireEvent.change(translationFontSizeSlider, { target: { value: '26' } });
    expect(useConfigStore.getState().config.captionTranslationFontSize).toBe(26);

    const fontColorPicker = screen.getByLabelText('live.translation_font_color');
    expect(fontColorPicker).toBeDefined();

    const yellowSwatch = screen.getAllByRole('button', { name: '#facc15' })[1];
    fireEvent.click(yellowSwatch);
    expect(useConfigStore.getState().config.captionTranslationFontColor).toBe('#facc15');
  });

  describe('preview stage reactivity', () => {
    it('renders only original line when real-time translation is disabled', () => {
      useConfigStore.setState((state) => ({
        config: {
          ...state.config,
          realtimeTranslationEnabled: false,
          captionFontSize: 24,
          captionFontColor: '#ffffff',
        },
      }));

      render(<ToolSubtitlesPanel />);

      const original = screen.getByTestId('subtitle-preview-original');
      expect(original).toBeDefined();
      expect(original.style.fontSize).toBe('24px');
      expect(original.style.color).toBe('rgb(255, 255, 255)');
      expect(screen.queryByTestId('subtitle-preview-translation')).toBeNull();
    });

    it('renders only original line when translation is enabled but display mode is original', () => {
      useConfigStore.setState((state) => ({
        config: {
          ...state.config,
          realtimeTranslationEnabled: true,
          captionDisplayMode: 'original',
          captionDisplayPriority: 'original_only',
        },
      }));

      render(<ToolSubtitlesPanel />);

      expect(screen.getByTestId('subtitle-preview-original')).toBeDefined();
      expect(screen.queryByTestId('subtitle-preview-translation')).toBeNull();
    });

    it('renders only translation line when display mode is translation', () => {
      useConfigStore.setState((state) => ({
        config: {
          ...state.config,
          realtimeTranslationEnabled: true,
          captionDisplayMode: 'translation',
          captionDisplayPriority: 'translation_only',
          captionTranslationFontColor: '#6ee7b7',
        },
      }));

      render(<ToolSubtitlesPanel />);

      expect(screen.queryByTestId('subtitle-preview-original')).toBeNull();
      const translation = screen.getByTestId('subtitle-preview-translation');
      expect(translation).toBeDefined();
      expect(translation.style.fontSize).toBe('24px');
      expect(translation.style.color).toBe('rgb(110, 231, 183)');
    });

    it('renders bilingual lines with original first when bilingualOrder is original_first', () => {
      useConfigStore.setState((state) => ({
        config: {
          ...state.config,
          realtimeTranslationEnabled: true,
          captionDisplayMode: 'bilingual',
          captionBilingualOrder: 'original_first',
          captionDisplayPriority: 'original_main',
          captionFontColor: '#ffffff',
          captionTranslationFontSize: 18,
          captionTranslationFontColor: '#6ee7b7',
        },
      }));

      render(<ToolSubtitlesPanel />);

      const stage = screen.getByTestId('subtitle-preview-stage');
      const children = stage.querySelectorAll('p');
      expect(children).toHaveLength(2);
      expect(children[0].getAttribute('data-testid')).toBe('subtitle-preview-original');
      expect(children[1].getAttribute('data-testid')).toBe('subtitle-preview-translation');
      expect(children[0].style.fontSize).toBe('24px');
      expect(children[1].style.fontSize).toBe('18px');
    });

    it('renders bilingual lines with translation first when bilingualOrder is translation_first', () => {
      useConfigStore.setState((state) => ({
        config: {
          ...state.config,
          realtimeTranslationEnabled: true,
          captionDisplayMode: 'bilingual',
          captionBilingualOrder: 'translation_first',
          captionDisplayPriority: 'translation_main',
          captionFontColor: '#ffffff',
          captionTranslationFontSize: 18,
          captionTranslationFontColor: '#6ee7b7',
        },
      }));

      render(<ToolSubtitlesPanel />);

      const stage = screen.getByTestId('subtitle-preview-stage');
      const children = stage.querySelectorAll('p');
      expect(children).toHaveLength(2);
      expect(children[0].getAttribute('data-testid')).toBe('subtitle-preview-translation');
      expect(children[1].getAttribute('data-testid')).toBe('subtitle-preview-original');
      expect(children[0].style.fontSize).toBe('24px');
      expect(children[1].style.fontSize).toBe('18px');
    });
  });

  it('toggles text stroke and updates stroke width and stroke color', () => {
    render(<ToolSubtitlesPanel />);

    const strokeSwitch = screen.getByRole('switch', {
      name: 'settings.caption_text_stroke',
    });
    expect(strokeSwitch.getAttribute('aria-checked')).toBe('false');

    // Toggle on
    fireEvent.click(strokeSwitch);
    expect(useConfigStore.getState().config.captionTextStroke).toBe(true);
    expect(useConfigStore.getState().config.captionStrokeWidth).toBe(2);

    // Stroke width slider
    const strokeWidthSlider = screen.getByRole('slider', {
      name: 'settings.caption_stroke_width',
    });
    fireEvent.change(strokeWidthSlider, { target: { value: '3.5' } });
    expect(useConfigStore.getState().config.captionStrokeWidth).toBe(3.5);

    // Stroke color swatch
    const strokeColorPicker = screen.getByLabelText('settings.caption_stroke_color');
    const colorBtn = within(strokeColorPicker).getByRole('button', { name: '#facc15' });
    fireEvent.click(colorBtn);
    expect(useConfigStore.getState().config.captionStrokeColor).toBe('#facc15');
  });

  it('toggles text shadow and background blur switches', () => {
    render(<ToolSubtitlesPanel />);

    const shadowSwitch = screen.getByRole('switch', {
      name: 'settings.caption_text_shadow',
    });
    expect(shadowSwitch.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(shadowSwitch);
    expect(useConfigStore.getState().config.captionTextShadow).toBe(false);

    const blurSwitch = screen.getByRole('switch', {
      name: 'settings.caption_background_blur',
    });
    expect(blurSwitch.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(blurSwitch);
    expect(useConfigStore.getState().config.captionBackgroundBlur).toBe(false);
  });

  it('updates text alignment and disappear delay', () => {
    render(<ToolSubtitlesPanel />);

    const leftTab = screen.getByRole('radio', { name: 'settings.caption_align_left' });
    fireEvent.click(leftTab);
    expect(useConfigStore.getState().config.captionTextAlign).toBe('left');

    const delaySlider = screen.getByRole('slider', {
      name: 'settings.caption_disappear_delay',
    });
    fireEvent.change(delaySlider, { target: { value: '6' } });
    expect(useConfigStore.getState().config.captionDisappearDelay).toBe(6);
  });

  it('calls captionWindowService.resetPosition when clicking reset position button', () => {
    render(<ToolSubtitlesPanel />);

    const resetBtn = screen.getByRole('button', {
      name: 'settings.caption_reset_position',
    });
    fireEvent.click(resetBtn);
    expect(captionWindowService.resetPosition).toHaveBeenCalledTimes(1);
  });
});
