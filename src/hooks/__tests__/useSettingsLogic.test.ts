import { renderHook, act } from '@testing-library/react';
import { useSettingsLogic } from '../useSettingsLogic';
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock stores
const mockSetConfig = vi.fn();
let mockConfig: any = {
    llmServiceType: 'openai',
    llmBaseUrl: 'https://api.openai.com/v1',
    llmApiKey: '',
    llmModel: '',
    llmServices: {}
};

vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: any) => {
        const state = {
            config: mockConfig,
            setConfig: mockSetConfig
        };
        return selector(state);
    }
}));

vi.mock('../../stores/dialogStore', () => ({
    useDialogStore: () => ({
        confirm: vi.fn(),
        alert: vi.fn()
    })
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
        i18n: { changeLanguage: vi.fn() }
    })
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false),
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn(),
        deleteModel: vi.fn()
    },
    PRESET_MODELS: []
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn()
}));

describe('useSettingsLogic', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockConfig = {
            llmServiceType: 'openai',
            llmBaseUrl: 'https://api.openai.com/v1',
            llmApiKey: '',
            llmModel: '',
            llmServices: {}
        };
    });

    it('should expose changeLlmServiceType for switching services', () => {
        const { result } = renderHook(() => useSettingsLogic(true, vi.fn()));

        expect(typeof result.current.changeLlmServiceType).toBe('function');
    });

    it('should update LLM Base URL and sync to llmServices', () => {
        const { result } = renderHook(() => useSettingsLogic(true, vi.fn()));

        const newUrl = 'https://custom.openai.com';

        act(() => {
            result.current.updateConfig({
                llmBaseUrl: newUrl,
                llmServices: {
                    ...mockConfig.llmServices,
                    openai: {
                        ...mockConfig.llmServices.openai,
                        baseUrl: newUrl
                    }
                }
            });
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llmBaseUrl: newUrl
        }));
    });

    it('should load saved settings when switching back to a service', () => {
        // Setup: config has saved settings for anthropic
        mockConfig.llmServices = {
            anthropic: {
                baseUrl: 'https://custom.anthropic.com',
                apiKey: 'sk-ant-test',
                model: 'claude-3'
            }
        };

        const { result } = renderHook(() => useSettingsLogic(true, vi.fn()));

        act(() => {
            result.current.changeLlmServiceType('anthropic');
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llmServiceType: 'anthropic',
            llmBaseUrl: 'https://custom.anthropic.com',
            llmApiKey: 'sk-ant-test',
            llmModel: 'claude-3'
        }));
    });
});
