import { expect, vi, beforeEach, describe, it } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import * as tauriApi from '@tauri-apps/api/core';
import { SettingsLLMServiceTab } from '../SettingsLLMServiceTab';
import { ProviderDetailsModal } from '../llm/ProviderDetailsModal';
import type { AppConfig } from '../../../types/config';
import type { LlmProvider } from '../../../types/transcript';
import {
  addLlmModel,
  buildLlmConfigPatch,
  createLlmSettings,
  findLlmModelId,
  syncProviderDiscoveredModels,
  setFeatureModelSelection,
  setFeatureReasoningEnabled,
  setFeatureTemperature,
  updateProviderSetting,
} from '../../../services/llm/state';
import { buildTestConfig } from '../../../test-utils/configTestUtils';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key.startsWith('settings.llm_providers.')) {
        return (options?.defaultValue as string) || key;
      }
      return key;
    },
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

function buildConfig(provider: LlmProvider = 'open_ai', includeApiKey = true): AppConfig {
  const baseConfig = buildTestConfig({
    streamingModelPath: '/path/to/model',
    batchModelPath: '',
    language: 'auto',
    summaryEnabled: true,
    llmSettings: createLlmSettings(provider),
  });

  let llmSettings = updateProviderSetting(baseConfig.llmSettings, provider, {
    apiKey: includeApiKey ? 'test-key' : '',
  });
  llmSettings = addLlmModel(llmSettings, {
    provider,
    model: provider === 'azure_openai' ? 'deployment-1' : 'gpt-4o',
  });
  llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
  llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);
  llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

  return {
    ...baseConfig,
    ...buildLlmConfigPatch(llmSettings),
  };
}

const mockUpdateConfig = vi.fn();
let currentConfig = buildConfig();

vi.mock('../../../stores/configStore', async () => {
  const actual = await vi.importActual<typeof import('../../../stores/configStore')>('../../../stores/configStore');
  return {
    ...actual,
    useLlmAssistantConfig: () => currentConfig,
    useSetConfig: () => mockUpdateConfig,
  };
});

describe('SettingsLLMServiceTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    currentConfig = buildConfig();
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return [
          { model: 'gpt-4o', contextWindow: 128000, supportsTools: true },
          { model: 'gpt-4.1-mini', supportsReasoning: true },
        ];
      }
      return 'OK';
    });
  });

  it('does not list provider models while inactive for tab prewarm', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab isActive={false} />,
      );
      await Promise.resolve();
    });

    expect(tauriApi.invoke).not.toHaveBeenCalledWith('list_llm_models', expect.anything());
  });

  it('shows details for non-google providers and keeps test connection for google providers', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    expect(screen.getByRole('button', { name: 'settings.llm.details' })).toBeDefined();

    currentConfig = buildConfig('google_translate_free', false);
    currentConfig.llmSettings = setFeatureModelSelection(
      addLlmModel(currentConfig.llmSettings, { provider: 'google_translate_free', model: 'default' }),
      'translation',
      'google_translate_free-default',
    );

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    expect(screen.getByRole('button', { name: 'settings.llm.test_connection' })).toBeDefined();
  });

  it('uses the llm_service tabpanel id expected by the settings tab button', async () => {
    let container!: HTMLElement;
    await act(async () => {
      ({ container } = render(
        <SettingsLLMServiceTab />,
      ));
    });

    expect(container.querySelector('#settings-panel-llm_service')).not.toBeNull();
    expect(container.querySelector('#settings-panel-llm')).toBeNull();
  });

  it('renders feature cards in polish-translation-summary order and keeps the credentials section', async () => {
    let container!: HTMLElement;
    await act(async () => {
      ({ container } = render(
        <SettingsLLMServiceTab />,
      ));
    });

    expect(screen.getByText('settings.llm.title')).toBeDefined();
    expect(screen.getByText('settings.llm.feature_models')).toBeDefined();
    expect(screen.getByText('settings.llm.enable_summary')).toBeDefined();
    expect(screen.getByText('settings.llm.polish_model')).toBeDefined();
    expect(screen.getByText('settings.llm.translation_model')).toBeDefined();
    expect(screen.getByText('settings.llm.summary_model')).toBeDefined();
    expect(screen.getByText('settings.llm.credentials_section')).toBeDefined();

    expect(
      Array.from(container.querySelectorAll('[data-feature-id]')).map((node) => node.getAttribute('data-feature-id')),
    ).toEqual(['polish', 'translation', 'summary']);
  });

  it('renders active provider fields from llmSettings in accordion', async () => {
    const conf = buildConfig();
    conf.llmSettings!.providers['open_ai']!.apiHost = 'test-host';
    currentConfig = conf;

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    // OpenAI is the active provider and so its accordion is expanded by default.
    expect(screen.getByDisplayValue('test-host')).toBeDefined();
    expect(screen.getByDisplayValue('test-key')).toBeDefined();
  });

  it('keeps expanded provider fields grouped inside the matching accordion content', async () => {
    const conf = buildConfig();
    conf.llmSettings!.providers['open_ai']!.apiHost = 'test-host';
    currentConfig = conf;

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const content = screen.getByTestId('provider-accordion-content-open_ai');
    expect(content.querySelector('#llm-open_ai-host')).not.toBeNull();
    expect(content.querySelector('#llm-open_ai-key')).not.toBeNull();
    expect(screen.queryByTestId('provider-accordion-content-gemini')).toBeNull();
  });

  it('shows missing api key status', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    await waitFor(() => {
      expect(screen.getAllByText('settings.llm.status_missing_api_key').length).toBeGreaterThan(0);
    });
  });

  it('fills Gemini host with the default host in accordion', async () => {
    const conf = buildConfig('gemini');
    conf.llmSettings!.providers['gemini']!.apiHost = 'gemini-host';
    currentConfig = conf;

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    // Gemini is the active provider and expanded by default.
    expect(screen.getByDisplayValue('gemini-host')).toBeDefined();
  });

  it('shows candidates only while the model input is focused', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const modelInputs = screen.getAllByPlaceholderText('gpt-4o-mini'); // Default placeholder for OpenAI
    const modelInput = modelInputs[0]; // Polish model input

    expect(screen.queryByText('gpt-4o')).toBeNull();

    await act(async () => {
      fireEvent.focus(modelInput);
      fireEvent.change(modelInput, { target: { value: '' } });
    });

    await waitFor(() => {
      expect(screen.getByText('gpt-4o')).toBeDefined();
    });

    await act(async () => {
      fireEvent.blur(modelInput);
    });

    await waitFor(() => {
      expect(screen.queryByText('gpt-4o')).toBeNull();
    });
  });

  it('uses persisted provider models instead of refetching candidates when they already exist', async () => {
    let llmSettings = buildConfig('open_ai').llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1', contextWindow: 128000 },
      { model: 'gpt-4.1-mini', supportsReasoning: true },
    ], new Date().toISOString());
    currentConfig = {
      ...buildConfig('open_ai'),
      llmSettings,
    };

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const modelInputs = screen.getAllByDisplayValue('gpt-4o');
    await act(async () => {
      fireEvent.focus(modelInputs[0]);
      fireEvent.change(modelInputs[0], { target: { value: '' } });
    });

    await waitFor(() => {
      expect(screen.getByText('gpt-4.1')).toBeDefined();
      expect(screen.getByText('gpt-4.1-mini')).toBeDefined();
    });

    expect(vi.mocked(tauriApi.invoke)).not.toHaveBeenCalledWith('list_llm_models', expect.anything());
  });

  it('refreshes expired persisted provider models for feature candidates and writes them back', async () => {
    let llmSettings = buildConfig('open_ai').llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1' },
    ], '2026-05-22T10:00:00.000Z');
    currentConfig = {
      ...buildConfig('open_ai'),
      llmSettings,
    };

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(vi.mocked(tauriApi.invoke)).toHaveBeenCalledWith('list_llm_models', expect.objectContaining({
        request: expect.objectContaining({
          provider: 'open_ai',
          apiKey: 'test-key',
        }),
      }));
    });

    const nextSettings = mockUpdateConfig.mock.calls[mockUpdateConfig.mock.calls.length - 1]?.[0].llmSettings;
    expect(nextSettings.modelDiscovery.open_ai).toEqual(expect.objectContaining({
      fetchedAt: expect.any(String),
      expiresAt: expect.any(String),
    }));
    expect(findLlmModelId(nextSettings, 'open_ai', 'gpt-4.1-mini')).toBeDefined();
  });

  it('adds a model through the searchable model input flow', async () => {
    let resolveDescription!: (value: { model: string; displayName: string }) => void;
    const description = new Promise<{ model: string; displayName: string }>((resolve) => {
      resolveDescription = resolve;
    });
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return [{ model: 'gpt-4o' }];
      }
      if (command === 'describe_llm_model') {
        return description;
      }
      return 'OK';
    });

    let unmount!: () => void;
    await act(async () => {
      ({ unmount } = render(
        <SettingsLLMServiceTab />,
      ));
    });

    const modelInputs = screen.getAllByPlaceholderText('gpt-4o-mini');
    const modelInput = modelInputs[0];

    await act(async () => {
      fireEvent.focus(modelInput);
      fireEvent.change(modelInput, { target: { value: 'gpt-4.2-new' } });
      fireEvent.keyDown(modelInput, { key: 'Enter' });
    });

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith('describe_llm_model', {
        config: expect.objectContaining({ model: 'gpt-4.2-new' }),
      });
    });
    const updateCount = mockUpdateConfig.mock.calls.length;
    await act(async () => {
      unmount();
      resolveDescription({ model: 'gpt-4.2-new', displayName: 'GPT-4.2 New' });
      await description;
    });
    expect(mockUpdateConfig).toHaveBeenCalledTimes(updateCount);
  });

  it('renders unified temperature controls for all three features', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    expect(screen.queryByTestId('provider-temperature-number')).toBeNull();
    expect(screen.getAllByText('settings.llm.temperature')).toHaveLength(3);
    expect(screen.getAllByRole('spinbutton')).toHaveLength(4);
  });

  it('updates polish temperature independently', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const sliders = screen.getAllByRole('spinbutton');
    await act(async () => {
      fireEvent.change(sliders[0], { target: { value: '0.25' } });
    });

    expect(mockUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({
      llmSettings: expect.objectContaining({
        selections: expect.objectContaining({
          polishTemperature: 0.25,
        }),
      }),
    }));
  });

  it('updates translation temperature independently', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const sliders = screen.getAllByRole('spinbutton');
    await act(async () => {
      fireEvent.change(sliders[1], { target: { value: '1.1' } });
    });

    expect(mockUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({
      llmSettings: expect.objectContaining({
        selections: expect.objectContaining({
          translationTemperature: 1.1,
        }),
      }),
    }));
  });

  it('updates summary temperature independently', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const sliders = screen.getAllByRole('spinbutton');
    await act(async () => {
      fireEvent.change(sliders[2], { target: { value: '0.6' } });
    });

    expect(mockUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({
      llmSettings: expect.objectContaining({
        selections: expect.objectContaining({
          summaryTemperature: 0.6,
        }),
      }),
    }));
  });

  it('keeps temperature controls editable when reasoning mode is enabled', async () => {
    let llmSettings = buildConfig('open_ai').llmSettings!;
    llmSettings = addLlmModel(llmSettings, {
      provider: 'open_ai',
      model: 'gpt-4.1-mini',
      metadata: { supportsReasoning: true },
    });
    const reasoningModelId = llmSettings.modelOrder.find((id) => llmSettings.models[id]?.model === 'gpt-4.1-mini')!;
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', reasoningModelId);
    llmSettings = setFeatureReasoningEnabled(llmSettings, 'polish', true);
    llmSettings = setFeatureTemperature(llmSettings, 'polish', 0.35);
    currentConfig = {
      ...buildConfig('open_ai'),
      ...buildLlmConfigPatch(llmSettings),
    };

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const temperatureInputs = screen.getAllByRole('spinbutton') as HTMLInputElement[];
    expect(temperatureInputs[0].disabled).toBe(false);
    expect(temperatureInputs[0].value).toBe('0.35');

    await act(async () => {
      fireEvent.change(temperatureInputs[0], { target: { value: '0.55' } });
    });

    expect(mockUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({
      llmSettings: expect.objectContaining({
        selections: expect.objectContaining({
          polishReasoningEnabled: true,
          polishTemperature: 0.55,
        }),
      }),
    }));
  });

  it('shows the summary toggle and marks the summary card as off when disabled', async () => {
    currentConfig = {
      ...buildConfig(),
      summaryEnabled: false,
    };

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const summarySwitch = screen.getByRole('switch', { name: 'settings.llm.enable_summary' });
    expect(summarySwitch.getAttribute('aria-checked')).toBe('false');
    expect(screen.getByText('settings.llm.status_off')).toBeDefined();

    await act(async () => {
      fireEvent.click(summarySwitch);
    });

    expect(mockUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({
      summaryEnabled: true,
    }));
  });

  it('shows missing model status when a feature is unassigned', async () => {
    const config = buildConfig();
    config.llmSettings!.selections.translationModelId = undefined;
    currentConfig = config;

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    expect(screen.getAllByText('settings.llm.status_missing_model').length).toBeGreaterThan(0);
  });

  it('surfaces normalized connection errors', async () => {
    const onOpenProviderDetails = vi.fn();
    await act(async () => {
      render(
        <SettingsLLMServiceTab onOpenProviderDetails={onOpenProviderDetails} />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.details' }));
    });

    expect(onOpenProviderDetails).toHaveBeenCalledWith('open_ai');
  });

  it('delegates provider details opening to the settings-level panel stack', async () => {
    const onOpenProviderDetails = vi.fn();
    await act(async () => {
      render(
        <SettingsLLMServiceTab onOpenProviderDetails={onOpenProviderDetails} />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.details' }));
    });

    expect(onOpenProviderDetails).toHaveBeenCalledWith('open_ai');
  });

  it('opens provider details for the requested provider when another provider is expanded', async () => {
    const onOpenProviderDetails = vi.fn();
    currentConfig = {
      ...buildConfig('gemini'),
    };

    await act(async () => {
      render(
        <SettingsLLMServiceTab onOpenProviderDetails={onOpenProviderDetails} />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.details' }));
    });

    expect(onOpenProviderDetails).toHaveBeenCalledWith('gemini');
  });

  it('still renders provider details buttons while provider-level modal content is handled elsewhere', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab onOpenProviderDetails={() => undefined} />,
      );
    });

    expect(screen.getAllByRole('button', { name: 'settings.llm.details' }).length).toBeGreaterThan(0);
  });

  it('passes the active provider through when opening details from the llm settings stack', async () => {
    const onOpenProviderDetails = vi.fn();
    await act(async () => {
      render(
        <SettingsLLMServiceTab onOpenProviderDetails={onOpenProviderDetails} />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.details' }));
    });

    expect(onOpenProviderDetails).toHaveBeenCalledWith(currentConfig.llmSettings!.activeProvider);
  });

  it('keeps provider details on the shared leading slot and only uses provider-specific copy and toolbar classes', async () => {
    let container!: HTMLElement;

    await act(async () => {
      ({ container } = render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={vi.fn()}
          t={(key) => key}
        />,
      ));
    });

    const dialog = screen.getByRole('dialog', { name: 'settings.llm.details' });

    expect(dialog.querySelector('.panel-modal-header-leading .panel-modal-back')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-top-row .panel-modal-badge')?.textContent).toBe('settings.llm.model_library');
    expect(dialog.querySelector('.panel-modal-top-row .panel-modal-badge svg')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-copy .panel-modal-badge')).toBeNull();
    expect(dialog.querySelector('.provider-details-header')).toBeNull();
    expect(dialog.querySelector('.provider-details-header-copy')).toBeTruthy();
    expect(dialog.querySelector('.provider-details-header-copy .provider-details-subtitle')).toBeNull();
    expect(dialog.querySelector('.provider-details-header-controls')).toBeNull();
    expect(dialog.querySelector('.provider-details-toolbar')).toBeTruthy();
    expect(dialog.querySelector('.provider-details-actions')).toBeTruthy();
    expect(dialog.querySelector('.provider-details-add-group')).toBeTruthy();
    expect(dialog.querySelector('.provider-details-refresh')).toBeTruthy();
    expect(dialog.querySelector('.provider-details-toolbar')?.contains(screen.getByRole('button', { name: 'common.close' }))).toBe(false);
    expect(container.querySelector('.panel-modal-header-leading.provider-details-header')).toBeNull();
  });

  it('renders provider model card edit and test actions as tooltip-only icon buttons', async () => {
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      {
        model: 'gpt-4.1',
        displayName: 'GPT-4.1',
        contextWindow: 128000,
        knowledgeCutoff: '2024-06',
        supportsStructuredOutput: true,
      },
    ]);
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={vi.fn()}
          t={(key) => key}
        />,
      );
    });

    const editButton = screen.getByRole('button', { name: 'settings.llm.edit_model_metadata gpt-4.1' });
    const testButton = screen.getByRole('button', { name: 'settings.llm.test_connection gpt-4.1' });

    expect(editButton.classList.contains('btn-icon')).toBe(true);
    expect(editButton.classList.contains('provider-model-edit')).toBe(true);
    expect(editButton.classList.contains('btn-secondary-soft')).toBe(false);
    expect(editButton.getAttribute('data-tooltip')).toBe('settings.llm.edit_model_metadata');
    expect(editButton.getAttribute('data-tooltip-pos')).toBe('top');
    expect(editButton.textContent).toBe('');

    expect(testButton.classList.contains('btn-icon')).toBe(true);
    expect(testButton.classList.contains('provider-model-test')).toBe(true);
    expect(testButton.classList.contains('btn-secondary-soft')).toBe(false);
    expect(testButton.getAttribute('data-tooltip')).toBe('settings.llm.test_connection');
    expect(testButton.getAttribute('data-tooltip-pos')).toBe('top');
    expect(testButton.textContent).toBe('');
    expect(testButton.querySelector('svg')).not.toBeNull();
    expect(screen.getByText('GPT-4.1')).toBeDefined();
    expect(screen.getByLabelText('settings.llm.capability_structured_output')).toBeDefined();
    expect(screen.getByText('settings.llm.model_knowledge_cutoff: 2024-06')).toBeDefined();
  });

  it('only refreshes provider models from details after an explicit refresh click', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1' },
    ], new Date().toISOString());
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };
    let resolveModels!: (models: unknown[]) => void;
    const modelListRequest = new Promise<unknown[]>((resolve) => {
      resolveModels = resolve;
    });
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return modelListRequest;
      }
      return 'OK';
    });

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
    });

    expect(tauriApi.invoke).not.toHaveBeenCalledWith('list_llm_models', expect.anything());

    const refreshButton = screen.getByRole('button', { name: 'settings.llm.refresh_models' }) as HTMLButtonElement;
    expect(refreshButton.disabled).toBe(false);

    await act(async () => {
      fireEvent.click(refreshButton);
    });

    expect(tauriApi.invoke).toHaveBeenCalledTimes(1);
    expect(tauriApi.invoke).toHaveBeenCalledWith('list_llm_models', expect.objectContaining({
      request: expect.objectContaining({
        provider: 'open_ai',
        apiKey: 'test-key',
      }),
    }));
    expect(refreshButton.disabled).toBe(true);
    expect(applyLlmSettings).not.toHaveBeenCalled();

    await act(async () => {
      resolveModels([
        { model: 'gpt-4.1', contextWindow: 128000 },
        { model: 'gpt-4.1-mini', supportsReasoning: true },
      ]);
      await modelListRequest;
    });

    await waitFor(() => {
      expect(refreshButton.disabled).toBe(false);
    });
    expect(applyLlmSettings).toHaveBeenCalledTimes(1);
    expect(tauriApi.invoke).toHaveBeenCalledTimes(1);
  });

  it('auto-refreshes an expired provider model library when details open', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1' },
    ], '2026-05-22T10:00:00.000Z');
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith('list_llm_models', expect.objectContaining({
        request: expect.objectContaining({
          provider: 'open_ai',
          apiKey: 'test-key',
        }),
      }));
    });
    expect(applyLlmSettings).toHaveBeenCalledTimes(1);
  });

  it('keeps fresh provider model library cache local when details open', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1' },
    ], new Date().toISOString());
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
      await Promise.resolve();
    });

    expect(tauriApi.invoke).not.toHaveBeenCalledWith('list_llm_models', expect.anything());
    expect(applyLlmSettings).not.toHaveBeenCalled();
  });

  it('edits discovered provider model metadata from the model card', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      {
        model: 'gpt-4.1',
        contextWindow: 128000,
        inputPrice: 2,
        outputPrice: 8,
        supportsTools: true,
      },
    ]);
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.edit_model_metadata gpt-4.1' }));
    });

    await act(async () => {
      fireEvent.change(screen.getByLabelText('settings.llm.model_context_window'), {
        target: { value: '200000' },
      });
      fireEvent.change(screen.getByLabelText('settings.llm.model_input_price'), {
        target: { value: '1.25' },
      });
      fireEvent.change(screen.getByLabelText('settings.llm.model_output_price'), {
        target: { value: '' },
      });
      fireEvent.click(screen.getByLabelText('settings.llm.model_supports_tools'));
      fireEvent.click(screen.getByLabelText('settings.llm.model_supports_reasoning'));
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.save_model_metadata' }));
    });

    expect(applyLlmSettings).toHaveBeenCalledTimes(1);
    const nextSettings = applyLlmSettings.mock.calls[0][0];
    const modelId = findLlmModelId(nextSettings, 'open_ai', 'gpt-4.1');
    expect(modelId).toBeDefined();
    expect(nextSettings.models[modelId!]).toEqual(expect.objectContaining({
      metadata: expect.objectContaining({
        contextWindow: 200000,
        inputPrice: 1.25,
        outputPrice: undefined,
        supportsTools: false,
        supportsReasoning: true,
      }),
      metadataOverrides: expect.objectContaining({
        contextWindow: true,
        inputPrice: true,
        outputPrice: true,
        supportsTools: true,
        supportsReasoning: true,
      }),
    }));
  });

  it('edits manual provider model metadata while keeping manual-only delete available', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = addLlmModel(llmSettings, {
      provider: 'open_ai',
      model: 'manual-model',
    });
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1' },
    ], new Date().toISOString());
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
    });

    const deleteButton = screen.getByRole('button', { name: 'common.delete manual-model' });
    expect(deleteButton).toBeDefined();
    expect(deleteButton.classList.contains('btn-icon')).toBe(true);
    expect(deleteButton.classList.contains('provider-model-delete')).toBe(true);
    expect(deleteButton.classList.contains('btn-secondary-soft')).toBe(false);
    expect(deleteButton.getAttribute('data-tooltip')).toBe('common.delete');
    expect(deleteButton.getAttribute('data-tooltip-pos')).toBe('top');

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.edit_model_metadata manual-model' }));
    });

    await act(async () => {
      fireEvent.change(screen.getByLabelText('settings.llm.model_max_output_tokens'), {
        target: { value: '8192' },
      });
      fireEvent.click(screen.getByLabelText('settings.llm.model_supports_multimodal'));
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.save_model_metadata' }));
    });

    expect(applyLlmSettings).toHaveBeenCalledTimes(1);
    const nextSettings = applyLlmSettings.mock.calls[0][0];
    const modelId = findLlmModelId(nextSettings, 'open_ai', 'manual-model');
    expect(modelId).toBeDefined();
    expect(nextSettings.models[modelId!]).toEqual(expect.objectContaining({
      source: 'manual',
      metadata: expect.objectContaining({
        maxOutputTokens: 8192,
        supportsMultimodal: true,
      }),
      metadataOverrides: {
        maxOutputTokens: true,
        supportsMultimodal: true,
      },
    }));
  });

  it('cancels provider model metadata edits without applying settings', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1', contextWindow: 128000 },
    ]);
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.edit_model_metadata gpt-4.1' }));
    });

    await act(async () => {
      fireEvent.change(screen.getByLabelText('settings.llm.model_context_window'), {
        target: { value: '200000' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'common.cancel' }));
    });

    expect(applyLlmSettings).not.toHaveBeenCalled();
    expect(screen.queryByLabelText('settings.llm.model_context_window')).toBeNull();
  });

  it('blocks saving provider model metadata when a numeric field is invalid', async () => {
    const applyLlmSettings = vi.fn();
    let llmSettings = currentConfig.llmSettings!;
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1', contextWindow: 128000 },
    ]);
    currentConfig = {
      ...currentConfig,
      llmSettings,
    };

    await act(async () => {
      render(
        <ProviderDetailsModal
          provider="open_ai"
          config={currentConfig}
          isOpen={true}
          origin="settings"
          onBack={vi.fn()}
          onClose={vi.fn()}
          applyLlmSettings={applyLlmSettings}
          t={(key) => key}
        />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.edit_model_metadata gpt-4.1' }));
    });

    await act(async () => {
      fireEvent.change(screen.getByLabelText('settings.llm.model_context_window'), {
        target: { value: '-1' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.save_model_metadata' }));
    });

    expect(applyLlmSettings).not.toHaveBeenCalled();
    expect(screen.getByText('settings.llm.model_metadata_invalid_number')).toBeDefined();
  });

  it('adds a custom provider and expands its credentials panel', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.add_custom_provider' }));
    });

    expect(screen.getByRole('dialog', { name: 'settings.llm.add_custom_provider' })).toBeDefined();

    await act(async () => {
      fireEvent.change(screen.getByLabelText('settings.llm.custom_provider_name'), {
        target: { value: 'Private Gateway' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.api_mode_openai_responses' }));
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'settings.llm.add_custom_provider_confirm' }));
    });

    expect(mockUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({
      llmSettings: expect.objectContaining({
        activeProvider: 'custom-private-gateway',
        customProviders: expect.objectContaining({
          'custom-private-gateway': expect.objectContaining({
            name: 'Private Gateway',
            strategy: 'openai_responses',
          }),
        }),
        providers: expect.objectContaining({
          'custom-private-gateway': expect.objectContaining({
            apiHost: '',
            apiPath: '/v1/responses',
          }),
        }),
      }),
    }));

    currentConfig = {
      ...currentConfig,
      llmSettings: mockUpdateConfig.mock.calls[mockUpdateConfig.mock.calls.length - 1]?.[0].llmSettings,
    };

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    expect(screen.getByText('Private Gateway')).toBeDefined();
    expect(screen.getByTestId('provider-accordion-content-custom-private-gateway')).toBeDefined();
  });

  it('keeps configured custom providers at the bottom of the credentials list', async () => {
    const conf = buildConfig();
    conf.llmSettings!.activeProvider = 'custom-private-gateway';
    conf.llmSettings!.customProviders = {
      'custom-private-gateway': {
        id: 'custom-private-gateway',
        name: 'Private Gateway',
        strategy: 'openai_compatible',
        createdAt: '2026-05-18T00:00:00.000Z',
      },
    };
    conf.llmSettings!.providers['custom-private-gateway'] = {
      apiHost: 'https://gateway.example.com/v1',
      apiKey: 'gateway-key',
      apiPath: '/v1/chat/completions',
    };
    currentConfig = conf;

    let container!: HTMLElement;
    await act(async () => {
      ({ container } = render(
        <SettingsLLMServiceTab />,
      ));
    });

    const credentialsList = container.querySelector('.accordion-container');
    const customProvider = screen.getByText('Private Gateway');
    const lastBuiltInProvider = screen.getByText('ChatGLM');
    const addButton = screen.getByRole('button', { name: 'settings.llm.add_custom_provider' });

    expect(credentialsList?.contains(customProvider)).toBe(true);
    expect(credentialsList?.contains(addButton)).toBe(true);
    expect(
      lastBuiltInProvider.compareDocumentPosition(customProvider) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      customProvider.compareDocumentPosition(addButton) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('offers configured custom providers in feature model selectors', async () => {
    const createdAt = '2026-05-18T00:00:00.000Z';
    const conf = buildConfig();
    conf.llmSettings!.customProviders = {
      'custom-private-gateway': {
        id: 'custom-private-gateway',
        name: 'Private Gateway',
        strategy: 'openai_compatible',
        createdAt,
      },
    };
    conf.llmSettings!.providers['custom-private-gateway'] = {
      apiHost: 'https://gateway.example.com/v1',
      apiKey: 'gateway-key',
      apiPath: '/v1/chat/completions',
    };
    currentConfig = conf;

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    await act(async () => {
      fireEvent.click(screen.getAllByRole('button', { name: /OpenAI/ })[0]);
    });

    expect(screen.getByRole('option', { name: 'Private Gateway' })).toBeDefined();
  });
});
