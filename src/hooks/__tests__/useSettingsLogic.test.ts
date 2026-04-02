import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useSettingsLogic } from '../useSettingsLogic';
import { buildLlmConfigPatch, createLlmSettings, updateProviderSetting } from '../../services/llmConfig';

const mockSetConfig = vi.fn();

function createMockConfig() {
    const baseConfig: any = {
        appLanguage: 'auto',
        language: 'auto',
        llmSettings: createLlmSettings(),
    };
    const llmSettings = updateProviderSetting(baseConfig.llmSettings, 'open_ai', {
        apiHost: 'https://api.openai.com',
        apiKey: 'openai-key',
    });

    return {
        ...baseConfig,
        ...buildLlmConfigPatch(llmSettings),
    };
}

let mockConfig: any = createMockConfig();

vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: any) => {
        const state = {
            config: mockConfig,
            setConfig: mockSetConfig,
        };
        return selector(state);
    },
}));

vi.mock('../../stores/dialogStore', () => ({
    useDialogStore: (selector: any) => selector({
        confirm: vi.fn(),
        showError: vi.fn(),
    }),
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
        i18n: { changeLanguage: vi.fn() },
    }),
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false),
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn(),
        deleteModel: vi.fn(),
    },
    PRESET_MODELS: [],
}));

describe('useSettingsLogic', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockConfig = createMockConfig();
    });

    it('exposes changeLlmServiceType', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));
        expect(typeof result.current.changeLlmServiceType).toBe('function');
    });

    it('passes direct config updates through to the store', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        act(() => {
            result.current.updateConfig({
                language: 'en',
            });
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            language: 'en',
        }));
    });

    it('switches active provider without discarding other provider settings', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        act(() => {
            result.current.changeLlmServiceType('azure_openai');
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llmSettings: expect.objectContaining({
                activeProvider: 'azure_openai',
                providers: expect.objectContaining({
                    open_ai: expect.objectContaining({
                        apiHost: 'https://api.openai.com',
                        apiKey: 'openai-key',
                    }),
                    azure_openai: expect.objectContaining({
                        apiHost: '',
                        apiVersion: '2024-10-21',
                    }),
                }),
            }),
        }));
    });

    it('uses the Chatbox Gemini default host when switching providers', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        act(() => {
            result.current.changeLlmServiceType('gemini');
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llmSettings: expect.objectContaining({
                activeProvider: 'gemini',
                providers: expect.objectContaining({
                    gemini: expect.objectContaining({
                        apiHost: 'https://generativelanguage.googleapis.com',
                    }),
                }),
            }),
        }));
    });
});
