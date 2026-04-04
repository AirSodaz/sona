import { useConfigStore } from '../stores/configStore';
import { LlmProvider } from '../types/transcript';
import { buildLlmConfigPatch, ensureLlmState, setActiveProvider } from '../services/llmConfig';

/**
 * Hook for LLM provider configuration.
 * Provides a function to switch the active LLM provider while preserving
 * existing provider settings.
 */
export function useLlmConfig() {
    const config = useConfigStore((state) => state.config);
    const setConfig = useConfigStore((state) => state.setConfig);

    const changeLlmServiceType = (provider: LlmProvider) => {
        const currentLlmState = config.llmSettings ? { llmSettings: config.llmSettings } : ensureLlmState(config as typeof config & Record<string, any>);
        const nextLlmSettings = setActiveProvider(currentLlmState.llmSettings, provider);
        setConfig(buildLlmConfigPatch(nextLlmSettings));
    };

    return { changeLlmServiceType };
}
