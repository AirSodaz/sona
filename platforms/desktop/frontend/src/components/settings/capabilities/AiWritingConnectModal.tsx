import { Loader2 } from 'lucide-react';
import type React from 'react';
import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { isFeatureLlmConfigComplete } from '../../../services/llm/configUtils';
import { getProviderDefinition } from '../../../services/llm/providers';
import {
  addCustomProvider,
  addLlmModel,
  buildLlmConfigPatch,
  findLlmModelId,
  setActiveProvider,
  setFeatureModelSelection,
  syncProviderDiscoveredModels,
  updateProviderSetting,
} from '../../../services/llm/state';
import { listLlmModels } from '../../../services/tauri/llm';
import { useLlmAssistantConfig, useSetConfig } from '../../../stores/configStore';
import type { LlmProvider, LlmSettings } from '../../../types/transcript';
import { extractErrorMessage } from '../../../utils/errorUtils';
import { Dropdown } from '../../Dropdown';
import { Modal } from '../../Modal';
import { getCurrentLlmSettings, getCurrentLlmState } from '../llm/helpers';
import {
  fingerprintApiKey,
  getProviderProbeCacheKey,
  writeProviderProbeCache,
} from './capabilityStatus';

type ConnectMode = 'open_ai' | 'ollama' | 'lm_studio' | 'openai_compatible';

const BUILTIN_MODE_TO_PROVIDER: Record<Exclude<ConnectMode, 'openai_compatible'>, LlmProvider> = {
  open_ai: 'open_ai',
  ollama: 'ollama',
  lm_studio: 'lm_studio',
};

const COMPATIBLE_PROVIDER_NAME = 'OpenAI Compatible';

export interface AiWritingConnectModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConnected?: () => void;
}

function findExistingCompatibleProvider(llmSettings: LlmSettings): LlmProvider | null {
  const customs = llmSettings.customProviders ?? {};
  const preferred = Object.values(customs).find(
    (provider) =>
      provider?.strategy === 'openai_compatible' &&
      (provider.id === 'custom-openai-compatible' ||
        provider.name.trim().toLowerCase() === COMPATIBLE_PROVIDER_NAME.toLowerCase())
  );
  if (preferred) {
    return preferred.id;
  }
  const anyCompatible = Object.values(customs).find(
    (provider) => provider?.strategy === 'openai_compatible'
  );
  return anyCompatible?.id ?? null;
}

/**
 * One-shot AI Writing connection: provider + credentials + probed model list,
 * then apply to Polish and Summary together.
 */
export function AiWritingConnectModal({
  isOpen,
  onClose,
  onConnected,
}: AiWritingConnectModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const currentLlmState = getCurrentLlmSettings(config);

  const [mode, setMode] = useState<ConnectMode>('open_ai');
  const [apiKey, setApiKey] = useState('');
  const [apiHost, setApiHost] = useState('');
  const [modelId, setModelId] = useState('');
  const [discoveredModels, setDiscoveredModels] = useState<string[]>([]);
  const [probing, setProbing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [compatibleProviderId, setCompatibleProviderId] = useState<LlmProvider | null>(() =>
    findExistingCompatibleProvider(currentLlmState)
  );

  const provider: LlmProvider =
    mode === 'openai_compatible'
      ? (compatibleProviderId ?? 'custom-openai-compatible')
      : BUILTIN_MODE_TO_PROVIDER[mode];

  const definition = getProviderDefinition(provider, currentLlmState.customProviders);
  const isCompatibleMode = mode === 'openai_compatible';
  // Compatible endpoints often run without a key (local proxies); treat key as optional.
  const requiresKey = isCompatibleMode ? false : definition.requiresApiKey;
  const defaultHost = isCompatibleMode ? '' : definition.defaultApiHost || '';
  const showApiHost = isCompatibleMode || definition.editableApiHost !== false;

  const resetForMode = useCallback(
    (nextMode: ConnectMode) => {
      setMode(nextMode);
      setModelId('');
      setDiscoveredModels([]);
      setError(null);

      if (nextMode === 'openai_compatible') {
        const existingId = findExistingCompatibleProvider(currentLlmState);
        setCompatibleProviderId(existingId);
        const existing = existingId ? currentLlmState.providers[existingId] : undefined;
        setApiKey(existing?.apiKey ?? '');
        setApiHost(existing?.apiHost ?? '');
        return;
      }

      const nextProvider = BUILTIN_MODE_TO_PROVIDER[nextMode];
      const nextDefinition = getProviderDefinition(nextProvider, currentLlmState.customProviders);
      const existing = currentLlmState.providers[nextProvider];
      setApiKey(existing?.apiKey ?? '');
      setApiHost(existing?.apiHost || nextDefinition.defaultApiHost || '');
    },
    [currentLlmState]
  );

  const effectiveHost = (apiHost.trim() || defaultHost).trim();
  const canProbe = Boolean(effectiveHost) && (!requiresKey || Boolean(apiKey.trim()));

  const modelOptions = useMemo(
    () =>
      discoveredModels.map((id) => ({
        value: id,
        label: id,
      })),
    [discoveredModels]
  );

  const ensureCompatibleProvider = useCallback(
    (llmSettings: LlmSettings): { settings: LlmSettings; providerId: LlmProvider } => {
      const existingId = findExistingCompatibleProvider(llmSettings) ?? compatibleProviderId;
      if (existingId && llmSettings.customProviders?.[existingId as `custom-${string}`]) {
        setCompatibleProviderId(existingId);
        return { settings: llmSettings, providerId: existingId };
      }

      const created = addCustomProvider(llmSettings, {
        name: COMPATIBLE_PROVIDER_NAME,
        strategy: 'openai_compatible',
      });
      const providerId = created.activeProvider;
      setCompatibleProviderId(providerId);
      return { settings: created, providerId };
    },
    [compatibleProviderId]
  );

  const resolveProviderForRequest = useCallback(
    (llmSettings: LlmSettings): { settings: LlmSettings; providerId: LlmProvider } => {
      if (mode !== 'openai_compatible') {
        return { settings: llmSettings, providerId: BUILTIN_MODE_TO_PROVIDER[mode] };
      }
      return ensureCompatibleProvider(llmSettings);
    },
    [ensureCompatibleProvider, mode]
  );

  const probeModels = async (): Promise<string[]> => {
    if (!canProbe) {
      setError(
        t('settings.capabilities.ai_needs_credentials', {
          defaultValue: 'Enter the required credentials before testing the connection.',
        })
      );
      return [];
    }

    setProbing(true);
    setError(null);

    try {
      const llmState = getCurrentLlmState(config);
      const { settings: withProviderBase, providerId } = resolveProviderForRequest(
        llmState.llmSettings
      );
      const providerDef = getProviderDefinition(providerId, withProviderBase.customProviders);
      const cacheKey = getProviderProbeCacheKey(
        providerId,
        effectiveHost,
        fingerprintApiKey(apiKey)
      );

      const withProvider = updateProviderSetting(
        setActiveProvider(withProviderBase, providerId),
        providerId,
        {
          apiHost: effectiveHost,
          apiKey: apiKey.trim(),
        }
      );

      const models = await listLlmModels({
        provider: providerId,
        strategy: providerDef.strategy,
        baseUrl: effectiveHost,
        apiKey: apiKey.trim(),
      });
      const ids = models.map((entry) => entry.model.trim()).filter(Boolean);
      setDiscoveredModels(ids);
      if (ids.length > 0 && !ids.includes(modelId)) {
        setModelId(ids[0]);
      }
      writeProviderProbeCache(cacheKey, {
        status: 'connected',
        checkedAt: Date.now(),
        modelIds: ids,
      });

      const synced = syncProviderDiscoveredModels(withProvider, providerId, models);
      updateConfig(buildLlmConfigPatch(synced));

      return ids;
    } catch (probeError) {
      const message = extractErrorMessage(probeError);
      const displayName = isCompatibleMode
        ? t('settings.capabilities.openai_compatible', {
            defaultValue: 'OpenAI-compatible',
          })
        : t(definition.labelKey, { defaultValue: definition.labelDefault });
      const friendly =
        /localhost|127\.0\.0\.1|failed to fetch|error sending request|connection refused/i.test(
          message
        )
          ? t('settings.capabilities.ai_not_detected', {
              defaultValue: '{{name}} was not detected at {{host}}.',
              name: displayName,
              host: effectiveHost || defaultHost || '—',
            })
          : message;
      setError(friendly);
      writeProviderProbeCache(
        getProviderProbeCacheKey(provider, effectiveHost, fingerprintApiKey(apiKey)),
        {
          status: /api.?key|unauthorized|401|403/i.test(message) ? 'needs_key' : 'not_running',
          message: friendly,
          checkedAt: Date.now(),
        }
      );
      setDiscoveredModels([]);
      return [];
    } finally {
      setProbing(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      let models = discoveredModels;
      if (models.length === 0) {
        models = await probeModels();
        if (models.length === 0) {
          return;
        }
      }

      const selectedModel = (modelId.trim() || models[0] || '').trim();
      if (!selectedModel) {
        setError(
          t('settings.capabilities.ai_select_model', {
            defaultValue: 'Select a model returned by the provider.',
          })
        );
        return;
      }
      if (!models.includes(selectedModel)) {
        setError(
          t('settings.capabilities.ai_model_not_listed', {
            defaultValue: 'Choose a model from the provider list. Prefill guesses are disabled.',
          })
        );
        return;
      }

      let nextSettings = getCurrentLlmSettings(config);
      const resolved = resolveProviderForRequest(nextSettings);
      nextSettings = resolved.settings;
      const providerId = resolved.providerId;
      nextSettings = setActiveProvider(nextSettings, providerId);
      nextSettings = updateProviderSetting(nextSettings, providerId, {
        apiHost: effectiveHost,
        apiKey: apiKey.trim(),
      });
      nextSettings = addLlmModel(nextSettings, {
        provider: providerId,
        model: selectedModel,
        source: 'discovered',
      });
      const storedModelId =
        findLlmModelId(nextSettings, providerId, selectedModel) ??
        nextSettings.modelOrder[nextSettings.modelOrder.length - 1];
      nextSettings = setFeatureModelSelection(nextSettings, 'polish', storedModelId);
      nextSettings = setFeatureModelSelection(nextSettings, 'summary', storedModelId);
      updateConfig(buildLlmConfigPatch(nextSettings));

      const verifyConfig = { ...config, ...buildLlmConfigPatch(nextSettings) };
      if (!isFeatureLlmConfigComplete(verifyConfig, 'polish')) {
        setError(
          t('settings.capabilities.ai_incomplete', {
            defaultValue: 'Connection saved but polish is still incomplete.',
          })
        );
        return;
      }

      writeProviderProbeCache(
        getProviderProbeCacheKey(providerId, effectiveHost, fingerprintApiKey(apiKey)),
        {
          status: 'connected',
          checkedAt: Date.now(),
          modelIds: models,
        }
      );
      onConnected?.();
      onClose();
    } finally {
      setSaving(false);
    }
  };

  if (!isOpen) {
    return null;
  }

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('settings.capabilities.connect_ai_writing', {
        defaultValue: 'Connect AI Writing',
      })}
      size="md"
      footer={
        <div className="capability-modal-footer">
          <button type="button" className="btn btn-secondary" onClick={onClose} disabled={saving}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void handleSave()}
            disabled={saving || probing || !canProbe}
          >
            {(saving || probing) && <Loader2 className="spin" size={14} />}
            {t('settings.capabilities.use_for_ai_writing', {
              defaultValue: 'Use for Polish & Summary',
            })}
          </button>
        </div>
      }
    >
      <div className="capability-connect">
        <div
          className="capability-connect-modes capability-connect-modes-4"
          role="radiogroup"
          aria-label="Provider"
        >
          {(
            [
              ['open_ai', 'OpenAI'],
              ['ollama', 'Ollama'],
              ['lm_studio', 'LM Studio'],
              [
                'openai_compatible',
                t('settings.capabilities.openai_compatible_short', {
                  defaultValue: 'Compatible',
                }),
              ],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              role="radio"
              aria-checked={mode === value}
              className={`capability-connect-mode ${mode === value ? 'active' : ''}`}
              onClick={() => resetForMode(value)}
            >
              {label}
            </button>
          ))}
        </div>

        {isCompatibleMode && (
          <p className="settings-hint">
            {t('settings.capabilities.openai_compatible_hint', {
              defaultValue:
                'Any OpenAI-compatible chat completions endpoint (local proxy, gateway, or cloud).',
            })}
          </p>
        )}

        {showApiHost && (
          <label className="capability-field">
            <span>{t('settings.llm.api_host', { defaultValue: 'Base URL' })}</span>
            <input
              className="settings-input"
              value={apiHost}
              onChange={(event) => setApiHost(event.target.value)}
              placeholder={
                isCompatibleMode
                  ? 'https://example.com/v1'
                  : defaultHost || 'http://localhost:11434'
              }
            />
          </label>
        )}

        {(requiresKey || isCompatibleMode) && (
          <label className="capability-field">
            <span>
              {isCompatibleMode
                ? t('settings.capabilities.api_key_optional', {
                    defaultValue: 'API Key (optional)',
                  })
                : t('settings.llm.api_key', { defaultValue: 'API Key' })}
            </span>
            <input
              className="settings-input"
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder="sk-…"
              autoComplete="off"
            />
          </label>
        )}

        <div className="capability-connect-actions">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void probeModels()}
            disabled={!canProbe || probing || saving}
          >
            {probing && <Loader2 className="spin" size={14} />}
            {t('settings.capabilities.test_connection', {
              defaultValue: 'Test connection',
            })}
          </button>
        </div>

        {discoveredModels.length > 0 && (
          <label className="capability-field">
            <span>{t('settings.llm.model', { defaultValue: 'Model' })}</span>
            <Dropdown
              value={modelId}
              onChange={setModelId}
              options={modelOptions}
              placeholder={t('settings.capabilities.select_probed_model', {
                defaultValue: 'Select a model from the server…',
              })}
            />
          </label>
        )}

        <p className="settings-hint">
          {t('settings.capabilities.ai_writing_apply_hint', {
            defaultValue: 'This connection will be used for Polish and Summary.',
          })}
        </p>

        {error && (
          <div className="capability-error" role="alert">
            {error}
          </div>
        )}
      </div>
    </Modal>
  );
}
