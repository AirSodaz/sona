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
  it('saves speakerDiarization when toggled and falls back to global config default', () => {
    const onSaved = vi.fn();
    useConfigStore.setState({
      config: {
        batchSpeakerDiarizationEnabled: false,
        asr: { providers: { online: {} } },
        llmSettings: { models: [] },
      } as unknown as AppConfig,
    });

    const { unmount } = render(
      <EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} onSaved={onSaved} />
    );

    const speakerSwitch = screen.getByRole('switch', { name: 'Speaker Diarization' });
    expect(speakerSwitch.getAttribute('aria-checked')).toBe('false');

    fireEvent.click(speakerSwitch);
    expect(speakerSwitch.getAttribute('aria-checked')).toBe('true');

    const saveBtn = screen.getByRole('button', { name: 'Save' });
    fireEvent.click(saveBtn);

    expect(onSaved).toHaveBeenCalledWith(
      expect.objectContaining({
        speakerDiarization: true,
      })
    );
    unmount();
  });

  it('falls back to online provider default when online ASR is active', () => {
    useConfigStore.setState({
      config: {
        batchSpeakerDiarizationEnabled: false,
        asr: {
          selections: {
            batch: {
              engine: 'online',
              mode: 'batch',
              providerId: 'volcengine-doubao',
            },
          },
          providers: {
            online: {
              'volcengine-doubao': {
                speakerDiarization: true,
              },
            },
          },
        },
        llmSettings: { models: [] },
      } as unknown as AppConfig,
    });

    const { unmount } = render(<EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} />);

    const speakerSwitch = screen.getByRole('switch', { name: 'Speaker Diarization' });
    expect(speakerSwitch.getAttribute('aria-checked')).toBe('true');
    unmount();
  });

  it('clamps speakerDiarization to false when active online provider does not support it', () => {
    const onSaved = vi.fn();
    useConfigStore.setState({
      config: {
        batchSpeakerDiarizationEnabled: false,
        asr: {
          selections: {
            batch: {
              engine: 'online',
              mode: 'batch',
              providerId: 'groq-whisper',
            },
          },
          providers: {
            online: {},
          },
        },
        llmSettings: { models: [] },
      } as unknown as AppConfig,
    });

    localStorage.setItem(
      'sona:emby:tool_overrides',
      JSON.stringify({
        speakerDiarization: true,
      })
    );

    const { unmount } = render(
      <EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} onSaved={onSaved} />
    );

    const speakerSwitch = screen.getByRole('switch', { name: 'Speaker Diarization' });
    expect(speakerSwitch.getAttribute('aria-checked')).toBe('false');
    expect(speakerSwitch.getAttribute('aria-disabled')).toBe('true');

    const saveBtn = screen.getByRole('button', { name: 'Save' });
    fireEvent.click(saveBtn);

    expect(onSaved).toHaveBeenCalledWith(
      expect.objectContaining({
        speakerDiarization: false,
      })
    );
    unmount();
  });
  it('resolves fallback diarization to false when active online provider does not support it and no overrides exist', () => {
    useConfigStore.setState({
      config: {
        batchSpeakerDiarizationEnabled: true,
        asr: {
          selections: {
            batch: {
              engine: 'online',
              mode: 'batch',
              providerId: 'groq-whisper',
            },
          },
          providers: {
            online: {},
          },
        },
        llmSettings: { models: [] },
      } as unknown as AppConfig,
    });

    localStorage.removeItem('sona:emby:tool_overrides');

    const { unmount } = render(
      <EmbyProcessingSettingsModal isOpen={true} onClose={vi.fn()} onSaved={vi.fn()} />
    );

    const speakerSwitch = screen.getByRole('switch', { name: 'Speaker Diarization' });
    expect(speakerSwitch.getAttribute('aria-checked')).toBe('false');
    expect(speakerSwitch.getAttribute('aria-disabled')).toBe('true');
    unmount();
  });
});
