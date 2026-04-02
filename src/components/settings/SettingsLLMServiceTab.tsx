import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Check, Loader2, Trash2, X } from 'lucide-react';
import { Dropdown } from '../Dropdown';
import { AppConfig, LlmConfig, LlmProvider, LlmProviderSetting } from '../../types/transcript';
import { normalizeError } from '../../utils/errorUtils';
import {
  addLlmModel,
  buildLlmConfigPatch,
  ensureLlmState,
  getActiveProvider,
  getActiveProviderSetting,
  getFeatureModelId,
  getOrderedLlmModels,
  getProviderDefinition,
  LLM_PROVIDER_DEFINITIONS,
  removeLlmModel,
  setFeatureModelSelection,
  updateProviderSetting,
} from '../../services/llmConfig';

interface SettingsLLMServiceTabProps {
  config: AppConfig;
  updateConfig: (updates: Partial<AppConfig>) => void;
  changeLlmServiceType: (type: LlmProvider) => void;
}

function getModelPlaceholder(provider: LlmProvider): string {
  switch (provider) {
    case 'azure_openai':
      return 'gpt-4o-deployment';
    case 'anthropic':
      return 'claude-sonnet-4-20250514';
    case 'gemini':
      return 'gemini-2.5-flash';
    case 'ollama':
      return 'qwen3:8b';
    case 'deep_seek':
      return 'deepseek-chat';
    case 'kimi':
      return 'moonshot-v1-8k';
    case 'qwen':
    case 'qwen_portal':
      return 'qwen-max';
    case 'groq':
      return 'llama-3.3-70b-versatile';
    case 'x_ai':
      return 'grok-3-mini';
    case 'mistral_ai':
      return 'mistral-large-latest';
    case 'perplexity':
      return 'sonar';
    default:
      return 'gpt-4.1-mini';
  }
}

function buildModelConfig(provider: LlmProvider, setting: LlmProviderSetting, model: string): LlmConfig {
  return {
    provider,
    baseUrl: setting.apiHost,
    apiKey: setting.apiKey,
    model,
    apiPath: setting.apiPath,
    apiVersion: setting.apiVersion,
    temperature: setting.temperature ?? 0.7,
  };
}

export function SettingsLLMServiceTab({
  config,
  updateConfig,
  changeLlmServiceType,
}: SettingsLLMServiceTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [testMessage, setTestMessage] = useState('');
  const [testingModelId, setTestingModelId] = useState<string | null>(null);
  const [newModelProvider, setNewModelProvider] = useState<LlmProvider>(getActiveProvider(config));
  const [newModelName, setNewModelName] = useState('');

  const activeProvider = getActiveProvider(config);
  const providerDefinition = getProviderDefinition(activeProvider);
  const providerSetting = getActiveProviderSetting(config);
  const models = getOrderedLlmModels(config.llmSettings);
  const polishModelId = getFeatureModelId(config, 'polish') || '';
  const translationModelId = getFeatureModelId(config, 'translation') || '';

  const llmBaseUrl = providerSetting.apiHost || providerDefinition.defaultApiHost;
  const llmApiKey = providerSetting.apiKey || '';
  const llmTemperature = providerSetting.temperature ?? 0.7;
  const llmApiPath = providerSetting.apiPath || providerDefinition.defaultApiPath || '';
  const llmApiVersion = providerSetting.apiVersion || providerDefinition.defaultApiVersion || '';

  const providerOptions = useMemo(() => LLM_PROVIDER_DEFINITIONS.map((provider) => ({
    value: provider.id,
    label: provider.label,
  })), []);

  const modelOptions = useMemo(() => models.map((entry) => ({
    value: entry.id,
    label: `${getProviderDefinition(entry.provider).label} / ${entry.model}`,
  })), [models]);

  const applyProviderSettingUpdates = (updates: Partial<LlmProviderSetting>) => {
    const currentLlmState = config.llmSettings
      ? { llmSettings: config.llmSettings }
      : ensureLlmState(config as AppConfig & Record<string, any>);
    const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, activeProvider, updates);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  };

  const applyLlmSettings = (nextLlmSettings: AppConfig['llmSettings']) => {
    if (!nextLlmSettings) {
      return;
    }
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  };

  const handleAddModel = () => {
    const trimmedModel = newModelName.trim();
    if (!trimmedModel) {
      return;
    }

    const currentLlmState = config.llmSettings
      ? config.llmSettings
      : ensureLlmState(config as AppConfig & Record<string, any>).llmSettings;
    const nextLlmSettings = addLlmModel(currentLlmState, {
      provider: newModelProvider,
      model: trimmedModel,
    });
    applyLlmSettings(nextLlmSettings);
    setNewModelName('');
  };

  const handleRemoveModel = (modelId: string) => {
    const currentLlmState = config.llmSettings
      ? config.llmSettings
      : ensureLlmState(config as AppConfig & Record<string, any>).llmSettings;
    applyLlmSettings(removeLlmModel(currentLlmState, modelId));

    if (testingModelId === modelId) {
      setTestingModelId(null);
      setTestStatus('idle');
      setTestMessage('');
    }
  };

  const handleFeatureSelection = (feature: 'polish' | 'translation', modelId: string) => {
    const currentLlmState = config.llmSettings
      ? config.llmSettings
      : ensureLlmState(config as AppConfig & Record<string, any>).llmSettings;
    applyLlmSettings(setFeatureModelSelection(currentLlmState, feature, modelId || undefined));
  };

  const handleTestConnection = async (modelId: string) => {
    const entry = config.llmSettings?.models[modelId];
    if (!entry) {
      return;
    }

    const setting = config.llmSettings?.providers[entry.provider];
    const providerConfig = buildModelConfig(
      entry.provider,
      setting || getActiveProviderSetting({ llmSettings: ensureLlmState(config as AppConfig & Record<string, any>).llmSettings }),
      entry.model,
    );

    setTestingModelId(modelId);
    setTestStatus('loading');
    setTestMessage('');

    try {
      const response = await invoke<string>('generate_llm_text', {
        request: {
          config: providerConfig,
          input: 'Hello, this is a connection test.',
        },
      });
      setTestStatus('success');
      setTestMessage(response);
    } catch (error) {
      setTestStatus('error');
      setTestMessage(normalizeError(error).message);
    }
  };

  const apiHostLabel = providerDefinition.apiHostLabel || t('settings.llm.base_url');

  return (
    <div className="settings-group" role="tabpanel">
      <div className="settings-item">
        <label className="settings-label">{t('settings.llm.service_type')}</label>
        <Dropdown
          id="llm-service-type"
          value={activeProvider}
          onChange={(value) => changeLlmServiceType(value as LlmProvider)}
          options={providerOptions}
          style={{ width: '100%' }}
        />
      </div>

      <div className="settings-item">
        <label className="settings-label">{apiHostLabel}</label>
        {providerDefinition.editableApiHost === false ? (
          <div className="settings-input" style={{ alignItems: 'center', display: 'flex', minHeight: 40, opacity: 0.75 }}>
            {llmBaseUrl}
          </div>
        ) : (
          <input
            type="text"
            className="settings-input"
            value={providerSetting.apiHost}
            onChange={(e) => applyProviderSettingUpdates({ apiHost: e.target.value })}
            placeholder={providerDefinition.defaultApiHost}
          />
        )}
      </div>

      <div className="settings-item">
        <label className="settings-label">{t('settings.llm.api_key')}</label>
        <input
          type="password"
          className="settings-input"
          value={llmApiKey}
          onChange={(e) => applyProviderSettingUpdates({ apiKey: e.target.value })}
          placeholder={providerDefinition.requiresApiKey ? 'sk-...' : t('settings.llm.optional_api_key')}
        />
      </div>

      {llmApiVersion && (
        <div className="settings-item">
          <label className="settings-label">{t('settings.llm.api_version')}</label>
          <input
            type="text"
            className="settings-input"
            value={llmApiVersion}
            onChange={(e) => applyProviderSettingUpdates({ apiVersion: e.target.value })}
            placeholder={providerDefinition.defaultApiVersion || ''}
          />
        </div>
      )}

      {llmApiPath && (
        <div className="settings-item">
          <label className="settings-label">{t('settings.llm.api_path')}</label>
          <input
            type="text"
            className="settings-input"
            value={llmApiPath}
            onChange={(e) => applyProviderSettingUpdates({ apiPath: e.target.value })}
            readOnly={activeProvider === 'open_ai_responses' || activeProvider === 'volcengine' || activeProvider === 'perplexity'}
          />
        </div>
      )}

      <div className="settings-item with-divider">
        <label className="settings-label">{t('settings.llm.temperature')}</label>
        <div
          style={{
            alignItems: 'center',
            display: 'grid',
            gap: '8px',
            gridTemplateColumns: '1fr 180px 60px',
          }}
        >
          <div />
          <input
            type="range"
            style={{ justifySelf: 'end', margin: 0, width: '180px' }}
            min={0}
            max={2}
            step={0.05}
            value={llmTemperature}
            onChange={(e) => applyProviderSettingUpdates({ temperature: parseFloat(e.target.value) })}
          />
          <input
            type="number"
            className="settings-input"
            style={{ padding: '2px 4px', textAlign: 'center', width: '60px' }}
            min={0}
            max={2}
            step={0.05}
            value={llmTemperature}
            onChange={(e) => {
              const value = parseFloat(e.target.value);
              if (!Number.isNaN(value) && value >= 0 && value <= 2) {
                applyProviderSettingUpdates({ temperature: value });
              }
            }}
          />
        </div>
      </div>

      <div className="settings-item with-divider">
        <label className="settings-label">{t('settings.llm.added_models')}</label>
        <div style={{ display: 'grid', gap: '8px', gridTemplateColumns: '180px 1fr auto' }}>
          <Dropdown
            id="llm-model-provider"
            value={newModelProvider}
            onChange={(value) => setNewModelProvider(value as LlmProvider)}
            options={providerOptions}
            style={{ width: '100%' }}
          />
          <input
            type="text"
            className="settings-input"
            value={newModelName}
            onChange={(e) => setNewModelName(e.target.value)}
            placeholder={getModelPlaceholder(newModelProvider)}
          />
          <button
            className="btn btn-primary"
            onClick={handleAddModel}
            disabled={!newModelName.trim()}
            type="button"
          >
            {t('settings.llm.add_model')}
          </button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '16px' }}>
          {models.length === 0 ? (
            <div className="settings-hint">{t('settings.llm.no_models_added')}</div>
          ) : (
            models.map((entry) => {
              const isTesting = testingModelId === entry.id && testStatus === 'loading';
              const providerLabel = getProviderDefinition(entry.provider).label;
              const usedBy: string[] = [];
              if (polishModelId === entry.id) usedBy.push(t('settings.llm.used_for_polish'));
              if (translationModelId === entry.id) usedBy.push(t('settings.llm.used_for_translation'));

              return (
                <div
                  key={entry.id}
                  style={{
                    alignItems: 'center',
                    border: '1px solid var(--color-border)',
                    borderRadius: 'var(--radius-md)',
                    display: 'grid',
                    gap: '12px',
                    gridTemplateColumns: '1fr auto',
                    padding: '12px',
                  }}
                >
                  <div>
                    <div style={{ fontWeight: 600 }}>{providerLabel} / {entry.model}</div>
                    {usedBy.length > 0 && (
                      <div style={{ color: 'var(--color-text-muted)', fontSize: '0.875rem', marginTop: '4px' }}>
                        {usedBy.join(' · ')}
                      </div>
                    )}
                  </div>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <button
                      type="button"
                      className="btn btn-secondary btn-loading-wrapper"
                      onClick={() => handleTestConnection(entry.id)}
                      disabled={isTesting}
                    >
                      <span className={isTesting ? 'btn-text-hidden' : ''}>{t('settings.llm.test_connection')}</span>
                      {isTesting && (
                        <div className="btn-spinner-overlay">
                          <Loader2 className="animate-spin" size={16} />
                        </div>
                      )}
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary btn-icon"
                      onClick={() => handleRemoveModel(entry.id)}
                      aria-label={t('common.delete_item', { item: entry.model })}
                      title={t('common.delete_item', { item: entry.model })}
                    >
                      <Trash2 size={16} />
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      <div className="settings-item with-divider">
        <label className="settings-label">{t('settings.llm.feature_models')}</label>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div>
            <label className="settings-label">{t('settings.llm.polish_model')}</label>
            <Dropdown
              id="llm-polish-model"
              value={polishModelId}
              onChange={(value) => handleFeatureSelection('polish', value)}
              options={modelOptions}
              placeholder={t('settings.llm.unassigned')}
              style={{ width: '100%' }}
            />
          </div>
          <div>
            <label className="settings-label">{t('settings.llm.translation_model')}</label>
            <Dropdown
              id="llm-translation-model"
              value={translationModelId}
              onChange={(value) => handleFeatureSelection('translation', value)}
              options={modelOptions}
              placeholder={t('settings.llm.unassigned')}
              style={{ width: '100%' }}
            />
          </div>
        </div>

        {(!polishModelId || !translationModelId) && (
          <div className="settings-hint" style={{ marginTop: '12px' }}>
            {t('settings.llm.feature_models_hint')}
          </div>
        )}

        {testMessage && (
          <div className={`connection-status ${testStatus === 'error' ? 'error' : 'success'}`} style={{ marginTop: '16px' }}>
            {testStatus === 'error' ? (
              <X size={16} style={{ flexShrink: 0, marginTop: 2 }} />
            ) : (
              <Check size={16} style={{ flexShrink: 0, marginTop: 2 }} />
            )}
            <div>
              <strong>
                {testStatus === 'error'
                  ? t('settings.llm.connection_failed')
                  : t('settings.llm.connection_success')}
              </strong>
              <div style={{ marginTop: 4, opacity: 0.9 }}>{testMessage}</div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
