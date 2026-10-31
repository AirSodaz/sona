import { AlignLeft, Globe, Plus, Settings2, Sparkles } from 'lucide-react';
import type React from 'react';
import { useCallback, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createProviderSetting,
  getProviderDefinition,
  listProviderDefinitions,
} from '../../services/llm/providers';
import {
  addCustomProvider,
  buildLlmConfigPatch,
  getFeatureModelEntry,
  removeCustomProvider,
  setActiveProvider,
  updateCustomProvider,
  updateProviderSetting,
} from '../../services/llm/state';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import type { LlmAssistantConfig } from '../../types/config';
import type {
  CustomLlmProviderStrategy,
  LlmFeature,
  LlmProvider,
  LlmProviderSetting,
} from '../../types/transcript';
import { RobotIcon } from '../Icons';
import { ProviderLogo } from '../icons/ProviderLogos';
import { Modal } from '../Modal';
import { FeatureCard } from './llm/FeatureCard';
import {
  getCurrentLlmSettings,
  getCurrentLlmState,
  isProviderConfiguredForConfig,
} from './llm/helpers';
import { LocalProviderAccordionItem } from './llm/LocalProviderAccordionItem';
import { ProviderAccordionItem } from './llm/ProviderAccordionItem';
import {
  ScenarioCardTabs,
  SettingsItem,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';
import './SettingsLLMServiceTab.css';

const CUSTOM_PROVIDER_MODES: Array<{
  strategy: CustomLlmProviderStrategy;
  labelKey: string;
  defaultLabel: string;
  providerLogo: string;
  endpointHint: string;
}> = [
  {
    strategy: 'openai_compatible',
    labelKey: 'settings.llm.api_mode_openai_compatible',
    defaultLabel: 'OpenAI API compatible',
    providerLogo: 'open_ai',
    endpointHint: '/v1/chat/completions',
  },
  {
    strategy: 'openai_responses',
    labelKey: 'settings.llm.api_mode_openai_responses',
    defaultLabel: 'OpenAI Responses API compatible',
    providerLogo: 'open_ai_responses',
    endpointHint: '/v1/responses',
  },
  {
    strategy: 'anthropic',
    labelKey: 'settings.llm.api_mode_claude',
    defaultLabel: 'Claude API compatible',
    providerLogo: 'anthropic',
    endpointHint: '/v1/messages',
  },
  {
    strategy: 'gemini',
    labelKey: 'settings.llm.api_mode_gemini',
    defaultLabel: 'Google Gemini API compatible',
    providerLogo: 'gemini',
    endpointHint: 'v1beta/models',
  },
];

interface SettingsLLMServiceTabProps {
  isActive?: boolean;
  onOpenProviderDetails?: (provider: LlmProvider) => void;
  /** Render without page chrome when nested under Capabilities → Advanced. */
  embedded?: boolean;
}

export function SettingsLLMServiceTab({
  isActive = true,
  onOpenProviderDetails,
  embedded = false,
}: SettingsLLMServiceTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const [expandedProvider, setExpandedProvider] = useState<LlmProvider | null>(null);
  const [isAddProviderOpen, setIsAddProviderOpen] = useState(false);
  const [providerToAdd, setProviderToAdd] = useState<LlmProvider | null>(null);
  const [activeFeature, setActiveFeature] = useState<LlmFeature>('polish');
  const [editingProvider, setEditingProvider] = useState<LlmProvider | null>(null);
  const [editProviderName, setEditProviderName] = useState('');
  const [editProviderStrategy, setEditProviderStrategy] =
    useState<CustomLlmProviderStrategy>('openai_compatible');
  const [setupApiHost, setSetupApiHost] = useState('');
  const [setupApiKey, setSetupApiKey] = useState('');
  const [customProviderName, setCustomProviderName] = useState('');
  const [customProviderStrategy, setCustomProviderStrategy] =
    useState<CustomLlmProviderStrategy>('openai_compatible');
  const [providerSearch, setProviderSearch] = useState('');
  const searchInputRef = useRef<HTMLInputElement>(null);
  const editNameInputRef = useRef<HTMLInputElement>(null);
  const handleCloseAddProvider = useCallback(() => {
    setIsAddProviderOpen(false);
    setProviderToAdd(null);
    setCustomProviderName('');
    setProviderSearch('');
  }, []);

  const handleCloseEditProvider = useCallback(() => {
    setEditingProvider(null);
  }, []);

  const applyLlmSettings = useCallback(
    (nextLlmSettings: LlmAssistantConfig['llmSettings']) => {
      if (!nextLlmSettings) return;
      updateConfig(buildLlmConfigPatch(nextLlmSettings));
    },
    [updateConfig]
  );

  const applyProviderUpdates = useCallback(
    (provider: LlmProvider, updates: Partial<LlmProviderSetting>) => {
      const currentLlmState = getCurrentLlmState(config);
      const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, provider, updates);
      updateConfig(buildLlmConfigPatch(nextLlmSettings));
    },
    [config, updateConfig]
  );

  const currentLlmState = getCurrentLlmSettings(config);
  const selectedProviderDefinition = providerToAdd
    ? getProviderDefinition(providerToAdd, currentLlmState.customProviders)
    : null;
  const providerDefinitions = useMemo(
    () => listProviderDefinitions(currentLlmState.customProviders),
    [currentLlmState.customProviders]
  );
  const orderedProviderDefinitions = useMemo(
    () =>
      [...providerDefinitions].sort((a, b) => {
        if (a.id === 'local') return -1;
        if (b.id === 'local') return 1;
        const aIsCustom = a.id.startsWith('custom-');
        const bIsCustom = b.id.startsWith('custom-');
        if (aIsCustom && !bIsCustom) return 1;
        if (!aIsCustom && bIsCustom) return -1;
        return 0;
      }),
    [providerDefinitions]
  );
  const configuredProviderDefinitions = useMemo(
    () =>
      orderedProviderDefinitions.filter(
        (def) =>
          def.id !== 'google_translate_free' &&
          (def.id === 'local' ||
            isProviderConfiguredForConfig(config, def.id, currentLlmState.providers[def.id]))
      ),
    [config, currentLlmState.providers, orderedProviderDefinitions]
  );
  const availableProviderDefinitions = useMemo(
    () =>
      orderedProviderDefinitions.filter(
        (def) =>
          def.id !== 'local' &&
          !isProviderConfiguredForConfig(config, def.id, currentLlmState.providers[def.id])
      ),
    [config, currentLlmState.providers, orderedProviderDefinitions]
  );
  const polishModel = getFeatureModelEntry(config, 'polish');
  const translationModel = getFeatureModelEntry(config, 'translation');
  const summaryModel = getFeatureModelEntry(config, 'summary');

  const effectiveExpandedProvider = expandedProvider;

  const openAddProvider = () => {
    setProviderToAdd(null);
    setSetupApiHost('');
    setSetupApiKey('');
    setCustomProviderName('');
    setProviderSearch('');
    setIsAddProviderOpen(true);
  };

  const selectProviderToAdd = (provider: LlmProvider) => {
    const setting =
      currentLlmState.providers[provider] ??
      createProviderSetting(provider, currentLlmState.customProviders);
    setProviderToAdd(provider);
    setSetupApiHost(setting.apiHost);
    setSetupApiKey(setting.apiKey);
  };

  const handleAddProvider = () => {
    if (!providerToAdd) return;
    const seeded = setActiveProvider(currentLlmState, providerToAdd);
    const effectiveHost =
      selectedProviderDefinition?.editableApiHost === false
        ? selectedProviderDefinition.defaultApiHost
        : setupApiHost.trim() || selectedProviderDefinition?.defaultApiHost || '';
    const nextLlmSettings = updateProviderSetting(seeded, providerToAdd, {
      apiHost: effectiveHost,
      apiKey: setupApiKey.trim(),
    });
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    setExpandedProvider(providerToAdd);
    setIsAddProviderOpen(false);
    setProviderToAdd(null);
    setProviderSearch('');
  };

  const openEditProvider = (provider: LlmProvider) => {
    const custom = currentLlmState.customProviders?.[provider as `custom-${string}`];
    if (!custom) return;
    setEditingProvider(provider);
    setEditProviderName(custom.name);
    setEditProviderStrategy(custom.strategy);
  };

  const handleSaveProviderEdit = () => {
    if (!editingProvider || !editProviderName.trim()) return;
    const nextLlmSettings = updateCustomProvider(currentLlmState, editingProvider, {
      name: editProviderName.trim(),
      strategy: editProviderStrategy,
    });
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    setEditingProvider(null);
  };

  const handleDeleteProvider = (provider: LlmProvider) => {
    if (
      !window.confirm(
        t('settings.llm.delete_provider_confirm', {
          defaultValue: 'Delete this provider and its saved models?',
        })
      )
    )
      return;
    const nextLlmSettings = removeCustomProvider(currentLlmState, provider);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    setExpandedProvider(null);
  };

  const handleAddCustomProvider = () => {
    const name = customProviderName.trim();
    if (!name) {
      return;
    }

    const nextLlmSettings = addCustomProvider(currentLlmState, {
      name,
      strategy: customProviderStrategy,
    });
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    const setting =
      nextLlmSettings.providers[nextLlmSettings.activeProvider] ??
      createProviderSetting(nextLlmSettings.activeProvider, nextLlmSettings.customProviders);
    setProviderToAdd(nextLlmSettings.activeProvider);
    setSetupApiHost(setting.apiHost);
    setSetupApiKey(setting.apiKey);
    setCustomProviderName('');
    setCustomProviderStrategy('openai_compatible');
  };

  const content = (
    <>
      {!embedded && (
        <SettingsPageHeader
          icon={<RobotIcon width={28} height={28} />}
          title={t('settings.llm.title')}
          description={t('settings.llm.description', {
            defaultValue:
              'Configure LLM providers and models used for polishing, translating, and summarizing transcripts.',
          })}
        />
      )}

      <SettingsSection
        id={embedded ? 'settings-llm-feature-models' : 'settings-llm-models-config'}
        title={t('settings.llm.feature_models')}
        description={t('settings.llm.feature_models_runtime_hint')}
        icon={<Settings2 size={20} />}
      >
        <ScenarioCardTabs<LlmFeature>
          className="llm-feature-tabs"
          ariaLabel={t('settings.llm.feature_models')}
          activeTab={activeFeature}
          onChange={setActiveFeature}
          idPrefix="settings-llm-feature-tab"
          getPanelId={() => 'settings-llm-feature-panel'}
          items={[
            {
              value: 'polish',
              label: t('settings.llm.polish_model'),
              description: t('settings.llm.polish_model_description', {
                defaultValue: 'Improve wording and readability',
              }),
              icon: <Sparkles size={18} />,
            },
            {
              value: 'translation',
              label: t('settings.llm.translation_model'),
              description: t('settings.llm.translation_model_description', {
                defaultValue: 'Translate transcript text between languages',
              }),
              icon: <Globe size={18} />,
            },
            {
              value: 'summary',
              label: t('settings.llm.summary_model'),
              description: t('settings.llm.summary_model_description', {
                defaultValue: 'Create concise summaries from transcripts',
              }),
              icon: <AlignLeft size={18} />,
            },
          ]}
        />
        <div
          id="settings-llm-feature-panel"
          className="llm-feature-panel"
          role="tabpanel"
          aria-labelledby={`settings-llm-feature-tab-${activeFeature}`}
        >
          {activeFeature === 'polish' && (
            <FeatureCard
              key={`polish:${polishModel?.provider ?? 'open_ai'}:${polishModel?.model ?? ''}`}
              stepNumber={1}
              featureId="polish"
              title={t('settings.llm.polish_model')}
              icon={<Sparkles size={20} />}
              config={config}
              applyLlmSettings={applyLlmSettings}
              t={t}
              isActive={isActive}
              showHeaderTitle={false}
            />
          )}
          {activeFeature === 'translation' && (
            <FeatureCard
              key={`translation:${translationModel?.provider ?? 'open_ai'}:${translationModel?.model ?? ''}`}
              stepNumber={2}
              featureId="translation"
              title={t('settings.llm.translation_model')}
              icon={<Globe size={20} />}
              config={config}
              applyLlmSettings={applyLlmSettings}
              t={t}
              isActive={isActive}
              showHeaderTitle={false}
            />
          )}
          {activeFeature === 'summary' && (
            <FeatureCard
              key={`summary:${summaryModel?.provider ?? 'open_ai'}:${summaryModel?.model ?? ''}`}
              stepNumber={3}
              featureId="summary"
              title={t('settings.llm.summary_model')}
              icon={<AlignLeft size={20} />}
              config={config}
              applyLlmSettings={applyLlmSettings}
              t={t}
              isActive={isActive}
              showHeaderTitle={false}
            />
          )}
        </div>
      </SettingsSection>

      <SettingsSection
        id="settings-llm-provider-list"
        title={t('settings.llm.credentials_section')}
        description={t('settings.llm.credentials_hint')}
        icon={<Settings2 size={20} />}
        contentClassName="accordion-container"
      >
        {configuredProviderDefinitions.map((def) =>
          def.id === 'local' ? (
            <LocalProviderAccordionItem
              key={def.id}
              config={config}
              isOpen={effectiveExpandedProvider === def.id}
              onToggle={() =>
                setExpandedProvider(effectiveExpandedProvider === def.id ? null : def.id)
              }
              applyLlmSettings={applyLlmSettings}
              t={t}
            />
          ) : (
            <ProviderAccordionItem
              key={def.id}
              provider={def.id}
              config={config}
              isOpen={effectiveExpandedProvider === def.id}
              onToggle={() =>
                setExpandedProvider(effectiveExpandedProvider === def.id ? null : def.id)
              }
              applyProviderUpdates={(updates) => applyProviderUpdates(def.id, updates)}
              onOpenDetails={
                onOpenProviderDetails ? () => onOpenProviderDetails(def.id) : undefined
              }
              onEdit={def.id.startsWith('custom-') ? () => openEditProvider(def.id) : undefined}
              onDelete={
                def.id.startsWith('custom-') ? () => handleDeleteProvider(def.id) : undefined
              }
              t={t}
            />
          )
        )}
        {configuredProviderDefinitions.length === 0 && (
          <div className="settings-model-empty provider-empty-state">
            {t('settings.llm.no_configured_providers', {
              defaultValue: 'No providers configured yet.',
            })}
          </div>
        )}
        <div id="settings-llm-custom-provider" className="custom-provider-actions">
          <button type="button" className="btn btn-secondary" onClick={openAddProvider}>
            <Plus size={16} />
            <span>{t('settings.llm.add_custom_provider')}</span>
          </button>
        </div>
      </SettingsSection>

      <SettingsSection>
        <SettingsItem
          title={t('settings.llm.timeout_label', { defaultValue: 'Request Timeout (s)' })}
          hint={t('settings.llm.timeout_hint', {
            defaultValue:
              'Maximum time allowed for an LLM request to complete, in seconds. Default is 180.',
          })}
        >
          <input
            type="number"
            className="input-text"
            value={config.llmRequestTimeoutSeconds ?? 180}
            onChange={(e) =>
              updateConfig({ llmRequestTimeoutSeconds: parseInt(e.target.value, 10) || 180 })
            }
            min={1}
            max={3600}
            style={{ width: '120px' }}
          />
        </SettingsItem>
      </SettingsSection>

      <Modal
        isOpen={isAddProviderOpen}
        onClose={handleCloseAddProvider}
        title={
          providerToAdd
            ? t('settings.llm.configure_provider', { defaultValue: 'Configure provider' })
            : t('settings.llm.add_custom_provider')
        }
        size="lg"
        autoFocus={!providerToAdd}
        initialFocusRef={searchInputRef}
        className="add-provider-modal"
        footer={
          <>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => (providerToAdd ? setProviderToAdd(null) : handleCloseAddProvider())}
            >
              {providerToAdd
                ? t('common.back', { defaultValue: 'Back' })
                : t('settings.llm.add_custom_provider_cancel')}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => (providerToAdd ? handleAddProvider() : handleAddCustomProvider())}
              disabled={
                providerToAdd
                  ? (selectedProviderDefinition?.editableApiHost !== false &&
                      !setupApiHost.trim() &&
                      !selectedProviderDefinition?.defaultApiHost) ||
                    (selectedProviderDefinition?.requiresApiKey && !setupApiKey.trim())
                  : !customProviderName.trim()
              }
            >
              {providerToAdd
                ? t('settings.llm.save_provider', { defaultValue: 'Save provider' })
                : t('settings.llm.add_custom_provider_confirm')}
            </button>
          </>
        }
      >
        {!providerToAdd ? (
          <>
            <div className="add-provider-search">
              <input
                ref={searchInputRef}
                aria-label={t('settings.llm.search_providers', {
                  defaultValue: 'Search providers…',
                })}
                className="settings-input"
                type="text"
                value={providerSearch}
                onChange={(e) => setProviderSearch(e.target.value)}
                placeholder={t('settings.llm.search_providers', {
                  defaultValue: 'Search providers…',
                })}
              />
            </div>
            {(() => {
              const query = providerSearch.trim().toLowerCase();
              const filteredCategories = (['llm', 'translation'] as const).map((category) => {
                const providers = availableProviderDefinitions
                  .filter((def) =>
                    category === 'translation'
                      ? def.id === 'google_translate' || def.id === 'google_translate_free'
                      : def.id !== 'google_translate' && def.id !== 'google_translate_free'
                  )
                  .filter((def) => {
                    if (!query) return true;
                    const label = t(def.labelKey, { defaultValue: def.labelDefault });
                    return (
                      label.toLowerCase().includes(query) ||
                      def.id.toLowerCase().includes(query) ||
                      def.labelDefault.toLowerCase().includes(query)
                    );
                  });
                return { category, providers };
              });

              const totalMatches = filteredCategories.reduce(
                (acc, c) => acc + c.providers.length,
                0
              );

              if (query && totalMatches === 0) {
                return (
                  <div className="provider-picker-empty" role="status" aria-live="polite">
                    {t('common.no_results', { defaultValue: 'No matching providers found' })}
                  </div>
                );
              }

              return filteredCategories.map(({ category, providers }) => {
                if (providers.length === 0) return null;
                return (
                  <div className="provider-picker-group" key={category}>
                    <div className="settings-label">
                      {category === 'llm'
                        ? t('settings.llm.provider_category_llm', { defaultValue: 'LLM' })
                        : t('settings.llm.provider_category_translation', {
                            defaultValue: 'Translation model / API',
                          })}
                    </div>
                    <div className="provider-picker-grid">
                      {providers.map((def) => {
                        const label = t(def.labelKey, { defaultValue: def.labelDefault });
                        return (
                          <button
                            key={def.id}
                            type="button"
                            className="provider-picker-option"
                            onClick={() => selectProviderToAdd(def.id)}
                          >
                            <ProviderLogo
                              provider={def.id}
                              label={label}
                              size={28}
                              aria-hidden="true"
                            />
                            <span className="provider-picker-name">{label}</span>
                            <span
                              className={`provider-picker-meta${!def.requiresApiKey ? ' no-key' : ''}`}
                            >
                              {def.requiresApiKey
                                ? t('settings.llm.requires_api_key', { defaultValue: 'API key' })
                                : t('settings.llm.no_api_key_required', {
                                    defaultValue: 'No key required',
                                  })}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                );
              });
            })()}
            <div className="provider-picker-group provider-picker-group-custom">
              <div className="settings-label">
                {t('settings.llm.custom_provider', { defaultValue: 'Custom provider' })}
              </div>
              <input
                id="custom-provider-name"
                aria-label={t('settings.llm.custom_provider_name')}
                className="settings-input"
                type="text"
                value={customProviderName}
                onChange={(event) => setCustomProviderName(event.target.value)}
                placeholder={t('settings.llm.custom_provider_name')}
              />
              <span className="provider-custom-mode-label">
                {t('settings.llm.custom_provider_api_mode', { defaultValue: 'API mode' })}
              </span>
              <div
                className="provider-mode-options"
                role="radiogroup"
                aria-label={t('settings.llm.custom_provider_api_mode', {
                  defaultValue: 'API mode',
                })}
              >
                {CUSTOM_PROVIDER_MODES.map(
                  ({ strategy, labelKey, defaultLabel, providerLogo, endpointHint }) => {
                    const isSelected = customProviderStrategy === strategy;
                    const label = t(labelKey, { defaultValue: defaultLabel });
                    return (
                      <button
                        key={strategy}
                        type="button"
                        className={`provider-mode-option ${isSelected ? 'selected' : ''}`}
                        aria-pressed={isSelected}
                        aria-label={label}
                        onClick={() =>
                          setCustomProviderStrategy(strategy as CustomLlmProviderStrategy)
                        }
                      >
                        <span className="provider-mode-option-icon" aria-hidden="true">
                          <ProviderLogo provider={providerLogo} size={20} alt="" />
                        </span>
                        <span className="provider-mode-option-content" aria-hidden="true">
                          <span className="provider-mode-option-title">{label}</span>
                          <span className="provider-mode-option-endpoint">{endpointHint}</span>
                        </span>
                        <span className="provider-mode-option-radio" aria-hidden="true">
                          <span
                            className={`provider-mode-radio-dot ${isSelected ? 'is-active' : ''}`}
                          />
                        </span>
                      </button>
                    );
                  }
                )}
              </div>
            </div>
          </>
        ) : (
          (() => {
            const selectedProviderLabel = selectedProviderDefinition
              ? t(selectedProviderDefinition.labelKey, {
                  defaultValue: selectedProviderDefinition.labelDefault,
                })
              : providerToAdd;

            return (
              <>
                <div className="add-provider-selected-header">
                  <ProviderLogo
                    provider={providerToAdd}
                    label={selectedProviderLabel}
                    size={24}
                    aria-hidden="true"
                  />
                  <span>{selectedProviderLabel}</span>
                </div>
                {selectedProviderDefinition?.editableApiHost !== false ? (
                  <div className="shared-form-field layout-vertical">
                    <label className="shared-form-field-label" htmlFor="setup-provider-host">
                      {t('settings.llm.base_url')}
                    </label>
                    <input
                      id="setup-provider-host"
                      className="settings-input"
                      value={setupApiHost}
                      onChange={(event) => setSetupApiHost(event.target.value)}
                      autoFocus
                    />
                  </div>
                ) : null}
                {selectedProviderDefinition?.requiresApiKey && (
                  <div className="shared-form-field layout-vertical">
                    <label className="shared-form-field-label" htmlFor="setup-provider-key">
                      {t('settings.llm.api_key')}
                    </label>
                    <input
                      id="setup-provider-key"
                      className="settings-input"
                      type="password"
                      value={setupApiKey}
                      onChange={(event) => setSetupApiKey(event.target.value)}
                      autoFocus={selectedProviderDefinition?.editableApiHost === false}
                    />
                  </div>
                )}
              </>
            );
          })()
        )}
      </Modal>

      <Modal
        isOpen={!!editingProvider}
        onClose={handleCloseEditProvider}
        title={t('settings.llm.edit_provider', { defaultValue: 'Edit provider' })}
        size="sm"
        className="edit-provider-modal"
        initialFocusRef={editNameInputRef}
        footer={
          <>
            <button type="button" className="btn btn-secondary" onClick={handleCloseEditProvider}>
              {t('common.cancel')}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={handleSaveProviderEdit}
              disabled={!editProviderName.trim()}
            >
              {t('common.save')}
            </button>
          </>
        }
      >
        <div className="shared-form-field layout-vertical">
          <label className="shared-form-field-label" htmlFor="edit-provider-name">
            {t('settings.llm.custom_provider_name')}
          </label>
          <input
            ref={editNameInputRef}
            id="edit-provider-name"
            className="settings-input"
            value={editProviderName}
            onChange={(event) => setEditProviderName(event.target.value)}
            autoFocus
          />
        </div>
        <div className="shared-form-field layout-vertical">
          <span className="shared-form-field-label">
            {t('settings.llm.custom_provider_api_mode')}
          </span>
          <div
            className="provider-mode-options"
            role="radiogroup"
            aria-label={t('settings.llm.custom_provider_api_mode', { defaultValue: 'API mode' })}
          >
            {CUSTOM_PROVIDER_MODES.map(
              ({ strategy, labelKey, defaultLabel, providerLogo, endpointHint }) => {
                const isSelected = editProviderStrategy === strategy;
                const label = t(labelKey, { defaultValue: defaultLabel });
                return (
                  <button
                    key={strategy}
                    type="button"
                    className={`provider-mode-option ${isSelected ? 'selected' : ''}`}
                    aria-pressed={isSelected}
                    aria-label={label}
                    onClick={() => setEditProviderStrategy(strategy as CustomLlmProviderStrategy)}
                  >
                    <span className="provider-mode-option-icon" aria-hidden="true">
                      <ProviderLogo provider={providerLogo} size={20} alt="" />
                    </span>
                    <span className="provider-mode-option-content" aria-hidden="true">
                      <span className="provider-mode-option-title">{label}</span>
                      <span className="provider-mode-option-endpoint">{endpointHint}</span>
                    </span>
                    <span className="provider-mode-option-radio" aria-hidden="true">
                      <span
                        className={`provider-mode-radio-dot ${isSelected ? 'is-active' : ''}`}
                      />
                    </span>
                  </button>
                );
              }
            )}
          </div>
        </div>
      </Modal>
    </>
  );

  if (embedded) {
    return <div className="settings-embedded-llm">{content}</div>;
  }

  return (
    <SettingsTabContainer id="settings-panel-llm_service" ariaLabelledby="settings-tab-llm_service">
      {content}
    </SettingsTabContainer>
  );
}

export default SettingsLLMServiceTab;
