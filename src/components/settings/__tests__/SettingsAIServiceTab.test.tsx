import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SettingsAIServiceTab } from '../SettingsAIServiceTab';
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

describe('SettingsAIServiceTab', () => {
    const mockUpdateConfig = vi.fn();
    const mockChangeAiServiceType = vi.fn();

    const mockConfig: AppConfig = {
        recognitionModelPath: '',
        language: 'auto',
        appLanguage: 'auto',
        aiServiceType: 'openai',
        aiBaseUrl: 'https://api.openai.com/v1',
        aiApiKey: 'test-key',
        aiModel: 'gpt-4o',
        aiServices: {}
    };

    const defaultProps = {
        config: mockConfig,
        updateConfig: mockUpdateConfig,
        changeAiServiceType: mockChangeAiServiceType
    };

    it('renders all fields with correct localization keys', () => {
        render(<SettingsAIServiceTab {...defaultProps} />);

        expect(screen.getByText('settings.ai.service_type')).toBeDefined();
        // Dropdown value (selected option label)
        expect(screen.getByText('OpenAI')).toBeDefined();

        expect(screen.getByText('settings.ai.base_url')).toBeDefined();
        expect(screen.getByDisplayValue('https://api.openai.com/v1')).toBeDefined();

        expect(screen.getByText('settings.ai.api_key')).toBeDefined();
        expect(screen.getByDisplayValue('test-key')).toBeDefined();

        expect(screen.getByText('settings.ai.model_name')).toBeDefined();
        expect(screen.getByDisplayValue('gpt-4o')).toBeDefined();

        expect(screen.getByText('settings.ai.test_connection')).toBeDefined();
    });

    it('updates Service Type when changed', async () => {
        render(<SettingsAIServiceTab {...defaultProps} />);

        // Open Dropdown
        const trigger = screen.getByText('OpenAI');
        fireEvent.click(trigger);

        // Find Anthropic option
        const anthropicOption = screen.getByText('Anthropic');
        fireEvent.click(anthropicOption);

        expect(mockChangeAiServiceType).toHaveBeenCalledWith('anthropic');
    });

    it('calls invoke when Test Connection is clicked', async () => {
        vi.mocked(tauriApi.invoke).mockResolvedValue('OK');

        render(<SettingsAIServiceTab {...defaultProps} />);

        const testBtn = screen.getByText('settings.ai.test_connection');
        fireEvent.click(testBtn);

        expect(tauriApi.invoke).toHaveBeenCalledWith('call_ai_model', {
            apiKey: 'test-key',
            baseUrl: 'https://api.openai.com/v1',
            modelName: 'gpt-4o',
            input: 'Hello, this is a connection test.',
            apiFormat: 'openai'
        });

        await waitFor(() => {
            expect(screen.getByText('settings.ai.connection_success')).toBeDefined();
            expect(screen.getByText('OK')).toBeDefined();
        });
    });

    it('displays error message when connection fails', async () => {
        vi.mocked(tauriApi.invoke).mockRejectedValue('Network Error');

        render(<SettingsAIServiceTab {...defaultProps} />);

        const testBtn = screen.getByText('settings.ai.test_connection');
        fireEvent.click(testBtn);

        await waitFor(() => {
            expect(screen.getByText('settings.ai.connection_failed')).toBeDefined();
            expect(screen.getByText('Network Error')).toBeDefined();
        });
    });
});
