import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SettingsLLMServiceTab } from '../SettingsLLMServiceTab';
import * as tauriApi from '@tauri-apps/api/core';
import { AppConfig } from '../../../types/transcript';

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

    it('updates Service Type when changed', async () => {
        render(<SettingsLLMServiceTab {...defaultProps} />);

        // Open Dropdown
        const trigger = screen.getByText('OpenAI');
        fireEvent.click(trigger);

        // Find Anthropic option
        const anthropicOption = screen.getByText('Anthropic');
        fireEvent.click(anthropicOption);

        expect(mockChangeLlmServiceType).toHaveBeenCalledWith('anthropic');
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

    it('displays error message when connection fails', async () => {
        vi.mocked(tauriApi.invoke).mockRejectedValue('Network Error');

        render(<SettingsLLMServiceTab {...defaultProps} />);

        const testBtn = screen.getByText('settings.llm.test_connection');
        fireEvent.click(testBtn);

        await waitFor(() => {
            expect(screen.getByText('settings.llm.connection_failed')).toBeDefined();
            expect(screen.getByText('Network Error')).toBeDefined();
        });
    });
});
