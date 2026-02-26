import { renderHook, act } from '@testing-library/react';
import { useSettingsLogic } from '../useSettingsLogic';
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock stores
const mockSetConfig = vi.fn();
let mockConfig: any = {
    aiServiceType: 'openai',
    aiBaseUrl: 'https://api.openai.com/v1',
    aiApiKey: '',
    aiModel: '',
    aiServices: {}
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
            aiServiceType: 'openai',
            aiBaseUrl: 'https://api.openai.com/v1',
            aiApiKey: '',
            aiModel: '',
            aiServices: {}
        };
    });

    it('should update AI service type and sync settings', () => {
        const { result } = renderHook(() => useSettingsLogic(true, vi.fn()));

        // Act: Change to Anthropic
        act(() => {
            result.current.changeAiServiceType('anthropic');
        });

        // Assert: config should be updated
        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            aiServiceType: 'anthropic',
            // Default URL for Anthropic
            aiBaseUrl: 'https://api.anthropic.com',
            // Saved settings for OpenAI should be in aiServices
            aiServices: expect.objectContaining({
                openai: expect.objectContaining({
                    baseUrl: 'https://api.openai.com/v1'
                })
            })
        }));
    });

    it('should update AI Base URL and sync to aiServices', () => {
        const { result } = renderHook(() => useSettingsLogic(true, vi.fn()));

        const newUrl = 'https://custom.openai.com';

        act(() => {
            result.current.updateConfig({
                aiBaseUrl: newUrl,
                aiServices: {
                    ...mockConfig.aiServices,
                    openai: {
                        ...mockConfig.aiServices.openai,
                        baseUrl: newUrl
                    }
                }
            });
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            aiBaseUrl: newUrl
        }));
    });

    it('should load saved settings when switching back to a service', () => {
        // Setup: config has saved settings for anthropic
        mockConfig.aiServices = {
            anthropic: {
                baseUrl: 'https://custom.anthropic.com',
                apiKey: 'sk-ant-test',
                model: 'claude-3'
            }
        };

        const { result } = renderHook(() => useSettingsLogic(true, vi.fn()));

        act(() => {
            result.current.changeAiServiceType('anthropic');
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            aiServiceType: 'anthropic',
            aiBaseUrl: 'https://custom.anthropic.com',
            aiApiKey: 'sk-ant-test',
            aiModel: 'claude-3'
        }));
    });
});
