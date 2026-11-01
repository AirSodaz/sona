import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { buildLlmConfig, getProviderDefinition } from '../../../services/llm/providers';
import {
  addCustomProvider,
  addLlmModel,
  buildLlmConfigPatch,
  setActiveProvider,
  setFeatureModelSelection,
  updateProviderSetting,
} from '../../../services/llm/state';
import { generateLlmText, listLlmModels } from '../../../services/tauri/llm';
import { useLlmAssistantConfig, useSetConfig } from '../../../stores/configStore';
import type { LlmProvider } from '../../../types/transcript';
import { ProviderLogo } from '../../icons/ProviderLogos';
import { getCurrentLlmSettings, getModelPlaceholder } from '../llm/helpers';
import { writeLlmProbe } from './llmProbe';

const GRID: Array<{ id: LlmProvider; local?: boolean }> = [
  { id: 'open_ai' },
  { id: 'anthropic' },
  { id: 'gemini' },
  { id: 'deep_seek' },
  { id: 'qwen' },
  { id: 'silicon_flow' },
  { id: 'ollama', local: true },
  { id: 'lm_studio', local: true },
];

const PRESETS: Partial<Record<string, string[]>> = {
  open_ai: ['gpt-4o-mini', 'gpt-4o', 'gpt-4.1-mini'],
  anthropic: ['claude-sonnet-4-20250514', 'claude-3-5-haiku-latest'],
  gemini: ['gemini-2.5-flash', 'gemini-2.5-pro'],
  deep_seek: ['deepseek-chat', 'deepseek-reasoner'],
  qwen: ['qwen-plus', 'qwen-turbo', 'qwen-max'],
  silicon_flow: ['Qwen/Qwen2.5-7B-Instruct'],
};

export function ConnectAiDialog({ onClose }: { onClose: () => void }): React.JSX.Element {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const [provider, setProvider] = useState<LlmProvider | 'custom' | null>('open_ai');
  const [step, setStep] = useState<'choose' | 'details'>('choose');
  const [apiKey, setApiKey] = useState('');
  const [model, setModel] = useState('gpt-4o-mini');
  const [localModels, setLocalModels] = useState<string[]>([]);
  const [localNote, setLocalNote] = useState('');
  const [includeTranslation, setIncludeTranslation] = useState(false);
  const [editingUse, setEditingUse] = useState(false);
  const [probe, setProbe] = useState<'idle' | 'busy' | 'ok' | 'err'>('idle');
  const [probeMessage, setProbeMessage] = useState('');

  const definition = provider && provider !== 'custom' ? getProviderDefinition(provider) : null;
  const local = provider === 'ollama' || provider === 'lm_studio';
  const modelChoices = useMemo(() => {
    if (local) return localModels;
    if (!provider || provider === 'custom') return model ? [model] : [];
    return PRESETS[provider] ?? [getModelPlaceholder(provider)];
  }, [local, localModels, model, provider]);

  useEffect(() => {
    if (step !== 'details' || !local || !definition) return;
    let cancelled = false;
    setLocalNote('');
    listLlmModels({
      provider: provider ?? '',
      strategy: definition.strategy,
      baseUrl: definition.defaultApiHost,
      apiKey: '',
    })
      .then((models) => {
        if (cancelled) return;
        const names = models.map((entry) => entry.model).filter(Boolean);
        setLocalModels(names);
        if (names[0]) setModel(names[0]);
        if (names.length === 0) {
          setLocalNote(
            provider === 'ollama'
              ? t('settings.speech_ai.local_empty_ollama')
              : t('settings.speech_ai.local_empty_lmstudio')
          );
        }
      })
      .catch(() => {
        if (cancelled) return;
        setLocalModels([]);
        setLocalNote(
          provider === 'ollama'
            ? t('settings.speech_ai.local_empty_ollama')
            : t('settings.speech_ai.local_empty_lmstudio')
        );
      });
    return () => {
      cancelled = true;
    };
  }, [definition, local, provider, step, t]);

  const runProbe = async () => {
    if (!provider || provider === 'custom' || !model.trim()) return;
    if (definition?.requiresApiKey && !apiKey.trim()) return;
    setProbe('busy');
    setProbeMessage('');
    const started = performance.now();
    try {
      const current = getCurrentLlmSettings(config);
      const setting = {
        ...(current.providers[provider] ?? {
          apiHost: definition?.defaultApiHost ?? '',
          apiKey: '',
        }),
        apiKey: apiKey.trim(),
        apiHost: definition?.defaultApiHost || current.providers[provider]?.apiHost || '',
      };
      const providerConfig = buildLlmConfig(provider, setting, current.customProviders);
      await generateLlmText({
        config: { ...providerConfig, model: model.trim() },
        input: 'Hi',
        source: 'connection_test',
      });
      const latencyMs = Math.round(performance.now() - started);
      setProbe('ok');
      setProbeMessage(
        t('settings.speech_ai.probe_ok', {
          model: model.trim(),
          seconds: (latencyMs / 1000).toFixed(1),
        })
      );
      writeLlmProbe({
        provider,
        model: model.trim(),
        ok: true,
        at: Date.now(),
        latencyMs,
      });
    } catch (error) {
      setProbe('err');
      const message = error instanceof Error ? error.message : String(error);
      setProbeMessage(message);
      writeLlmProbe({
        provider,
        model: model.trim(),
        ok: false,
        at: Date.now(),
        error: message,
      });
    }
  };

  const save = () => {
    if (!provider || probe !== 'ok') return;
    const current = getCurrentLlmSettings(config);
    let next = current;
    let savedProvider: LlmProvider = provider === 'custom' ? 'open_ai' : provider;
    if (provider === 'custom') {
      next = addCustomProvider(next, {
        name: t('settings.speech_ai.custom_service_name'),
        strategy: 'openai_compatible',
      });
      savedProvider = next.activeProvider?.startsWith('custom-')
        ? next.activeProvider
        : savedProvider;
    }
    const def = getProviderDefinition(savedProvider, next.customProviders);
    next = updateProviderSetting(next, savedProvider, {
      apiKey: apiKey.trim(),
      apiHost: def.defaultApiHost || next.providers[savedProvider]?.apiHost || '',
    });
    next = setActiveProvider(next, savedProvider);
    next = addLlmModel(next, { provider: savedProvider, model: model.trim() });
    const modelId = [...next.modelOrder]
      .reverse()
      .find(
        (id) =>
          next.models[id]?.provider === savedProvider && next.models[id]?.model === model.trim()
      );
    if (modelId) {
      next = setFeatureModelSelection(next, 'polish', modelId);
      next = setFeatureModelSelection(next, 'summary', modelId);
      if (includeTranslation) {
        next = setFeatureModelSelection(next, 'translation', modelId);
      }
    }
    updateConfig(buildLlmConfigPatch(next));
    onClose();
  };

  return (
    <div className="speech-ai-dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="speech-ai-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="speech-ai-connect-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="speech-ai-dialog-head">
          <div>
            <h2 id="speech-ai-connect-title">
              {step === 'choose'
                ? t('settings.speech_ai.connect_title')
                : t('settings.speech_ai.connect_provider', {
                    name:
                      provider === 'custom'
                        ? t('settings.speech_ai.openai_compatible')
                        : definition
                          ? t(definition.labelKey)
                          : '',
                  })}
            </h2>
            <p className="speech-ai-detail">
              {step === 'choose'
                ? t('settings.speech_ai.connect_hint')
                : t('settings.speech_ai.connect_key_hint')}
            </p>
          </div>
          <button
            type="button"
            className="btn btn-icon"
            onClick={onClose}
            aria-label={t('common.close')}
          >
            ×
          </button>
        </div>

        {step === 'choose' ? (
          <>
            <div className="speech-ai-provider-grid">
              {GRID.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={`speech-ai-provider${provider === item.id ? ' is-selected' : ''}`}
                  onClick={() => {
                    setProvider(item.id);
                    const preset = PRESETS[item.id]?.[0] ?? getModelPlaceholder(item.id);
                    setModel(preset);
                    setProbe('idle');
                  }}
                >
                  <ProviderLogo provider={item.id} size={28} />
                  <strong>{t(getProviderDefinition(item.id).labelKey)}</strong>
                  <small>
                    {t(
                      item.local
                        ? 'settings.speech_ai.hint_local'
                        : 'settings.speech_ai.hint_api_key'
                    )}
                  </small>
                </button>
              ))}
            </div>
            <button
              type="button"
              className="btn btn-text"
              style={{ justifyContent: 'flex-start' }}
              onClick={() => {
                setProvider('custom');
                setModel('gpt-4o-mini');
                setStep('details');
              }}
            >
              {t('settings.speech_ai.other_service')}
            </button>
          </>
        ) : (
          <>
            {definition?.requiresApiKey ? (
              <label className="speech-ai-field">
                {t('settings.speech_ai.api_key')}
                <input
                  type="password"
                  value={apiKey}
                  autoComplete="off"
                  onChange={(event) => {
                    setApiKey(event.target.value);
                    setProbe('idle');
                  }}
                  onBlur={() => void runProbe()}
                />
              </label>
            ) : null}
            <label className="speech-ai-field">
              {t('settings.speech_ai.model')}
              {modelChoices.length > 0 ? (
                <select
                  value={model}
                  onChange={(event) => {
                    setModel(event.target.value);
                    setProbe('idle');
                  }}
                  onBlur={() => void runProbe()}
                >
                  {modelChoices.map((choice) => (
                    <option key={choice} value={choice}>
                      {choice}
                      {choice === modelChoices[0] ? ` ${t('common.recommended')}` : ''}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  value={model}
                  onChange={(event) => setModel(event.target.value)}
                  onBlur={() => void runProbe()}
                />
              )}
            </label>
            {localNote ? <p className="speech-ai-note">{localNote}</p> : null}
            {probe === 'ok' ? (
              <div className="speech-ai-probe" role="status">
                <strong>{t('settings.speech_ai.connected')}</strong>
                <div>{probeMessage}</div>
              </div>
            ) : null}
            {probe === 'err' ? (
              <div className="speech-ai-probe is-err" role="alert">
                {probeMessage}
              </div>
            ) : null}
            {probe === 'busy' ? (
              <p className="speech-ai-note">{t('settings.speech_ai.checking')}</p>
            ) : null}
            <div className="speech-ai-detail">
              {includeTranslation
                ? t('settings.speech_ai.use_for_all')
                : t('settings.speech_ai.use_for_writing')}{' '}
              <button
                type="button"
                className="speech-ai-inline-btn"
                onClick={() => setEditingUse((value) => !value)}
              >
                {t('settings.speech_ai.edit')}
              </button>
              {editingUse ? (
                <label className="speech-ai-note" style={{ display: 'flex', gap: 8, marginTop: 6 }}>
                  <input
                    type="checkbox"
                    checked={includeTranslation}
                    onChange={(event) => setIncludeTranslation(event.target.checked)}
                  />
                  {t('settings.speech_ai.also_translate')}
                </label>
              ) : null}
            </div>
          </>
        )}

        <div className="speech-ai-dialog-actions">
          {step === 'details' ? (
            <button type="button" className="btn btn-text" onClick={() => setStep('choose')}>
              {t('settings.speech_ai.back_button')}
            </button>
          ) : (
            <button type="button" className="btn btn-text" onClick={onClose}>
              {t('common.cancel')}
            </button>
          )}
          {step === 'choose' ? (
            <button
              type="button"
              className="btn btn-primary"
              disabled={!provider || provider === 'custom'}
              onClick={() => setStep('details')}
            >
              {t('settings.speech_ai.continue')}
            </button>
          ) : (
            <button
              type="button"
              className="btn btn-primary"
              disabled={probe !== 'ok'}
              onClick={save}
            >
              {t('settings.speech_ai.done')}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
