import { expect, vi, beforeEach, describe, it } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import * as tauriApi from '@tauri-apps/api/core';
import { SettingsLLMServiceTab } from '../SettingsLLMServiceTab';
import { AppConfig, LlmProvider } from '../../../types/transcript';
import {
  addLlmModel,
  buildLlmConfigPatch,
  createLlmSettings,
  setFeatureModelSelection,
  updateProviderSetting,
} from '../../../services/llmConfig';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

function buildConfig(provider: LlmProvider = 'open_ai', includeApiKey = true): AppConfig {
  const baseConfig: AppConfig = {
    streamingModelPath: '/path/to/model',
    offlineModelPath: '',
    language: 'auto',
    appLanguage: 'auto',
    summaryEnabled: true,
    llmSettings: createLlmSettings(provider),
  } as AppConfig;

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

vi.mock('../../../stores/configStore', () => ({
  useLlmAssistantConfig: () => currentConfig,
  useSetConfig: () => mockUpdateConfig,
}));

describe('SettingsLLMServiceTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    currentConfig = buildConfig();
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return ['gpt-4o', 'gpt-4.1-mini'];
      }
      return 'OK';
    });
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
    let conf = buildConfig();
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
    let conf = buildConfig('gemini');
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

    expect(screen.queryByText('gpt-4.1-mini')).toBeNull();

    await act(async () => {
      fireEvent.focus(modelInput);
      fireEvent.change(modelInput, { target: { value: '' } });
    });

    await waitFor(() => {
      expect(screen.getByText('gpt-4.1-mini')).toBeDefined();
    });

    await act(async () => {
      fireEvent.blur(modelInput);
    });

    await waitFor(() => {
      expect(screen.queryByText('gpt-4.1-mini')).toBeNull();
    });
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
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return ['gpt-4o'];
      }
      throw 'error invoking command `generate_llm_text`: failed to deserialize response body: Caused by: Network Error';
    });

    await act(async () => {
      render(
        <SettingsLLMServiceTab />,
      );
    });

    const testButtons = screen.getAllByText('settings.llm.test_connection');
    await act(async () => {
      fireEvent.click(testButtons[0]);
    });

    await waitFor(() => {
      expect(screen.getByText('settings.llm.connection_failed')).toBeDefined();
      expect(screen.getByText('Network Error')).toBeDefined();
    });
  });
});
