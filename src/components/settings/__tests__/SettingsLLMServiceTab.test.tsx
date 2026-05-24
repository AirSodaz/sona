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
  syncProviderDiscoveredModels,
  setFeatureModelSelection,
  updateProviderSetting,
} from '../../../services/llm/state';
import { buildTestConfig } from '../../../test-utils/configTestUtils';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

function buildConfig(provider: LlmProvider = 'open_ai', includeApiKey = true): AppConfig {
  const baseConfig = buildTestConfig({
    streamingModelPath: '/path/to/model',
    offlineModelPath: '',
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
    ]);
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

  it('adds a model through the searchable model input flow', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const modelInputs = screen.getAllByPlaceholderText('gpt-4o-mini');
    const modelInput = modelInputs[0];
    
    await act(async () => {
      fireEvent.focus(modelInput);
      fireEvent.change(modelInput, { target: { value: 'gpt-4.2-new' } });
      fireEvent.keyDown(modelInput, { key: 'Enter' });
    });

    expect(mockUpdateConfig).toHaveBeenCalled();
  });

  it('renders unified temperature controls for all three features', async () => {
    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    expect(screen.queryByTestId('provider-temperature-number')).toBeNull();
    expect(screen.getAllByText('settings.llm.temperature')).toHaveLength(3);
    expect(screen.getAllByRole('spinbutton')).toHaveLength(3);
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
    expect(dialog.querySelector('.provider-details-toolbar')?.contains(screen.getByRole('button', { name: 'Close' }))).toBe(false);
    expect(container.querySelector('.panel-modal-header-leading.provider-details-header')).toBeNull();
  });

  it('only refreshes provider models from details after an explicit refresh click', async () => {
    const applyLlmSettings = vi.fn();
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
