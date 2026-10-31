import { Loader2 } from 'lucide-react';
import type React from 'react';
import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { isFeatureLlmConfigComplete } from '../../../services/llm/configUtils';
import { getProviderDefinition } from '../../../services/llm/providers';
import {
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
import type { LlmProvider } from '../../../types/transcript';
import { extractErrorMessage } from '../../../utils/errorUtils';
import { Dropdown } from '../../Dropdown';
import { Modal } from '../../Modal';
import { getCurrentLlmSettings, getCurrentLlmState } from '../llm/helpers';
import {
  fingerprintApiKey,
  getProviderProbeCacheKey,
  writeProviderProbeCache,
} from './capabilityStatus';

type ConnectMode = 'open_ai' | 'ollama' | 'lm_studio';

const MODE_TO_PROVIDER: Record<ConnectMode, LlmProvider> = {
  open_ai: 'open_ai',
  ollama: 'ollama',
  lm_studio: 'lm_studio',
};

export interface AiWritingConnectModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConnected?: () => void;
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

  const provider = MODE_TO_PROVIDER[mode];
  const definition = getProviderDefinition(provider, currentLlmState.customProviders);
  const requiresKey = definition.requiresApiKey;
  const defaultHost = definition.defaultApiHost || '';

  const resetForMode = useCallback(
    (nextMode: ConnectMode) => {
      const nextProvider = MODE_TO_PROVIDER[nextMode];
      const nextDefinition = getProviderDefinition(nextProvider, currentLlmState.customProviders);
      const existing = currentLlmState.providers[nextProvider];
      setMode(nextMode);
      setApiKey(existing?.apiKey ?? '');
      setApiHost(existing?.apiHost || nextDefinition.defaultApiHost || '');
      setModelId('');
      setDiscoveredModels([]);
      setError(null);
    },
    [currentLlmState.customProviders, currentLlmState.providers]
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
    const cacheKey = getProviderProbeCacheKey(provider, effectiveHost, fingerprintApiKey(apiKey));

    try {
      const models = await listLlmModels({
        provider,
        strategy: definition.strategy,
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

      // Persist discovered models into settings so FeatureCard / Advanced can reuse them.
      const llmState = getCurrentLlmState(config);
      const withProvider = updateProviderSetting(
        setActiveProvider(llmState.llmSettings, provider),
        provider,
        {
          apiHost: effectiveHost,
          apiKey: apiKey.trim(),
        }
      );
      const synced = syncProviderDiscoveredModels(withProvider, provider, models);
      updateConfig(buildLlmConfigPatch(synced));

      return ids;
    } catch (probeError) {
      const message = extractErrorMessage(probeError);
      const friendly =
        /localhost|127\.0\.0\.1|failed to fetch|error sending request|connection refused/i.test(
          message
        )
          ? t('settings.capabilities.ai_not_detected', {
              defaultValue: '{{name}} was not detected at {{host}}.',
              name: t(definition.labelKey, { defaultValue: definition.labelDefault }),
              host: effectiveHost || defaultHost,
            })
          : message;
      setError(friendly);
      writeProviderProbeCache(cacheKey, {
        status: /api.?key|unauthorized|401|403/i.test(message) ? 'needs_key' : 'not_running',
        message: friendly,
        checkedAt: Date.now(),
      });
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
      nextSettings = setActiveProvider(nextSettings, provider);
      nextSettings = updateProviderSetting(nextSettings, provider, {
        apiHost: effectiveHost,
        apiKey: apiKey.trim(),
      });
      nextSettings = addLlmModel(nextSettings, {
        provider,
        model: selectedModel,
        source: 'discovered',
      });
      const storedModelId =
        findLlmModelId(nextSettings, provider, selectedModel) ??
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
        getProviderProbeCacheKey(provider, effectiveHost, fingerprintApiKey(apiKey)),
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
        <div className="capability-connect-modes" role="radiogroup" aria-label="Provider">
          {(
            [
              ['open_ai', 'OpenAI'],
              ['ollama', 'Ollama'],
              ['lm_studio', 'LM Studio'],
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

        {definition.editableApiHost !== false && (
          <label className="capability-field">
            <span>{t('settings.llm.api_host', { defaultValue: 'Base URL' })}</span>
            <input
              className="settings-input"
              value={apiHost}
              onChange={(event) => setApiHost(event.target.value)}
              placeholder={defaultHost}
            />
          </label>
        )}

        {requiresKey && (
          <label className="capability-field">
            <span>{t('settings.llm.api_key', { defaultValue: 'API Key' })}</span>
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
