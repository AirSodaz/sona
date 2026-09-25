import {
  AlertCircle,
  Check,
  ChevronRight,
  ExternalLink,
  Eye,
  EyeOff,
  Loader2,
  Plus,
  Zap,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { syncOnlineAsrProviderConfig } from '../../services/asrConfigService';
import {
  getOnlineProviderConfig,
  getProviderAddedModels,
  isVolcengineFlashBatchMode,
  ONLINE_ASR_PROVIDER_DEFAULT_NAMES,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  type OnlineAsrProviderDefinition,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
} from '../../services/onlineAsrProviders';
import { openUrl } from '../../services/tauri/platform/opener';
import { testOnlineAsrProvider } from '../../services/tauri/recognizer';
import { useModelConfig, useSetConfig } from '../../stores/configStore';
import { Dropdown } from '../Dropdown';
import { CheckIcon, TrashIcon } from '../Icons';
import { ModelBrandLogo } from '../icons/ModelLogos';
import { LanguageBadges } from '../LanguageBadges';
import { SettingsItem } from './SettingsLayout';

interface InlineApiKeyInputProps {
  id?: string;
  value: string;
  onChange: (val: string) => void;
  consoleUrl?: string;
  placeholder?: string;
}

function InlineApiKeyInput({
  id,
  value,
  onChange,
  consoleUrl,
  placeholder = 'API Key',
}: InlineApiKeyInputProps): React.JSX.Element {
  const { t } = useTranslation();
  const [showPassword, setShowPassword] = useState(false);
  const handleOpenConsole = async (e: React.MouseEvent) => {
    e.preventDefault();
    if (!consoleUrl) return;
    try {
      await openUrl(consoleUrl);
    } catch {
      window.open(consoleUrl, '_blank', 'noopener,noreferrer');
    }
  };

  return (
    <div className="cloud-api-key-group">
      <div className="cloud-api-key-header">
        <span className="cloud-api-key-label">
          {t('settings.asr.api_key', { defaultValue: 'API Key' })}
        </span>
        {consoleUrl && (
          <a
            href={consoleUrl}
            target="_blank"
            rel="noreferrer"
            className="cloud-console-link"
            onClick={handleOpenConsole}
            data-tooltip={t('settings.asr.get_api_key', {
              defaultValue: 'Get API Key from Console',
            })}
            data-tooltip-pos="top"
          >
            <span>
              {t('settings.asr.get_api_key', { defaultValue: 'Get API Key from Console' })}
            </span>
            <ExternalLink size={12} />
          </a>
        )}
      </div>
      <div className="cloud-api-key-input-wrapper">
        <input
          id={id}
          type={showPassword ? 'text' : 'password'}
          className="settings-input cloud-api-key-input"
          value={value}
          onChange={(e) => onChange(e.target.value.trim())}
          placeholder={placeholder}
        />
        <button
          type="button"
          className="cloud-api-key-eye-btn"
          onClick={() => setShowPassword((prev) => !prev)}
          tabIndex={-1}
          aria-label={
            showPassword
              ? t('settings.sync_hide_password', { defaultValue: 'Hide password' })
              : t('settings.sync_show_password', { defaultValue: 'Show password' })
          }
        >
          {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
        </button>
      </div>
    </div>
  );
}

interface ProviderConfigPanelProps {
  provider: OnlineAsrProviderDefinition;
}

function VolcengineConfigPanel({ provider }: ProviderConfigPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();
  const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
  const spec = provider.spec;

  const update = (updates: Record<string, string | number | boolean>) => {
    updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, updates));
  };

  const isBatchFlash = isVolcengineFlashBatchMode(config);
  const selectedBatchMode = isBatchFlash ? 'flash' : 'standard';
  const batchUnavailableMsg = t('settings.asr.volcengine_batch_mode_url_only_unavailable', {
    defaultValue: 'Requires a public audio URL; local batch import does not support it yet.',
  });
  return (
    <>
      <InlineApiKeyInput
        id="settings-volcengine-api-key"
        value={(config.apiKey as string) ?? ''}
        onChange={(value) => update({ apiKey: value })}
        consoleUrl={spec?.consoleUrl}
        placeholder="X-Api-Key"
      />

      <SettingsItem
        title={t('settings.asr.volcengine_batch_mode_label', {
          defaultValue: 'Recording File Mode',
        })}
        hint={t('settings.asr.volcengine_batch_mode_hint', {
          defaultValue:
            'Local batch import currently supports Flash only. Standard and Off-peak require a public audio URL.',
        })}
      >
        <div style={{ width: '280px' }}>
          <Dropdown
            id="settings-volcengine-batch-mode"
            value={selectedBatchMode}
            onChange={(value) => {
              if (value === 'flash') {
                update({
                  batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
                  batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
                });
              }
            }}
            options={[
              {
                value: 'flash',
                label: t('settings.asr.volcengine_batch_mode_flash', {
                  defaultValue: 'Flash (Sync Direct)',
                }),
              },
              {
                value: 'standard',
                label: t('settings.asr.volcengine_batch_mode_standard', {
                  defaultValue: 'Standard (Async Polling)',
                }),
                description: batchUnavailableMsg,
                disabled: true,
              },
              {
                value: 'offpeak',
                label: t('settings.asr.volcengine_batch_mode_offpeak', {
                  defaultValue: 'Off-peak (Discount Async)',
                }),
                description: batchUnavailableMsg,
                disabled: true,
              },
            ]}
          />
        </div>
      </SettingsItem>
    </>
  );
}

function GroqConfigPanel({ provider }: ProviderConfigPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();
  const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
  const spec = provider.spec;

  const update = (updates: Record<string, string | number | boolean>) => {
    updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, updates));
  };

  return (
    <>
      <InlineApiKeyInput
        id="settings-groq-api-key"
        value={(config.apiKey as string) ?? ''}
        onChange={(value) => update({ apiKey: value })}
        consoleUrl={spec?.consoleUrl}
        placeholder="gsk_..."
      />

      <SettingsItem
        title={t('settings.asr.endpoint', { defaultValue: 'Endpoint' })}
        hint="https://api.groq.com/openai/v1/audio/transcriptions"
      >
        <div style={{ width: '280px' }}>
          <input
            id="settings-groq-endpoint"
            type="text"
            className="settings-input"
            value={
              (config.batchEndpoint as string) ??
              (provider.defaultConfig?.batchEndpoint as string) ??
              ''
            }
            onChange={(e) => update({ batchEndpoint: e.target.value.trim() })}
            placeholder="https://api.groq.com/openai/v1/audio/transcriptions"
          />
        </div>
      </SettingsItem>
    </>
  );
}

function GenericConfigPanel({ provider }: ProviderConfigPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();
  const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
  const spec = provider.spec;

  const update = (key: string, value: string | boolean) => {
    updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, { [key]: value }));
  };

  return (
    <>
      <InlineApiKeyInput
        id={`settings-${provider.id}-api-key`}
        value={(config.apiKey as string) ?? ''}
        onChange={(val) => update('apiKey', val)}
        consoleUrl={spec?.consoleUrl}
      />

      {(provider.manifestEntry.ui.fields || []).map((field) => {
        if (field.name === 'apiKey' || field.name === 'model') return null;
        const defaultFieldLabel =
          field.name === 'batchEndpoint' || field.name === 'endpoint' ? 'Endpoint' : field.name;
        const label = t(field.labelKey, { defaultValue: defaultFieldLabel });
        const val = (config[field.name] as string) ?? '';
        return (
          <SettingsItem key={field.name} title={label}>
            <div style={{ width: '280px' }}>
              <input
                id={`settings-${provider.id}-${field.name}`}
                type={field.type === 'password' ? 'password' : 'text'}
                className="settings-input"
                value={val}
                onChange={(e) => update(field.name, e.target.value.trim())}
                placeholder={defaultFieldLabel}
              />
            </div>
          </SettingsItem>
        );
      })}
    </>
  );
}

const CUSTOM_CONFIG_PANELS: Record<string, React.ComponentType<ProviderConfigPanelProps>> = {
  'volcengine-doubao': VolcengineConfigPanel,
  'groq-whisper': GroqConfigPanel,
};

export function CloudAsrProviderGrid(): React.JSX.Element {
  const { t } = useTranslation();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();

  // Test status per model key: `${provider.id}::${model.id}`
  const [modelTestStates, setModelTestStates] = useState<
    Record<
      string,
      {
        status: 'loading' | 'success' | 'error';
        message?: string;
        latency?: number;
      }
    >
  >({});

  const handleToggle = useCallback((providerId: string) => {
    setExpandedId((prev) => (prev === providerId ? null : providerId));
  }, []);

  const handleTestModel = useCallback(
    async (provider: OnlineAsrProviderDefinition, modelId: string) => {
      const key = `${provider.id}::${modelId}`;
      setModelTestStates((prev) => ({ ...prev, [key]: { status: 'loading' } }));
      const providerConfig = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
      try {
        const elapsed = await testOnlineAsrProvider(provider.id, {
          ...providerConfig,
          model: modelId,
        });
        setModelTestStates((prev) => ({
          ...prev,
          [key]: { status: 'success', latency: elapsed },
        }));
        setTimeout(() => {
          setModelTestStates((prev) => {
            const next = { ...prev };
            delete next[key];
            return next;
          });
        }, 3500);
      } catch (err) {
        setModelTestStates((prev) => ({
          ...prev,
          [key]: {
            status: 'error',
            message: err instanceof Error ? err.message : String(err),
          },
        }));
      }
    },
    [modelConfig.asr?.providers]
  );

  const handleAddModel = useCallback(
    (provider: OnlineAsrProviderDefinition, modelId: string) => {
      const providerConfig = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
      const hasApiKey =
        typeof providerConfig?.apiKey === 'string' && providerConfig.apiKey.trim().length > 0;
      if (!hasApiKey) return;

      const currentAdded = getProviderAddedModels(providerConfig, provider);
      const nextAdded = Array.from(new Set([...currentAdded, modelId]));
      updateConfig(
        syncOnlineAsrProviderConfig(modelConfig, provider.id, { addedModels: nextAdded })
      );
    },
    [modelConfig, updateConfig]
  );

  const handleRemoveModel = useCallback(
    (provider: OnlineAsrProviderDefinition, modelId: string) => {
      const providerConfig = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
      const currentAdded = getProviderAddedModels(providerConfig, provider);
      const nextAdded = currentAdded.filter((id) => id !== modelId);
      const patch = syncOnlineAsrProviderConfig(modelConfig, provider.id, {
        addedModels: nextAdded,
      });

      // Clear selection if currently using this removed model
      const updatedAsr = patch.asr ?? modelConfig.asr;
      if (updatedAsr) {
        const nextSelections = { ...updatedAsr.selections };
        let modified = false;
        for (const slot of ['live', 'caption', 'voiceTyping', 'batch'] as const) {
          const sel = nextSelections[slot];
          if (sel?.engine === 'online' && sel.providerId === provider.id) {
            const isTargetModel =
              sel.modelId === modelId ||
              (!sel.modelId && provider.models.find((m) => m.id === modelId)?.isDefault) ||
              nextAdded.length === 0;
            if (isTargetModel) {
              nextSelections[slot] = {
                engine: 'local',
                mode: sel.mode,
                modelId: null,
                modelPath: '',
              };
              modified = true;
            }
          }
        }
        if (modified) {
          patch.asr = { ...updatedAsr, selections: nextSelections };
        }
      }

      updateConfig(patch);
    },
    [modelConfig, updateConfig]
  );

  return (
    <>
      {ONLINE_ASR_PROVIDER_DEFINITIONS.map((provider) => {
        const providerConfig = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
        const hasApiKey =
          typeof providerConfig?.apiKey === 'string' && providerConfig.apiKey.trim().length > 0;
        const isExpanded = expandedId === provider.id;
        const displayName = t(provider.titleKey, {
          defaultValue: ONLINE_ASR_PROVIDER_DEFAULT_NAMES[provider.id] ?? provider.id,
        });
        const ConfigPanel = CUSTOM_CONFIG_PANELS[provider.id] || GenericConfigPanel;

        const spec = provider.spec;
        const description = spec?.descriptionKey
          ? t(spec.descriptionKey, { defaultValue: 'Cloud speech recognition service.' })
          : t(provider.onlineUploadHintKey, {
              defaultValue: 'Audio will be sent to the cloud for recognition.',
            });

        const supportsStreaming = provider.manifestEntry.streaming?.supported !== false;
        const supportsBatch = provider.manifestEntry.batch?.localFileMode?.supported !== false;

        const commonTagTokens = new Set<string>();
        commonTagTokens.add('cloud');
        if (supportsStreaming) {
          commonTagTokens.add('live');
          commonTagTokens.add('streaming');
        }
        if (supportsBatch) {
          commonTagTokens.add('batch');
        }
        if (provider.supportsSpeakerDiarization) {
          commonTagTokens.add('diarization');
          commonTagTokens.add('speaker diarization');
          commonTagTokens.add('speaker labels');
          commonTagTokens.add('smart diarization');
          commonTagTokens.add('区分说话人');
        }

        const addedModels = getProviderAddedModels(providerConfig, provider);
        const addedModelIds = new Set(addedModels);
        const totalModels = provider.models.length;
        const addedCount = provider.models.filter((m) => addedModelIds.has(m.id)).length;

        const statusChip = (() => {
          if (!hasApiKey && addedCount === 0) {
            return (
              <span className="model-status-chip model-status-not-installed">
                {t('settings.asr.not_configured', { defaultValue: 'Not Configured' })}
              </span>
            );
          }
          if (addedCount === 0) {
            return (
              <span className="model-status-chip model-status-not-installed">
                {t('settings.asr.not_added', { defaultValue: 'Not added' })}
              </span>
            );
          }
          if (addedCount === totalModels) {
            return (
              <span className="model-status-chip model-status-installed">
                <CheckIcon />
                {t('settings.asr.added', { defaultValue: 'Added' })}
              </span>
            );
          }
          return (
            <span className="model-status-chip model-status-partial">
              {t('settings.asr.partial_added', {
                added: addedCount,
                total: totalModels,
                defaultValue: `${addedCount}/${totalModels} added`,
              })}
            </span>
          );
        })();

        return (
          <div
            key={provider.id}
            className={`model-card cloud-provider-card${isExpanded ? ' expanded' : ''}`}
            role="listitem"
          >
            <button
              type="button"
              className="cloud-provider-card-header"
              onClick={() => handleToggle(provider.id)}
              aria-expanded={isExpanded}
              aria-label={`${displayName} — ${addedCount}/${totalModels}`}
            >
              <div className="model-card-identity">
                <div className="model-card-logo-badge">
                  <ModelBrandLogo model={{ id: provider.id, name: displayName }} size={36} />
                </div>
                <div className="model-card-title">
                  <span className="model-name">{displayName}</span>
                  <LanguageBadges languages={provider.manifestEntry.languages} />
                </div>
              </div>
              <div className="model-card-side">
                {statusChip}
                <ChevronRight
                  size={16}
                  className={`cloud-provider-chevron ${isExpanded ? 'open' : ''}`}
                />
              </div>
            </button>

            <div className="cloud-provider-card-summary">
              <div className="model-description">{description}</div>
              <div className="model-card-footer">
                <div className="model-card-meta">
                  <div className="model-tags">
                    {supportsStreaming && <span className="model-tag model-tag-mode">Live</span>}
                    {supportsBatch && <span className="model-tag model-tag-mode">Batch</span>}
                    {provider.supportsSpeakerDiarization && (
                      <span className="model-tag model-tag-diarization">
                        {t('settings.asr.speaker_diarization_label', {
                          defaultValue: 'Speaker Diarization',
                        })}
                      </span>
                    )}
                    <span className="model-tag model-tag-engine">Cloud</span>
                  </div>
                </div>
              </div>
            </div>

            {isExpanded && (
              <div className="cloud-provider-config-panel">
                <ConfigPanel provider={provider} />

                {/* Supported Models Section with Elevated White Card */}
                <div className="cloud-models-section">
                  <div className="cloud-models-header">
                    <span className="cloud-models-title">
                      {t('settings.asr.supported_models', { defaultValue: 'Supported Models' })}
                    </span>
                    <span className="cloud-models-counter">
                      {t('settings.asr.partial_added', {
                        added: addedCount,
                        total: totalModels,
                        defaultValue: `${addedCount}/${totalModels} added`,
                      })}
                    </span>
                  </div>

                  <div className="cloud-provider-models-card">
                    <div className="model-versions">
                      {provider.models.map((model) => {
                        const isAdded = addedModelIds.has(model.id);
                        const testKey = `${provider.id}::${model.id}`;
                        const testState = modelTestStates[testKey];
                        const isTesting = testState?.status === 'loading';

                        let testTooltip = t('settings.asr.test_model', {
                          defaultValue: 'Test Connection',
                        });
                        if (testState?.status === 'loading') {
                          testTooltip = t('settings.asr.verifying', {
                            defaultValue: 'Verifying...',
                          });
                        } else if (testState?.status === 'success') {
                          testTooltip = `✓ ${t('settings.asr.verify_success', { defaultValue: 'Connection verified' })} (${testState.latency}ms)`;
                        } else if (testState?.status === 'error') {
                          testTooltip = `✕ ${testState.message}`;
                        }

                        const displayTags = (model.tags ?? []).filter(
                          (tag) => !commonTagTokens.has(tag.toLowerCase())
                        );

                        return (
                          <div key={model.id} className="model-version-row">
                            <div className="model-version-main">
                              <div className="model-version-info">
                                <span className="model-version-label">{model.name}</span>
                                {displayTags.map((tag) => (
                                  <span key={tag} className="model-tag">
                                    {tag}
                                  </span>
                                ))}
                                {isAdded && (
                                  <span className="cloud-model-added-badge">
                                    <CheckIcon className="model-version-check" />
                                    <span>
                                      {t('settings.asr.added', { defaultValue: 'Added' })}
                                    </span>
                                  </span>
                                )}
                              </div>

                              <div className="model-card-side">
                                {/* Test button: icon-only placed to the left of Add/Delete button */}
                                <button
                                  type="button"
                                  className={`model-action-icon model-action-test${testState?.status ? ` ${testState.status}` : ''}`}
                                  onClick={() => handleTestModel(provider, model.id)}
                                  disabled={isTesting || !hasApiKey}
                                  aria-label={`${t('settings.asr.test_model', { defaultValue: 'Test Connection' })} ${model.name}`}
                                  data-tooltip={testTooltip}
                                  data-tooltip-pos="top"
                                >
                                  {isTesting ? (
                                    <Loader2 size={15} className="spin" />
                                  ) : testState?.status === 'success' ? (
                                    <Check size={15} />
                                  ) : testState?.status === 'error' ? (
                                    <AlertCircle size={15} />
                                  ) : (
                                    <Zap size={15} />
                                  )}
                                </button>

                                {/* Add / Delete button */}
                                {isAdded ? (
                                  <button
                                    type="button"
                                    className="model-action-icon model-action-delete"
                                    onClick={() => handleRemoveModel(provider, model.id)}
                                    aria-label={`${t('common.delete', { defaultValue: 'Delete' })} ${model.name}`}
                                    data-tooltip={t('settings.asr.delete_model', {
                                      defaultValue: 'Delete Model',
                                    })}
                                    data-tooltip-pos="top"
                                  >
                                    <TrashIcon />
                                  </button>
                                ) : (
                                  <button
                                    type="button"
                                    className="model-action-icon model-action-add"
                                    onClick={() => handleAddModel(provider, model.id)}
                                    disabled={!hasApiKey}
                                    aria-label={`${t('common.add', { defaultValue: 'Add' })} ${model.name}`}
                                    data-tooltip={
                                      !hasApiKey
                                        ? t('settings.asr.api_key_required_to_add', {
                                            defaultValue: 'Configure API Key first',
                                          })
                                        : t('settings.asr.add_model', {
                                            defaultValue: 'Add Model',
                                          })
                                    }
                                    data-tooltip-pos="top"
                                  >
                                    <Plus size={16} />
                                  </button>
                                )}
                              </div>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}
