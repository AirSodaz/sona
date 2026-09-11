import { ensureLlmState } from '../services/llm/migration';
import { buildLlmConfigPatch, setActiveProvider } from '../services/llm/state';
import { useConfigStore } from '../stores/configStore';
import type { LlmProvider } from '../types/transcript';

/**
 * Hook for LLM provider configuration.
 * Provides a function to switch the active LLM provider while preserving
 * existing provider settings.
 */
export function useLlmConfig() {
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);

  const changeLlmServiceType = (provider: LlmProvider) => {
    const currentLlmState = config.llmSettings
      ? { llmSettings: config.llmSettings }
      : ensureLlmState(config);
    const nextLlmSettings = setActiveProvider(currentLlmState.llmSettings, provider);
    setConfig(buildLlmConfigPatch(nextLlmSettings));
  };

  return { changeLlmServiceType };
}
