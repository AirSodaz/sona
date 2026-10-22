import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as reactI18next from 'react-i18next';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { asrConfigService } from '../../services/asrConfigService';
import { modelService } from '../../services/modelService';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import type { ModelCatalogSnapshot } from '../../types/modelCatalog';
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
    vi.restoreAllMocks();
    useTranscriptRuntimeStore.setState({
      mode: 'create',
      ingestionFocus: 'live',
      isRecording: false,
    });
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        enableTimeline: false,
        language: 'en',
      },
    });

    vi.spyOn(asrConfigService, 'resolveActiveLanguageCapability').mockReturnValue({
      languages: ['zh', 'en', 'ja'],
      languageMode: 'selectable',
    });
  });
  const mockSnapshot: ModelCatalogSnapshot = {
    modelsDir: '/models',
    models: [
      {
        id: 'sensevoice-live',
        name: 'SenseVoice Live',
        installPath: '/models/sensevoice-live.onnx',
        isInstalled: true,
      } as any,
      {
        id: 'whisper-live',
        name: 'Whisper Live',
        installPath: '/models/whisper-live.onnx',
        isInstalled: true,
      } as any,
      {
        id: 'sensevoice-batch',
        name: 'SenseVoice Batch',
        installPath: '/models/sensevoice-batch.onnx',
        isInstalled: true,
      } as any,
      {
        id: 'whisper-batch',
        name: 'Whisper Batch',
        installPath: '/models/whisper-batch.onnx',
        isInstalled: true,
      } as any,
    ],
    sections: [],
    selectionOptions: {
      streaming: [
        {
          id: 'sensevoice-live',
          label: 'SenseVoice Live',
          isInstalled: true,
          installPath: '/models/sensevoice-live.onnx',
        },
        {
          id: 'whisper-live',
          label: 'Whisper Live',
          isInstalled: true,
          installPath: '/models/whisper-live.onnx',
        },
      ],
      batch: [
        {
          id: 'sensevoice-batch',
          label: 'SenseVoice Batch',
          isInstalled: true,
          installPath: '/models/sensevoice-batch.onnx',
        },
        {
          id: 'whisper-batch',
          label: 'Whisper Batch',
          isInstalled: true,
          installPath: '/models/whisper-batch.onnx',
        },
      ],
      speakerSegmentation: [],
      speakerEmbedding: [],
    },
    modelPathById: {
      'sensevoice-live': '/models/sensevoice-live.onnx',
      'whisper-live': '/models/whisper-live.onnx',
      'sensevoice-batch': '/models/sensevoice-batch.onnx',
      'whisper-batch': '/models/whisper-batch.onnx',
    },
    modelIdByNormalizedPath: {
      '/models/sensevoice-live.onnx': 'sensevoice-live',
      '/models/whisper-live.onnx': 'whisper-live',
      '/models/sensevoice-batch.onnx': 'sensevoice-batch',
      '/models/whisper-batch.onnx': 'whisper-batch',
    },
    pathMatchTokens: [],
    dependencyRequestsByModelId: {},
    restoreDefaults: { enableITN: true, vadBufferSize: 8, maxConcurrent: 1 },
  };
  const getTriggerButton = () => screen.getByRole('button', { name: /^Preset/i });

  it('renders closed by default with proper aria attributes', () => {
    render(<TranscriptionOptions />);

    const button = getTriggerButton();
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

    const button = getTriggerButton();
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

    const button = getTriggerButton();
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    fireEvent.click(button);
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(button.className).not.toContain('active');
  });

  it('collapses popover on Escape key and restores focus to trigger button', () => {
    render(<TranscriptionOptions />);

    const button = getTriggerButton();
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

    const button = getTriggerButton();
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    fireEvent.mouseDown(screen.getByTestId('outside-area'));
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('keeps popover open when toggling timeline Switch', () => {
    render(<TranscriptionOptions />);

    fireEvent.click(getTriggerButton());
    const switchControl = screen.getByRole('switch', { name: 'batch.timeline_mode' });

    expect(useConfigStore.getState().config.enableTimeline).toBe(false);
    fireEvent.click(switchControl);
    expect(useConfigStore.getState().config.enableTimeline).toBe(true);

    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();
  });

  it('keeps popover open when interacting with portaled language Dropdown', () => {
    render(<TranscriptionOptions />);
    fireEvent.click(getTriggerButton());
    const popover = screen.getByRole('region', { name: 'Parameter Settings' });
    expect(popover).toBeTruthy();

    // Open language dropdown (its trigger has id="parameter-settings-language")
    const dropdownTrigger = popover.querySelector('#parameter-settings-language') as HTMLElement;
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
    const button = getTriggerButton();
    expect((button as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(button);
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('automatically collapses when disabled prop changes to true while open', () => {
    const { rerender } = render(<TranscriptionOptions disabled={false} />);

    const button = getTriggerButton();
    fireEvent.click(button);
    expect(screen.getByRole('region', { name: 'Parameter Settings' })).toBeTruthy();

    rerender(<TranscriptionOptions disabled={true} />);
    expect(screen.queryByRole('region', { name: 'Parameter Settings' })).toBeNull();
  });

  it('disables button in live mode when recording is active', () => {
    useTranscriptRuntimeStore.setState({ isRecording: true });

    render(<TranscriptionOptions surface="live" />);

    const button = getTriggerButton();
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it('renders model selection field in popover and switches models in live surface', async () => {
    vi.spyOn(modelService, 'getModelCatalogSnapshot').mockResolvedValue(mockSnapshot);
    vi.spyOn(modelService, 'latestCatalogSnapshot', 'get').mockReturnValue(mockSnapshot);

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        streamingModelPath: '/models/sensevoice-live.onnx',
        asr: {
          selections: {
            live: {
              engine: 'local',
              mode: 'streaming',
              modelId: 'sensevoice-live',
              modelPath: '/models/sensevoice-live.onnx',
            },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
          },
          providers: { online: {} },
        },
      },
    });

    render(<TranscriptionOptions surface="live" />);

    fireEvent.click(getTriggerButton());
    const popover = screen.getByRole('region', { name: 'Parameter Settings' });
    expect(popover).toBeTruthy();

    // Verify model FormField is present
    expect(screen.getByText('Model')).toBeTruthy();
    expect(screen.getByText('Select transcription model')).toBeTruthy();
    // Open model dropdown
    const modelDropdown = popover.querySelector('#parameter-settings-model') as HTMLElement;
    expect(modelDropdown).toBeTruthy();
    fireEvent.click(modelDropdown);

    // Listbox should contain the streaming options
    const whisperOption = screen.getByRole('option', { name: /Whisper Live/i });
    expect(whisperOption).toBeTruthy();

    // Select Whisper Live
    fireEvent.mouseDown(whisperOption);
    fireEvent.click(whisperOption);

    // Config should be updated to whisper-live
    const updatedConfig = useConfigStore.getState().config;
    expect(updatedConfig.streamingModelPath).toBe('/models/whisper-live.onnx');
    expect(updatedConfig.asr?.selections.live.modelId).toBe('whisper-live');
  });

  it('switches models in batch surface and updates batch configuration', async () => {
    vi.spyOn(modelService, 'getModelCatalogSnapshot').mockResolvedValue(mockSnapshot);
    vi.spyOn(modelService, 'latestCatalogSnapshot', 'get').mockReturnValue(mockSnapshot);

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        batchModelPath: '/models/sensevoice-batch.onnx',
        asr: {
          selections: {
            live: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: {
              engine: 'local',
              mode: 'batch',
              modelId: 'sensevoice-batch',
              modelPath: '/models/sensevoice-batch.onnx',
            },
          },
          providers: { online: {} },
        },
      },
    });

    render(<TranscriptionOptions surface="batch" />);

    fireEvent.click(getTriggerButton());
    const popover = screen.getByRole('region', { name: 'Parameter Settings' });
    expect(popover).toBeTruthy();

    // Open model dropdown
    const modelDropdown = popover.querySelector('#parameter-settings-model') as HTMLElement;
    fireEvent.click(modelDropdown);

    // Select Whisper Batch
    const whisperOption = screen.getByRole('option', { name: /Whisper Batch/i });
    expect(whisperOption).toBeTruthy();
    fireEvent.mouseDown(whisperOption);
    fireEvent.click(whisperOption);

    // Config should be updated to whisper-batch
    const updatedConfig = useConfigStore.getState().config;
    expect(updatedConfig.batchModelPath).toBe('/models/whisper-batch.onnx');
    expect(updatedConfig.asr?.selections.batch.modelId).toBe('whisper-batch');
  });

  it('coerces language when switching model to one with fixed language', async () => {
    vi.spyOn(modelService, 'getModelCatalogSnapshot').mockResolvedValue(mockSnapshot);
    vi.spyOn(modelService, 'latestCatalogSnapshot', 'get').mockReturnValue(mockSnapshot);

    // Start with language 'en'
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        language: 'en',
        streamingModelPath: '/models/sensevoice-live.onnx',
      },
    });

    // Mock coerceConfiguredLanguage to coerce to 'zh' when whisper is selected
    vi.spyOn(asrConfigService, 'coerceConfiguredLanguage').mockImplementation((cfg) => {
      if (cfg.streamingModelPath === '/models/whisper-live.onnx') {
        return 'zh';
      }
      return cfg.language ?? 'en';
    });

    render(<TranscriptionOptions surface="live" />);

    fireEvent.click(getTriggerButton());
    const popover = screen.getByRole('region', { name: 'Parameter Settings' });

    const modelDropdown = popover.querySelector('#parameter-settings-model') as HTMLElement;
    fireEvent.click(modelDropdown);

    const whisperOption = screen.getByRole('option', { name: /Whisper Live/i });
    fireEvent.mouseDown(whisperOption);
    fireEvent.click(whisperOption);

    // Language should have been coerced to 'zh'
    expect(useConfigStore.getState().config.language).toBe('zh');
  });

  it('renders pill layout with status dot and active model/language summaries', () => {
    vi.spyOn(modelService, 'latestCatalogSnapshot', 'get').mockReturnValue(mockSnapshot);
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        asr: {
          selections: {
            live: {
              engine: 'local',
              mode: 'streaming',
              modelId: 'sensevoice-live',
              modelPath: '/models/sensevoice-live.onnx',
            },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
          },
          providers: { online: {} },
        },
        streamingModelPath: '/models/sensevoice-live.onnx',
        language: 'en',
        enableTimeline: true,
      },
    });

    render(<TranscriptionOptions surface="live" />);

    const button = getTriggerButton();
    expect(button.querySelector('.parameter-settings-status-dot')).toBeTruthy();
    expect(button.querySelector('.parameter-settings-capsule-summary')).toBeTruthy();
    expect(button.querySelector('.parameter-settings-summary-model')?.textContent).toBe(
      'SenseVoice Live'
    );
    expect(button.querySelector('.parameter-settings-summary-divider')?.textContent).toBe('/');
    expect(button.querySelector('.parameter-settings-summary-lang')?.textContent).toBe('English');
    expect(button.getAttribute('title')).toBeNull();
    expect(button.getAttribute('aria-label')).toBe('Preset: SenseVoice Live, English');
  });

  it('refreshes catalog snapshot when opening popover', async () => {
    const getSnapshotSpy = vi
      .spyOn(modelService, 'getModelCatalogSnapshot')
      .mockResolvedValue(mockSnapshot);

    render(<TranscriptionOptions surface="live" />);
    expect(getSnapshotSpy).toHaveBeenCalledTimes(1);

    fireEvent.click(getTriggerButton());
    await waitFor(() => {
      expect(getSnapshotSpy).toHaveBeenCalledTimes(2);
    });
  });

  it('coerces language in capsule summary when active model enforces auto or fixed language', () => {
    vi.spyOn(modelService, 'latestCatalogSnapshot', 'get').mockReturnValue(mockSnapshot);
    vi.spyOn(asrConfigService, 'coerceConfiguredLanguage').mockReturnValue('auto');

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        language: 'en',
      },
    });

    render(<TranscriptionOptions surface="live" />);

    const button = getTriggerButton();
    expect(button.querySelector('.parameter-settings-summary-lang')?.textContent).toBe('Auto');
    expect(button.getAttribute('aria-label')).toContain('Auto');
  });
});
