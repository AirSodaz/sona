import { fireEvent, render, screen } from '@testing-library/react';
import type * as reactI18next from 'react-i18next';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { asrConfigService } from '../../services/asrConfigService';
import { useConfigStore } from '../../stores/configStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { TranscriptionOptions } from '../TranscriptionOptions';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof reactI18next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: { defaultValue?: string }) => {
        if (options?.defaultValue) return options.defaultValue;
        return key;
      },
      i18n: { language: 'en' },
    }),
  };
});

describe('TranscriptionOptions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTranscriptRuntimeStore.setState({
      mode: 'live',
      isRecording: false,
    });
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        enableTimeline: false,
        language: 'en',
      },
    });

    vi.spyOn(asrConfigService, 'resolveActiveLanguageCapability').mockReturnValue({
      languages: ['zh', 'en', 'ja'],
      languageMode: 'selectable',
    });
  });

  it('renders closed by default with proper aria attributes', () => {
    render(<TranscriptionOptions />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    expect(button).toBeTruthy();
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(button.getAttribute('aria-haspopup')).toBe('true');
    expect(button.getAttribute('aria-controls')).toBe('parameter-settings-popover');
    expect(button.className).not.toContain('active');

    const chevron = button.querySelector('.parameter-settings-chevron');
    expect(chevron).toBeTruthy();
    expect(chevron?.classList.contains('open')).toBe(false);

    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('expands popover on click and shows timeline and language options', () => {
    render(<TranscriptionOptions />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    fireEvent.click(button);

    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(button.className).toContain('active');

    const chevron = button.querySelector('.parameter-settings-chevron');
    expect(chevron?.classList.contains('open')).toBe(true);

    const popover = screen.getByRole('region', { name: 'Parameter Settings' });
    expect(popover).toBeTruthy();
    expect(popover.id).toBe('parameter-settings-popover');

    expect(screen.getByText('batch.timeline_mode')).toBeTruthy();
    expect(screen.getByText('batch.language')).toBeTruthy();
    expect(screen.getByRole('switch', { name: 'batch.timeline_mode' })).toBeTruthy();
  });

  it('collapses popover when clicking the trigger button again', () => {
    render(<TranscriptionOptions />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    fireEvent.click(button);
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(button.className).not.toContain('active');
  });

  it('collapses popover on Escape key and restores focus to trigger button', () => {
    render(<TranscriptionOptions />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    button.focus();
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
    expect(document.activeElement).toBe(button);
  });

  it('collapses popover when clicking outside', () => {
    render(
      <div>
        <div data-testid="outside-area">Outside Area</div>
        <TranscriptionOptions />
      </div>
    );

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    fireEvent.mouseDown(screen.getByTestId('outside-area'));
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('keeps popover open when toggling timeline Switch', () => {
    render(<TranscriptionOptions />);

    fireEvent.click(screen.getByRole('button', { name: 'Parameter Settings' }));
    const switchControl = screen.getByRole('switch', { name: 'batch.timeline_mode' });

    expect(useConfigStore.getState().config.enableTimeline).toBe(false);
    fireEvent.click(switchControl);
    expect(useConfigStore.getState().config.enableTimeline).toBe(true);

    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();
  });

  it('keeps popover open when interacting with portaled language Dropdown', () => {
    render(<TranscriptionOptions />);

    fireEvent.click(screen.getByRole('button', { name: 'Parameter Settings' }));
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    // Open language dropdown (its trigger is inside the FormField)
    const dropdownTrigger = screen.getByRole('button', { expanded: false });
    fireEvent.click(dropdownTrigger);

    const listbox = screen.getByRole('listbox');
    expect(listbox).toBeTruthy();

    // Find and select Chinese option
    const chineseOption = screen.getByRole('option', { name: /Chinese/i });
    fireEvent.mouseDown(chineseOption);
    fireEvent.click(chineseOption);

    expect(useConfigStore.getState().config.language).toBe('zh');
    // Popover remains open after dropdown selection
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();
  });

  it('disables button when disabled prop is true and prevents expanding', () => {
    render(<TranscriptionOptions disabled={true} />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    expect((button as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(button);
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('automatically collapses when disabled prop changes to true while open', () => {
    const { rerender } = render(<TranscriptionOptions disabled={false} />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    rerender(<TranscriptionOptions disabled={true} />);
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('disables button in live mode when recording is active', () => {
    useTranscriptRuntimeStore.setState({ isRecording: true });

    render(<TranscriptionOptions surface="live" />);

    const button = screen.getByRole('button', { name: 'Parameter Settings' });
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });
});
