import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SettingsLLMServiceTab } from '../SettingsLLMServiceTab';
import * as tauriApi from '@tauri-apps/api/core';
import { AppConfig, LlmConfig } from '../../../types/transcript';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

describe('SettingsLLMServiceTab', () => {
    const mockUpdateConfig = vi.fn();
    const mockChangeLlmServiceType = vi.fn();

    const mockConfig: AppConfig = {
        streamingModelPath: '/path/to/model',
        offlineModelPath: '',
        language: 'auto',
        appLanguage: 'auto',
        llm: {
            provider: 'open_ai',
            baseUrl: 'https://api.openai.com/v1',
            apiKey: 'test-key',
            model: 'gpt-4o',
            temperature: 0.7
        }
    };

    const defaultProps = {
        config: mockConfig,
        updateConfig: mockUpdateConfig,
        changeLlmServiceType: mockChangeLlmServiceType
    };

    const geminiConfig: LlmConfig = {
        provider: 'gemini',
        baseUrl: '',
        apiKey: mockConfig.llm?.apiKey ?? '',
        model: mockConfig.llm?.model ?? '',
        temperature: mockConfig.llm?.temperature,
    };

    it('renders all fields with correct localization keys', () => {
        render(<SettingsLLMServiceTab {...defaultProps} />);

        expect(screen.getByText('settings.llm.service_type')).toBeDefined();
        // Dropdown value (selected option label)
        expect(screen.getByText('OpenAI')).toBeDefined();

        expect(screen.getByText('settings.llm.base_url')).toBeDefined();
        expect(screen.getByDisplayValue('https://api.openai.com/v1')).toBeDefined();

        expect(screen.getByText('settings.llm.api_key')).toBeDefined();
        expect(screen.getByDisplayValue('test-key')).toBeDefined();

        expect(screen.getByText('settings.llm.model_name')).toBeDefined();
        expect(screen.getByDisplayValue('gpt-4o')).toBeDefined();

        expect(screen.getByText('settings.llm.test_connection')).toBeDefined();
    });

    it('renders Gemini base URL placeholder for Gemini provider', () => {
        render(
            <SettingsLLMServiceTab
                {...defaultProps}
                config={{
                    ...mockConfig,
                    llm: geminiConfig
                }}
            />
        );

        expect(screen.getByPlaceholderText('https://generativelanguage.googleapis.com')).toBeDefined();
    });

    it('calls invoke when Test Connection is clicked', async () => {
        vi.mocked(tauriApi.invoke).mockResolvedValue('OK');

        render(<SettingsLLMServiceTab {...defaultProps} />);

        const testBtn = screen.getByText('settings.llm.test_connection');
        fireEvent.click(testBtn);

        expect(tauriApi.invoke).toHaveBeenCalledWith('generate_llm_text', {
            request: {
                config: {
                    provider: 'open_ai',
                    baseUrl: 'https://api.openai.com/v1',
                    apiKey: 'test-key',
                    model: 'gpt-4o',
                    temperature: 0.7
                },
                input: 'Hello, this is a connection test.'
            }
        });

        await waitFor(() => {
            expect(screen.getByText('settings.llm.connection_success')).toBeDefined();
            expect(screen.getByText('OK')).toBeDefined();
        });
    });

    it('displays normalized error message when connection fails', async () => {
        vi.mocked(tauriApi.invoke).mockRejectedValue('error invoking command `generate_llm_text`: failed to deserialize response body: Caused by: Network Error');

        render(<SettingsLLMServiceTab {...defaultProps} />);

        const testBtn = screen.getByText('settings.llm.test_connection');
        fireEvent.click(testBtn);

        await waitFor(() => {
            expect(screen.getByText('settings.llm.connection_failed')).toBeDefined();
            expect(screen.getByText('Network Error')).toBeDefined();
        });
    });
});
