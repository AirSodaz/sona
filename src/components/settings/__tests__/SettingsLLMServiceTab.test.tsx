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

function buildConfig(provider: LlmProvider = 'open_ai'): AppConfig {
  const baseConfig: AppConfig = {
    streamingModelPath: '/path/to/model',
    offlineModelPath: '',
    language: 'auto',
    appLanguage: 'auto',
    llmSettings: createLlmSettings(provider),
  } as AppConfig;

  let llmSettings = updateProviderSetting(baseConfig.llmSettings, provider, {
    apiKey: 'test-key',
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
    vi.mocked(tauriApi.invoke).mockResolvedValue('OK');
  });

  it('renders active provider fields from llmSettings', () => {
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

  it('adds a model through the manual model pool flow', () => {
    render(
      <SettingsLLMServiceTab
        config={buildConfig()}
        updateConfig={mockUpdateConfig}
        changeLlmServiceType={mockChangeLlmServiceType}
      />,
    );

    const inputs = screen.getAllByRole('textbox');
    fireEvent.change(inputs[inputs.length - 1], { target: { value: 'claude-3-7-sonnet' } });
    fireEvent.click(screen.getByText('settings.llm.add_model'));

    expect(mockUpdateConfig).toHaveBeenCalled();
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
    vi.mocked(tauriApi.invoke).mockRejectedValue(
      'error invoking command `generate_llm_text`: failed to deserialize response body: Caused by: Network Error',
    );

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
