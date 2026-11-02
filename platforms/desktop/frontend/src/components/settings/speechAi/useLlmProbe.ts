import { useEffect, useState } from 'react';
import { buildLlmConfig, getProviderDefinition } from '../../../services/llm/providers';
import { generateLlmText } from '../../../services/tauri/llm';
import { useLlmAssistantConfig } from '../../../stores/configStore';
import type { LlmProvider } from '../../../types/transcript';
import { getCurrentLlmSettings } from '../llm/helpers';
import { probeIsFresh, readLlmProbe, writeLlmProbe } from './llmProbe';

export function useLlmProbe(
  provider: LlmProvider | null | undefined,
  model: string | null | undefined,
  enabled = true
): { busy: boolean; ok: boolean; error?: string; retry: () => void } {
  const llmConfig = useLlmAssistantConfig();
  const [attempt, setAttempt] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!enabled || !provider || !model) return;
    if (provider === 'google_translate' || provider === 'google_translate_free') return;
    const existing = readLlmProbe(provider);
    if (
      attempt === 0 &&
      existing &&
      existing.provider === provider &&
      existing.model === model &&
      probeIsFresh(existing)
    ) {
      return;
    }
    const current = getCurrentLlmSettings(llmConfig);
    const setting = current.providers[provider];
    if (!setting) return;
    const definition = getProviderDefinition(provider, current.customProviders);
    if (definition.requiresApiKey && !setting.apiKey) return;
    let cancelled = false;
    setBusy(true);
    const started = Date.now();
    generateLlmText({
      config: { ...buildLlmConfig(provider, setting, current.customProviders), model },
      input: 'Hi',
      source: 'connection_test',
    })
      .then(() => {
        if (cancelled) return;
        writeLlmProbe({
          provider,
          model,
          ok: true,
          at: Date.now(),
          latencyMs: Date.now() - started,
        });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        writeLlmProbe({
          provider,
          model,
          ok: false,
          at: Date.now(),
          error: error instanceof Error ? error.message : String(error),
        });
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [attempt, enabled, llmConfig, model, provider]);

  const record = provider ? readLlmProbe(provider) : null;
  const matched = record && record.model === model ? record : null;
  return {
    busy,
    ok: Boolean(matched?.ok),
    error: matched && !matched.ok ? matched.error : undefined,
    retry: () => setAttempt((value) => value + 1),
  };
}
