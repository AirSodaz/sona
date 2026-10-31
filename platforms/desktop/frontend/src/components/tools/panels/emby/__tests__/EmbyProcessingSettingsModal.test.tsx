import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../../../../../stores/configStore';
import type { AppConfig } from '../../../../../types/config';
import { EmbyProcessingSettingsModal } from '../EmbyProcessingSettingsModal';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (_key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? _key,
      i18n: { language: 'en' },
    }),
  };
});

describe('EmbyProcessingSettingsModal', () => {
  beforeEach(() => {
    localStorage.clear();
    useConfigStore.setState({
      config: {
        asr: {
          providers: {
            online: {},
          },
        },
        llmSettings: {
          models: [
            {
              id: 'model-deepseek',
              model: 'deepseek-chat',
              provider: 'deepseek',
              metadata: { displayName: 'DeepSeek V3' },
            },
            {
              id: 'model-gpt4o',
              model: 'gpt-4o',
              provider: 'openai',
            },
          ],
        },
      } as unknown as AppConfig,
    });
  });

  afterEach(() => {
    cleanup();
  });

  it('renders translation options with configured LLM models', () => {
    render(<EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} />);

    const translationDropdown = screen.getByRole('button', { name: 'Translation Model' });
    fireEvent.click(translationDropdown);

    expect(screen.getByText('DeepSeek V3 (deepseek)')).toBeDefined();
    expect(screen.getByText('gpt-4o (openai)')).toBeDefined();
  });

  it('renders the 3 ASR parameters aligned with create page bottom-left: Model, Subtitle Mode, and Language', () => {
    render(<EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} />);

    expect(screen.getByRole('heading', { name: 'Media Processing Settings' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Model' })).toBeDefined();
    expect(screen.getByRole('switch', { name: 'Subtitle Mode' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Recognition Language' })).toBeDefined();
  });

  it('saves enableTimeline when toggled', () => {
    const onSaved = vi.fn();
    render(<EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} onSaved={onSaved} />);

    const timelineSwitch = screen.getByRole('switch', { name: 'Subtitle Mode' });
    expect(timelineSwitch.getAttribute('aria-checked')).toBe('true');

    fireEvent.click(timelineSwitch);
    expect(timelineSwitch.getAttribute('aria-checked')).toBe('false');

    const saveBtn = screen.getByRole('button', { name: 'Save' });
    fireEvent.click(saveBtn);

    expect(onSaved).toHaveBeenCalledWith(
      expect.objectContaining({
        enableTimeline: false,
      })
    );
  });
});
