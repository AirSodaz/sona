import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useVoiceTypingHistoryStore } from '../../stores/voiceTypingHistoryStore';
import { SettingsSubtitleTab } from '../settings/SettingsSubtitleTab';

// Mock translation
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

const mockUpdateConfig = vi.fn();
const mockReadiness = vi.hoisted(() => ({
  state: 'ready' as 'ready' | 'failed',
  lastErrorSource: null as null | 'shortcut_registration' | 'warmup' | 'microphone' | 'session',
  lastErrorMessage: null as string | null,
}));

vi.mock('../../hooks/useVoiceTypingReadiness', () => ({
  useVoiceTypingReadiness: () => mockReadiness,
}));

vi.mock('../Dropdown', () => ({
  Dropdown: ({ id, value, onChange, options }: any) => (
    <select id={id} value={value} onChange={(event) => onChange?.(event.target.value)}>
      {options?.map((option: any) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock('../settings/SettingsShortcutInput', () => ({
  SettingsShortcutInput: ({ value, onChange }: any) => (
    <input
      aria-label="voice typing shortcut"
      value={value}
      onChange={(event) => onChange?.(event.target.value)}
    />
  ),
}));

vi.mock('../../stores/configStore', () => ({
  useCaptionConfig: () => ({
    lockWindow: false,
    alwaysOnTop: true,
    startOnLaunch: false,
    captionWindowWidth: 800,
    captionFontSize: 24,
    captionFontColor: '#ffffff',
    captionBackgroundColor: '#000000',
    captionBackgroundOpacity: 0.6,
  }),
  useVoiceTypingConfig: () => ({
    voiceTypingEnabled: false,
    voiceTypingShortcut: 'Alt+V',
    voiceTypingMode: 'hold',
  }),
  useSetConfig: () => mockUpdateConfig,
  useConfigStore: {
    getState: () => ({
      config: {
        hotwordSets: [],
      },
    }),
  },
}));

describe('SettingsSubtitleTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockReadiness.state = 'ready';
    mockReadiness.lastErrorSource = null;
    mockReadiness.lastErrorMessage = null;
  });

  it('renders secondary tabs and defaults to voice typing', () => {
    render(<SettingsSubtitleTab />);

    const vtTab = screen.getByRole('tab', { name: 'settings.voice_typing' });
    const subtitleTab = screen.getByRole('tab', { name: 'live.subtitle_settings' });

    expect(vtTab.getAttribute('aria-selected')).toBe('true');
    expect(subtitleTab.getAttribute('aria-selected')).toBe('false');

    screen.getByText('settings.enable_voice_typing');
    screen.getByText('settings.voice_typing_mode');
    screen.getByText('settings.voice_typing_availability');

    // Switch to subtitles tab
    fireEvent.click(subtitleTab);
    expect(subtitleTab.getAttribute('aria-selected')).toBe('true');
    expect(vtTab.getAttribute('aria-selected')).toBe('false');

    screen.getByText('live.start_on_launch');
    screen.getByText('live.lock_window');
    screen.getByText('live.always_on_top');
    screen.getByText('live.window_width');
    screen.getByText('live.font_size');
    screen.getByText('live.font_color');
    screen.getByText('live.background_color');
  });

  it('navigates secondary tabs using arrow keys', () => {
    render(<SettingsSubtitleTab />);

    const vtTab = screen.getByRole('tab', { name: 'settings.voice_typing' });
    const subtitleTab = screen.getByRole('tab', { name: 'live.subtitle_settings' });

    expect(vtTab.getAttribute('aria-selected')).toBe('true');

    fireEvent.keyDown(vtTab, { key: 'ArrowRight' });
    expect(subtitleTab.getAttribute('aria-selected')).toBe('true');

    fireEvent.keyDown(subtitleTab, { key: 'ArrowLeft' });
    expect(vtTab.getAttribute('aria-selected')).toBe('true');
  });
  it('renders width input with correct values and classes', () => {
    render(<SettingsSubtitleTab initialSubTab="subtitles" />);

    const numberInput = screen.getByDisplayValue('800');

    expect(numberInput).toBeDefined();
    expect(numberInput.tagName).toBe('INPUT');
    expect(numberInput.getAttribute('type')).toBe('number');
    expect(numberInput.classList.contains('settings-input')).toBe(true);

    // Ensure range slider is removed
    const widthRange = document.querySelector('input[type="range"][value="800"]');
    expect(widthRange).toBeNull();
  });

  it('calls updateConfig when inputs change', () => {
    render(<SettingsSubtitleTab initialSubTab="subtitles" />);

    const numberInput = screen.getByDisplayValue('800');

    fireEvent.change(numberInput, { target: { value: '1200' } });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ captionWindowWidth: 1200 });
  });

  it('renders font size input with correct classes', () => {
    render(<SettingsSubtitleTab initialSubTab="subtitles" />);

    const numberInput = screen.getByDisplayValue('24');

    expect(numberInput).toBeDefined();
    expect(numberInput.tagName).toBe('INPUT');
    expect(numberInput.getAttribute('type')).toBe('number');
    expect(numberInput.classList.contains('settings-input')).toBe(true);

    // Ensure range slider is removed
    const fontRange = document.querySelector('input[type="range"][value="24"]');
    expect(fontRange).toBeNull();
  });

  it('renders color swatch picker with correct structure and allows changing font color', () => {
    render(<SettingsSubtitleTab initialSubTab="subtitles" />);

    // Font color container exists
    const fontColorContainer = screen.getByLabelText('live.font_color');
    expect(fontColorContainer).toBeDefined();
    expect(fontColorContainer.className).toContain('project-color-swatches');

    // Preset color swatches are rendered within font color picker
    const emeraldSwatch = within(fontColorContainer).getByRole('button', { name: '#10B981' });
    expect(emeraldSwatch).toBeDefined();

    // Clicking a preset color updates captionFontColor
    fireEvent.click(emeraldSwatch);
    expect(mockUpdateConfig).toHaveBeenCalledWith({ captionFontColor: '#10B981' });

    // Custom color button is rendered with project-custom-color-swatch class
    const customColorBtn = within(fontColorContainer).getByRole('button', {
      name: 'common.custom_color',
    });
    expect(customColorBtn.className).toContain('project-custom-color-swatch');

    // Clicking custom color button opens ColorPicker
    fireEvent.click(customColorBtn);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();

    // Changing color via ColorPicker updates captionFontColor
    const toggleBtn = screen.getByLabelText('Toggle RGB / HEX');
    fireEvent.click(toggleBtn);
    const hexInput = document.querySelector(
      '.sona-color-picker-popover input[type="text"]'
    ) as HTMLInputElement;
    fireEvent.change(hexInput, { target: { value: '#FFE600' } });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ captionFontColor: '#FFE600' });
  });

  it('renders color swatch picker for background color and allows changing it', () => {
    render(<SettingsSubtitleTab initialSubTab="subtitles" />);

    // Background color container exists
    const bgColorContainer = screen.getByLabelText('live.background_color');
    expect(bgColorContainer).toBeDefined();
    expect(bgColorContainer.className).toContain('project-color-swatches');

    // Preset color swatches are rendered within background color picker
    const cyanSwatch = within(bgColorContainer).getByRole('button', { name: '#06B6D4' });
    expect(cyanSwatch).toBeDefined();

    // Clicking a preset color updates captionBackgroundColor
    fireEvent.click(cyanSwatch);
    expect(mockUpdateConfig).toHaveBeenCalledWith({ captionBackgroundColor: '#06B6D4' });

    // Custom color button is rendered
    const customColorBtn = within(bgColorContainer).getByRole('button', {
      name: 'common.custom_color',
    });
    expect(customColorBtn.className).toContain('project-custom-color-swatch');

    // Clicking custom color button opens ColorPicker
    fireEvent.click(customColorBtn);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();

    // Changing color via ColorPicker updates captionBackgroundColor
    const toggleBtn = screen.getByLabelText('Toggle RGB / HEX');
    fireEvent.click(toggleBtn);
    const hexInput = document.querySelector(
      '.sona-color-picker-popover input[type="text"]'
    ) as HTMLInputElement;
    fireEvent.change(hexInput, { target: { value: '#1A1A1A' } });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ captionBackgroundColor: '#1A1A1A' });
  });

  it('updates voice typing settings from the combined page', () => {
    render(<SettingsSubtitleTab />);

    const switchBtns = screen.getAllByRole('switch');
    fireEvent.click(switchBtns[0]);
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingEnabled: true });

    const shortcutInputs = screen.getAllByLabelText('voice typing shortcut');
    fireEvent.change(shortcutInputs[0], {
      target: { value: 'Ctrl+Alt+V' },
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingShortcut: 'Ctrl+Alt+V' });

    fireEvent.change(shortcutInputs[1], {
      target: { value: 'Ctrl+Shift+H' },
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({
      voiceTypingQuickRecallShortcut: 'Ctrl+Shift+H',
    });
    fireEvent.change(document.querySelector('#vt-mode-select') as HTMLSelectElement, {
      target: { value: 'toggle' },
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingMode: 'toggle' });

    fireEvent.change(document.querySelector('#vt-processing-mode-select') as HTMLSelectElement, {
      target: { value: 'polish' },
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingProcessingMode: 'polish' });

    fireEvent.change(document.querySelector('#vt-placement-select') as HTMLSelectElement, {
      target: { value: 'bottom_center' },
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingPlacement: 'bottom_center' });
    fireEvent.click(switchBtns[1]);
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingSoundEnabled: false });

    fireEvent.click(switchBtns[2]);
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingCjkSpacingEnabled: false });

    fireEvent.click(switchBtns[3]);
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingContextAwarenessEnabled: false });

    fireEvent.change(document.querySelector('#vt-context-preset-select') as HTMLSelectElement, {
      target: { value: 'developer' },
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingContextPreset: 'developer' });
  });
  it('shows only simplified availability and the runtime failure reason', () => {
    mockReadiness.state = 'failed';
    mockReadiness.lastErrorSource = 'microphone';
    mockReadiness.lastErrorMessage = 'Microphone is unavailable.';

    render(<SettingsSubtitleTab />);

    screen.getByText('settings.voice_typing_unavailable');
    screen.getByText('settings.voice_typing_failure_reason_with_source');
    expect(screen.queryByText('settings.voice_typing_dependencies')).toBeNull();
    expect(screen.queryByText('settings.voice_typing_open_model_hub')).toBeNull();
    expect(screen.queryByText('settings.voice_typing_open_input_device')).toBeNull();
  });
  it('renders history empty state and history items with actions', async () => {
    useVoiceTypingHistoryStore.getState().clearHistory();
    const { rerender } = render(<SettingsSubtitleTab />);

    expect(screen.getByTestId('voice-typing-history-empty')).toBeTruthy();
    screen.getByText('settings.voice_typing_history');

    // Add an item to history store
    useVoiceTypingHistoryStore.getState().addItem({
      rawText: 'like the weather is great today',
      polishedText: 'The weather is great today.',
      injectedText: 'The weather is great today.',
      mode: 'polish',
    });

    rerender(<SettingsSubtitleTab />);

    expect(screen.queryByTestId('voice-typing-history-empty')).toBeNull();
    screen.getByText('The weather is great today.');

    screen.getByText('settings.voice_typing_mode_badge_polish');

    // Test add to hotwords
    const addHotwordBtn = screen.getByText('settings.voice_typing_add_hotword');
    fireEvent.click(addHotwordBtn);
    expect(mockUpdateConfig).toHaveBeenCalled();
    const updateCall = mockUpdateConfig.mock.calls.find((call) => call[0]?.dictionaryContent);
    expect(updateCall).toBeDefined();
    expect(updateCall![0].dictionaryContent).toContain('(id:voice-typing):');
    expect(updateCall![0].dictionaryContent).toContain('- The weather is great today.');
  });
});
