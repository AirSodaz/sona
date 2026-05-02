import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useSettingsLogic, type SettingsTabInput } from '../useSettingsLogic';
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

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
        i18n: { changeLanguage: vi.fn() },
    }),
}));

describe('useSettingsLogic', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockConfig = createMockConfig();
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

    it('returns activeTab and setActiveTab', () => {
        const { result } = renderHook(() => useSettingsLogic(false, vi.fn()));

        expect(result.current.activeTab).toBe('general');
        expect(typeof result.current.setActiveTab).toBe('function');
    });

    it('sets initial tab when opened', () => {
        const { result } = renderHook(() => useSettingsLogic(true, vi.fn(), 'models'));

        expect(result.current.activeTab).toBe('models');
    });

    it('maps the legacy context tab to vocabulary when opened', () => {
        const { result } = renderHook(() => useSettingsLogic(true, vi.fn(), 'context'));

        expect(result.current.activeTab).toBe('vocabulary');
    });

    it('derives the opened initial tab before the sync effect commits state', () => {
        const initialProps: { isOpen: boolean; initialTab?: SettingsTabInput } = {
            isOpen: false,
            initialTab: undefined,
        };
        const { result, rerender } = renderHook(
            ({ isOpen, initialTab }: { isOpen: boolean; initialTab?: SettingsTabInput }) => {
                return useSettingsLogic(isOpen, vi.fn(), initialTab);
            },
            { initialProps },
        );

        rerender({ isOpen: true, initialTab: 'models' });

        expect(result.current.activeTab).toBe('models');
    });
});
