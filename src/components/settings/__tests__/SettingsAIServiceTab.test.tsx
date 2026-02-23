import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SettingsAIServiceTab } from '../SettingsAIServiceTab';
import * as tauriApi from '@tauri-apps/api/core';

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
    const defaultProps = {
        aiServiceType: 'openai',
        setAiServiceType: vi.fn(),
        aiBaseUrl: 'https://api.openai.com/v1',
        setAiBaseUrl: vi.fn(),
        aiApiKey: 'test-key',
        setAiApiKey: vi.fn(),
        aiModel: 'gpt-4o',
        setAiModel: vi.fn(),
    };

    it('renders all fields with correct localization keys', () => {
        render(<SettingsAIServiceTab {...defaultProps} />);

        expect(screen.getByText('settings.ai.service_type')).toBeDefined();
        // Dropdown value (selected option label)
        // Since t returns key, the label is settings.ai.services.openai
        expect(screen.getByText('settings.ai.services.openai')).toBeDefined();

        expect(screen.getByText('settings.ai.base_url')).toBeDefined();
        expect(screen.getByDisplayValue('https://api.openai.com/v1')).toBeDefined();

        expect(screen.getByText('settings.ai.api_key')).toBeDefined();
        expect(screen.getByDisplayValue('test-key')).toBeDefined();

        expect(screen.getByText('settings.ai.model_name')).toBeDefined();
        expect(screen.getByDisplayValue('gpt-4o')).toBeDefined();

        expect(screen.getByText('settings.ai.test_connection')).toBeDefined();
    });

    it('updates Base URL when Service Type changes', async () => {
        const setAiServiceType = vi.fn();
        const setAiBaseUrl = vi.fn();

        render(
            <SettingsAIServiceTab
                {...defaultProps}
                aiServiceType="openai"
                setAiServiceType={setAiServiceType}
                setAiBaseUrl={setAiBaseUrl}
            />
        );

        // Open Dropdown
        const trigger = screen.getByText('settings.ai.services.openai');
        fireEvent.click(trigger);

        // Find Anthropic option
        const anthropicOption = screen.getByText('settings.ai.services.anthropic');
        fireEvent.click(anthropicOption);

        expect(setAiServiceType).toHaveBeenCalledWith('anthropic');
        expect(setAiBaseUrl).toHaveBeenCalledWith('https://api.anthropic.com');
    });

    it('updates Base URL for Ollama', async () => {
        const setAiServiceType = vi.fn();
        const setAiBaseUrl = vi.fn();

        render(
            <SettingsAIServiceTab
                {...defaultProps}
                aiServiceType="openai"
                setAiServiceType={setAiServiceType}
                setAiBaseUrl={setAiBaseUrl}
            />
        );

        // Open Dropdown
        const trigger = screen.getByText('settings.ai.services.openai');
        fireEvent.click(trigger);

        // Find Ollama option
        const ollamaOption = screen.getByText('settings.ai.services.ollama');
        fireEvent.click(ollamaOption);

        expect(setAiServiceType).toHaveBeenCalledWith('ollama');
        expect(setAiBaseUrl).toHaveBeenCalledWith('http://localhost:11434/v1');
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
            expect(screen.getByText('settings.ai.connection_successOK')).toBeDefined();
        });
    });

    it('displays error message when connection fails', async () => {
        vi.mocked(tauriApi.invoke).mockRejectedValue('Network Error');

        render(<SettingsAIServiceTab {...defaultProps} />);

        const testBtn = screen.getByText('settings.ai.test_connection');
        fireEvent.click(testBtn);

        await waitFor(() => {
            expect(screen.getByText('settings.ai.connection_failedNetwork Error')).toBeDefined();
        });
    });
});
