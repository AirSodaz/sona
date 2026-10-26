import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../../../../stores/configStore';
import { useTranscriptRuntimeStore } from '../../../../stores/transcriptRuntimeStore';
import { buildTestConfig } from '../../../../test-utils/configTestUtils';
import { ToolSubtitlesPanel } from '../ToolSubtitlesPanel';

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

  it('toggles real-time translation switch and updates config', () => {
    render(<ToolSubtitlesPanel />);

    const switchBtn = screen.getByRole('switch', {
      name: 'settings.enable_realtime_translation',
    });
    expect(switchBtn.getAttribute('aria-checked')).toBe('false');

    fireEvent.click(switchBtn);

    expect(useConfigStore.getState().config.realtimeTranslationEnabled).toBe(true);
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
          captionTranslationFontSize: 20,
          captionTranslationFontColor: '#6ee7b7',
        },
      }));

      render(<ToolSubtitlesPanel />);

      expect(screen.queryByTestId('subtitle-preview-original')).toBeNull();
      const translation = screen.getByTestId('subtitle-preview-translation');
      expect(translation).toBeDefined();
      expect(translation.style.fontSize).toBe('20px');
      expect(translation.style.color).toBe('rgb(110, 231, 183)');
    });

    it('renders bilingual lines with original first when bilingualOrder is original_first', () => {
      useConfigStore.setState((state) => ({
        config: {
          ...state.config,
          realtimeTranslationEnabled: true,
          captionDisplayMode: 'bilingual',
          captionBilingualOrder: 'original_first',
          captionFontSize: 24,
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
          captionFontSize: 24,
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
      expect(children[0].style.fontSize).toBe('18px');
      expect(children[1].style.fontSize).toBe('24px');
    });
  });
});
