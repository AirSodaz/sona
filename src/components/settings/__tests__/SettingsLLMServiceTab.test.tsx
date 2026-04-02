import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
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

  return {
    ...baseConfig,
    ...buildLlmConfigPatch(llmSettings),
  };
}

describe('SettingsLLMServiceTab', () => {
  const mockUpdateConfig = vi.fn();
  const mockChangeLlmServiceType = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return ['gpt-4o', 'gpt-4.1-mini'];
      }
      return 'OK';
    });
  });

  it('renders active provider fields from llmSettings', async () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig()}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    expect(screen.getByText('settings.llm.service_type')).toBeDefined();
    expect(screen.getAllByText('OpenAI').length).toBeGreaterThan(0);
    expect(screen.getByDisplayValue('https://api.openai.com')).toBeDefined();
    expect(screen.getByDisplayValue('test-key')).toBeDefined();
    expect(screen.getAllByText('OpenAI / gpt-4o').length).toBeGreaterThan(0);

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith('list_llm_models', {
        request: {
          provider: 'open_ai',
          baseUrl: 'https://api.openai.com',
          apiKey: 'test-key',
        },
      });
    });
  });

  it('shows simple provider readiness status', async () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig('open_ai', false)}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    await waitFor(() => {
      expect(screen.getAllByText('settings.llm.status_missing_api_key').length).toBeGreaterThan(0);
    });
  });

  it('fills Gemini host with the Chatbox default host', () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig('gemini')}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    expect(screen.getByDisplayValue('https://generativelanguage.googleapis.com')).toBeDefined();
  });

  it('renders Azure-specific labels and version field', () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig('azure_openai')}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    expect(screen.getByText('Endpoint')).toBeDefined();
    expect(screen.getByText('settings.llm.api_version')).toBeDefined();
    expect(screen.getByDisplayValue('2024-10-21')).toBeDefined();
    expect(screen.getAllByText('Azure OpenAI / deployment-1').length).toBeGreaterThan(0);
  });

  it('shows candidates only while the model input is focused', async () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig()}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    const modelInput = screen.getByRole('combobox');

    expect(screen.queryByRole('listbox')).toBeNull();

    fireEvent.focus(modelInput);

    await waitFor(() => {
      expect(screen.getByRole('listbox')).toBeDefined();
      expect(screen.getByRole('option', { name: 'gpt-4.1-mini' })).toBeDefined();
    });

    fireEvent.blur(modelInput);

    await waitFor(() => {
      expect(screen.queryByRole('listbox')).toBeNull();
    });
  });

  it('adds a model through the searchable model input flow', async () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig()}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    const modelInput = screen.getByRole('combobox');
    fireEvent.focus(modelInput);
    fireEvent.change(modelInput, { target: { value: 'gpt-4.1' } });

    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'gpt-4.1-mini' })).toBeDefined();
    });

    fireEvent.keyDown(modelInput, { key: 'ArrowDown' });
    fireEvent.keyDown(modelInput, { key: 'Enter' });
    fireEvent.click(screen.getByText('settings.llm.add_model'));

    expect(mockUpdateConfig).toHaveBeenCalled();
  });

  it('shows missing model status when a feature is unassigned', () => {
    const config = buildConfig();
    if (config.llmSettings) {
      config.llmSettings = {
        ...config.llmSettings,
        selections: {
          ...config.llmSettings.selections,
          translationModelId: undefined,
        },
      };
    }

    render(
      <SettingsLLMServiceTab
        config={config}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    expect(screen.getByText('settings.llm.status_missing_model')).toBeDefined();
  });

  it('tests a configured model entry', async () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig()}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    fireEvent.click(screen.getAllByText('settings.llm.test_connection')[0]);

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith('generate_llm_text', {
        request: {
          config: {
            provider: 'open_ai',
            baseUrl: 'https://api.openai.com',
            apiKey: 'test-key',
            model: 'gpt-4o',
            apiPath: undefined,
            apiVersion: undefined,
            temperature: 0.7,
          },
          input: 'Hello, this is a connection test.',
        },
      });
    });
  });

  it('surfaces normalized connection errors', async () => {
    vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
      if (command === 'list_llm_models') {
        return ['gpt-4o'];
      }
      throw 'error invoking command `generate_llm_text`: failed to deserialize response body: Caused by: Network Error';
    });

    render(
      <SettingsLLMServiceTab
        config={buildConfig()}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    fireEvent.click(screen.getAllByText('settings.llm.test_connection')[0]);

    await waitFor(() => {
      expect(screen.getByText('settings.llm.connection_failed')).toBeDefined();
      expect(screen.getByText('Network Error')).toBeDefined();
    });
  });
});
