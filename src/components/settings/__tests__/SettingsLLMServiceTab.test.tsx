import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import * as tauriApi from '@tauri-apps/api/core';
import { SettingsLLMServiceTab } from '../SettingsLLMServiceTab';
import { AppConfig, LlmProvider } from '../../../types/transcript';
import { buildLlmConfigPatch, createLlmSettings, updateProviderSetting } from '../../../services/llmConfig';

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
    };

    const llmSettings = updateProviderSetting(baseConfig.llmSettings, provider, {
        apiKey: 'test-key',
        model: provider === 'azure_openai' ? 'deployment-1' : 'gpt-4o',
    });

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
        vi.mocked(tauriApi.invoke).mockResolvedValue([]);
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
        expect(screen.getByText('OpenAI')).toBeDefined();
        expect(screen.getByDisplayValue('https://api.openai.com')).toBeDefined();
        expect(screen.getByDisplayValue('test-key')).toBeDefined();
        expect(screen.getByDisplayValue('gpt-4o')).toBeDefined();
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
        expect(screen.getByText('Deployment Name')).toBeDefined();
        expect(screen.getByText('settings.llm.api_version')).toBeDefined();
        expect(screen.getByDisplayValue('2024-10-21')).toBeDefined();
    });

    it('calls invoke when test connection is clicked', async () => {
        vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
            if (command === 'generate_llm_text') return 'OK';
            return [];
        });

        render(
            <SettingsLLMServiceTab
                config={buildConfig()}
                updateConfig={mockUpdateConfig}
                changeLlmServiceType={mockChangeLlmServiceType}
            />,
        );

        fireEvent.click(screen.getByText('settings.llm.test_connection'));

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

        await waitFor(() => {
            expect(screen.getByText('settings.llm.connection_success')).toBeDefined();
            expect(screen.getByText('OK')).toBeDefined();
        });
    });

    it('surfaces normalized connection errors', async () => {
        vi.mocked(tauriApi.invoke).mockImplementation(async (command) => {
            if (command === 'generate_llm_text') {
                throw 'error invoking command `generate_llm_text`: failed to deserialize response body: Caused by: Network Error';
            }
            return [];
        });

        render(
            <SettingsLLMServiceTab
                config={buildConfig()}
                updateConfig={mockUpdateConfig}
                changeLlmServiceType={mockChangeLlmServiceType}
            />,
        );

        fireEvent.click(screen.getByText('settings.llm.test_connection'));

        await waitFor(() => {
            expect(screen.getByText('settings.llm.connection_failed')).toBeDefined();
            expect(screen.getByText('Network Error')).toBeDefined();
        });
    });
});
