import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useLlmConfig } from '../useLlmConfig';
import { buildLlmConfigPatch, createLlmSettings, updateProviderSetting } from '../../services/llm/state';

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

vi.mock('../../stores/configStore', () => ({
    useConfigStore: (selector: any) => {
        const state = {
            config: mockConfig,
            setConfig: mockSetConfig,
        };
        return selector(state);
    },
}));

describe('useLlmConfig', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockConfig = createMockConfig();
    });

    it('exposes changeLlmServiceType', () => {
        const { result } = renderHook(() => useLlmConfig());
        expect(typeof result.current.changeLlmServiceType).toBe('function');
    });

    it('switches active provider without discarding other provider settings', () => {
        const { result } = renderHook(() => useLlmConfig());

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

    it('uses the correct Gemini default host when switching providers', () => {
        const { result } = renderHook(() => useLlmConfig());

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
