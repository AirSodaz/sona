import { renderHook, act } from '@testing-library/react';
import { useSettingsLogic } from '../useSettingsLogic';
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock stores
const mockSetConfig = vi.fn();
let mockConfig: any = {
    llm: {
        provider: 'open_ai',
        baseUrl: 'https://api.openai.com/v1',
        apiKey: '',
        model: '',
        temperature: 0.7
    }
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
    useDialogStore: (selector: any) => selector({
        confirm: vi.fn(),
        showError: vi.fn()
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
            llm: {
                provider: 'open_ai',
                baseUrl: 'https://api.openai.com/v1',
                apiKey: '',
                model: '',
                temperature: 0.7
            }
        };
    });

    it('should expose changeLlmServiceType for switching services', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        expect(typeof result.current.changeLlmServiceType).toBe('function');
    });

    it('should update llm config directly', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        const newUrl = 'https://custom.openai.com';

        act(() => {
            result.current.updateConfig({
                llm: {
                    ...mockConfig.llm,
                    baseUrl: newUrl
                }
            });
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llm: expect.objectContaining({ baseUrl: newUrl })
        }));
    });

    it('should replace llm config when switching provider', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        act(() => {
            result.current.changeLlmServiceType('anthropic');
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llm: expect.objectContaining({
                provider: 'anthropic',
                baseUrl: 'https://api.anthropic.com',
                apiKey: '',
                model: ''
            })
        }));
    });

    it('should reset Gemini to the default base URL when switching provider', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        act(() => {
            result.current.changeLlmServiceType('gemini');
        });

        expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
            llm: expect.objectContaining({
                provider: 'gemini',
                baseUrl: 'https://generativelanguage.googleapis.com',
                apiKey: '',
                model: ''
            })
        }));
    });
});
