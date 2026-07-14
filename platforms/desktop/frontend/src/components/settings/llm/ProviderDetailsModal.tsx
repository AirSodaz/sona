import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  BrainCircuit,
  Check,
  Database,
  FileJson2,
  ImageIcon,
  LibraryBig,
  Loader2,
  Pencil,
  PlugZap,
  RefreshCw,
  Trash2,
  Wrench,
  X,
} from 'lucide-react';
import type { LlmGenerateCommandRequest } from '../../../types/dashboard';
import type {
  LlmAssistantConfig,
} from '../../../types/config';
import type {
  LlmModelEntry,
  LlmProvider,
} from '../../../types/transcript';
import { normalizeError } from '../../../utils/errorUtils';
import {
  addLlmModel,
  getProviderLlmModels,
  isProviderModelDiscoveryExpired,
  modelSummaryToMetadata,
  removeLlmModel,
  syncProviderDiscoveredModels,
  updateLlmModelMetadata,
} from '../../../services/llm/state';
import {
  buildLlmConfig,
  createProviderSetting,
  getProviderDefinition,
} from '../../../services/llm/providers';
import { describeLlmModel, generateLlmText, listLlmModels } from '../../../services/tauri/llm';
import { PanelModal } from '../../PanelModal';
import { getCurrentLlmSettings } from './helpers';
import { ModelMetadataEditor } from './ModelMetadataEditor';

type ModelTestState = {
  status: 'idle' | 'loading' | 'success' | 'error';
  message: string;
};

interface ProviderDetailsModalProps {
  provider: LlmProvider;
  config: LlmAssistantConfig;
  isOpen: boolean;
  onClose: () => void;
  origin?: 'settings' | 'standalone';
  onBack?: () => void;
  applyLlmSettings: (s: LlmAssistantConfig['llmSettings']) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

function formatOptionalNumber(value: number | undefined): string {
  return typeof value === 'number' ? String(value) : '—';
}

export const ProviderDetailsModal = React.memo(function ProviderDetailsModal({
  provider,
  config,
  isOpen,
  onClose,
  origin = 'standalone',
  onBack,
  applyLlmSettings,
  t,
}: ProviderDetailsModalProps) {
  const currentLlmState = getCurrentLlmSettings(config);
  const definition = getProviderDefinition(provider, currentLlmState.customProviders);
  const setting = currentLlmState.providers[provider] ?? createProviderSetting(provider, currentLlmState.customProviders);
  const providerModels = useMemo(
    () => getProviderLlmModels(currentLlmState, provider),
    [currentLlmState, provider],
  );
  const [refreshState, setRefreshState] = useState<'idle' | 'loading' | 'error'>('idle');
  const [refreshMessage, setRefreshMessage] = useState('');
  const [draftModelName, setDraftModelName] = useState('');
  const [modelTests, setModelTests] = useState<Record<string, ModelTestState>>({});
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const autoRefreshKeyRef = useRef<string | null>(null);
  const latestLlmStateRef = useRef(currentLlmState);
  latestLlmStateRef.current = currentLlmState;
  const savedModelCount = providerModels.length;

  const applyTrackedLlmSettings = useCallback((nextSettings: LlmAssistantConfig['llmSettings']) => {
    if (nextSettings) {
      latestLlmStateRef.current = nextSettings;
    }
    applyLlmSettings(nextSettings);
  }, [applyLlmSettings]);

  const refreshProviderModels = useCallback(async () => {
    if (!definition.supportsModelListing) {
      return;
    }

    setRefreshState('loading');
    setRefreshMessage('');
    try {
      const fetchedAt = new Date().toISOString();
      const result = await listLlmModels({
        provider,
        strategy: definition.strategy,
        baseUrl: setting.apiHost,
        apiKey: setting.apiKey,
      });
      applyTrackedLlmSettings(syncProviderDiscoveredModels(latestLlmStateRef.current, provider, result, fetchedAt));
      setRefreshState('idle');
    } catch (error) {
      setRefreshState('error');
      setRefreshMessage(normalizeError(error).message);
    }
  }, [
    applyTrackedLlmSettings,
    definition.strategy,
    definition.supportsModelListing,
    provider,
    setting.apiHost,
    setting.apiKey,
  ]);

  useEffect(() => {
    if (!isOpen || !definition.supportsModelListing || refreshState === 'loading') {
      return;
    }

    if (!isProviderModelDiscoveryExpired(currentLlmState, provider)) {
      autoRefreshKeyRef.current = null;
      return;
    }

    const discoveryStatus = currentLlmState.modelDiscovery?.[provider];
    const autoRefreshKey = [
      provider,
      setting.apiHost,
      setting.apiKey,
      discoveryStatus?.fetchedAt ?? 'missing',
      discoveryStatus?.expiresAt ?? 'missing',
    ].join('|');
    if (autoRefreshKeyRef.current === autoRefreshKey) {
      return;
    }

    autoRefreshKeyRef.current = autoRefreshKey;
    void refreshProviderModels();
  }, [
    currentLlmState,
    definition.supportsModelListing,
    isOpen,
    provider,
    refreshProviderModels,
    refreshState,
    setting.apiHost,
    setting.apiKey,
  ]);

  if (!isOpen) {
    return null;
  }

  const handleRefresh = async () => {
    await refreshProviderModels();
  };

  const handleAddModel = () => {
    const model = draftModelName.trim();
    if (!model) {
      return;
    }

    const nextSettings = addLlmModel(latestLlmStateRef.current, {
      provider,
      model,
      source: 'manual',
    });
    applyTrackedLlmSettings(nextSettings);
    setDraftModelName('');

    if (provider === 'google_translate' || provider === 'google_translate_free') {
      return;
    }

    void describeLlmModel({
      ...buildLlmConfig(provider, setting, currentLlmState.customProviders),
      model,
    }).then((summary) => {
      if (!summary || summary.model !== model) {
        return;
      }
      const metadata = modelSummaryToMetadata(summary);
      if (Object.keys(metadata).length === 0) {
        return;
      }
      applyTrackedLlmSettings(addLlmModel(latestLlmStateRef.current, {
        provider,
        model,
        source: 'manual',
        metadata,
      }));
    }).catch(() => {
      // Catalog enrichment is best-effort and must not block a manual model.
    });
  };

  const handleDeleteModel = (entry: LlmModelEntry) => {
    applyTrackedLlmSettings(removeLlmModel(latestLlmStateRef.current, entry.id));
  };

  const handleEditMetadata = (entry: LlmModelEntry) => {
    setEditingModelId(entry.id);
  };

  const handleCancelMetadataEdit = () => {
    setEditingModelId(null);
  };

  const handleSaveMetadata = (
    entry: LlmModelEntry,
    metadata: Partial<NonNullable<LlmModelEntry['metadata']>>,
  ) => {
    applyTrackedLlmSettings(updateLlmModelMetadata(latestLlmStateRef.current, entry.id, metadata));
    setEditingModelId(null);
  };

  const handleTestModel = async (entry: LlmModelEntry) => {
    setModelTests((current) => ({
      ...current,
      [entry.id]: {
        status: 'loading',
        message: '',
      },
    }));

    try {
      const providerConfig = buildLlmConfig(provider, setting, currentLlmState.customProviders);
      await generateLlmText({
        config: {
          ...providerConfig,
          model: entry.model,
        },
        input: 'Hello, this is a connection test.',
        source: 'connection_test',
      } satisfies LlmGenerateCommandRequest);
      setModelTests((current) => ({
        ...current,
        [entry.id]: {
          status: 'success',
          message: '',
        },
      }));
    } catch (error) {
      setModelTests((current) => ({
        ...current,
        [entry.id]: {
          status: 'error',
          message: normalizeError(error).message,
        },
      }));
    }
  };

  return (
    <PanelModal
      isOpen={isOpen}
      onClose={onClose}
      size="settings"
      origin={origin}
      onBack={onBack}
      backLabel={t('common.back', { defaultValue: 'Back' })}
      ariaLabel={t('settings.llm.details')}
      className="provider-details-modal"
      overlayClassName="provider-details-overlay"
      headerCopyClassName="provider-details-header-copy"
      toolbarClassName="provider-details-toolbar"
      badge={(
        <>
          <LibraryBig size={16} />
          <span>{t('settings.llm.model_library')}</span>
        </>
      )}
      title={<h2>{t(definition.labelKey, { defaultValue: definition.labelDefault })}</h2>}
      headerActions={(
        <div className="provider-details-actions">
          <div className="provider-details-add-group">
            <input
              className="settings-input"
              type="text"
              value={draftModelName}
              onChange={(event) => setDraftModelName(event.target.value)}
              placeholder={t('settings.llm.model_name')}
            />
            <button
              type="button"
              className="btn btn-secondary"
              onClick={handleAddModel}
              disabled={!draftModelName.trim()}
            >
              {t('common.add')}
            </button>
          </div>
          {definition.supportsModelListing ? (
            <button
              type="button"
              className="btn btn-secondary provider-details-refresh"
              onClick={handleRefresh}
              disabled={refreshState === 'loading'}
            >
              {refreshState === 'loading' ? <Loader2 size={16} className="animate-spin" /> : <RefreshCw size={16} />}
              <span>{t('settings.llm.refresh_models')}</span>
            </button>
          ) : null}
        </div>
      )}
      meta={(
        <>
          <span className="panel-modal-meta-label">{t('settings.llm.model_library')}</span>
          <span>{savedModelCount}</span>
        </>
      )}
    >
      {!definition.supportsModelListing ? (
        <div className="settings-hint">{t('settings.llm.manual_only_provider_hint')}</div>
      ) : null}
      {refreshState === 'error' && refreshMessage ? (
        <div className="connection-error-detail">
          <X size={12} />
          <span>{refreshMessage}</span>
        </div>
      ) : null}

      <div className="provider-model-list">
        {providerModels.map((entry) => {
          const testState = modelTests[entry.id] ?? { status: 'idle', message: '' };
          const inputModalities = entry.metadata?.inputModalities
            ?.map((modality) => t(`settings.llm.modality_${modality}`))
            .join(', ');
          const outputModalities = entry.metadata?.outputModalities
            ?.map((modality) => t(`settings.llm.modality_${modality}`))
            .join(', ');
          const metadataSources = entry.metadata?.metadataSources
            ?.map((source) => t(`settings.llm.metadata_source_${source}`))
            .join(', ');
          return (
            <div className="provider-model-card" key={entry.id}>
              <div className="provider-model-card-header">
                <div className="provider-model-heading">
                  <div className="provider-model-name-row">
                    <div>
                      <div className="provider-model-name">{entry.metadata?.displayName || entry.model}</div>
                      {entry.metadata?.displayName ? (
                        <div className="provider-model-id">{entry.model}</div>
                      ) : null}
                    </div>
                    <div className="provider-model-capabilities">
                      {entry.metadata?.supportsMultimodal ? (
                        <span
                          className="provider-model-capability"
                          aria-label={t('settings.llm.capability_multimodal')}
                          title={t('settings.llm.capability_multimodal')}
                        >
                          <ImageIcon size={14} />
                        </span>
                      ) : null}
                      {entry.metadata?.supportsTools ? (
                        <span
                          className="provider-model-capability"
                          aria-label={t('settings.llm.capability_tools')}
                          title={t('settings.llm.capability_tools')}
                        >
                          <Wrench size={14} />
                        </span>
                      ) : null}
                      {entry.metadata?.supportsReasoning ? (
                        <span
                          className="provider-model-capability"
                          aria-label={t('settings.llm.capability_reasoning')}
                          title={t('settings.llm.capability_reasoning')}
                        >
                          <BrainCircuit size={14} />
                        </span>
                      ) : null}
                      {entry.metadata?.supportsStructuredOutput ? (
                        <span
                          className="provider-model-capability"
                          aria-label={t('settings.llm.capability_structured_output')}
                          title={t('settings.llm.capability_structured_output')}
                        >
                          <FileJson2 size={14} />
                        </span>
                      ) : null}
                      {entry.metadata?.supportsPromptCaching ? (
                        <span
                          className="provider-model-capability"
                          aria-label={t('settings.llm.capability_prompt_caching')}
                          title={t('settings.llm.capability_prompt_caching')}
                        >
                          <Database size={14} />
                        </span>
                      ) : null}
                    </div>
                  </div>
                </div>
                <div className="provider-model-actions">
                  <button
                    type="button"
                    className="btn btn-icon provider-model-edit"
                    aria-label={`${t('settings.llm.edit_model_metadata')} ${entry.model}`}
                    data-tooltip={t('settings.llm.edit_model_metadata')}
                    data-tooltip-pos="top"
                    onClick={() => handleEditMetadata(entry)}
                  >
                    <Pencil size={16} />
                  </button>
                  <button
                    type="button"
                    className="btn btn-icon provider-model-test"
                    aria-label={`${t('settings.llm.test_connection')} ${entry.model}`}
                    data-tooltip={t('settings.llm.test_connection')}
                    data-tooltip-pos="top"
                    onClick={() => void handleTestModel(entry)}
                    disabled={testState.status === 'loading'}
                  >
                    {testState.status === 'loading'
                      ? <Loader2 size={16} className="animate-spin" />
                      : testState.status === 'success'
                        ? <Check size={16} />
                        : <PlugZap size={16} />}
                  </button>
                  {entry.source === 'manual' ? (
                    <button
                      type="button"
                      className="btn btn-icon provider-model-delete"
                      aria-label={`${t('common.delete')} ${entry.model}`}
                      data-tooltip={t('common.delete')}
                      data-tooltip-pos="top"
                      onClick={() => handleDeleteModel(entry)}
                    >
                      <Trash2 size={16} />
                    </button>
                  ) : null}
                </div>
              </div>
              <div className="provider-model-metadata">
                <span>{`${t('settings.llm.model_context_window')}: ${formatOptionalNumber(entry.metadata?.contextWindow)}`}</span>
                <span>{`${t('settings.llm.model_max_output_tokens')}: ${formatOptionalNumber(entry.metadata?.maxOutputTokens)}`}</span>
                <span>{`${t('settings.llm.model_input_price')}: ${formatOptionalNumber(entry.metadata?.inputPrice)}`}</span>
                <span>{`${t('settings.llm.model_output_price')}: ${formatOptionalNumber(entry.metadata?.outputPrice)}`}</span>
                <span>{`${t('settings.llm.model_cache_read_price')}: ${formatOptionalNumber(entry.metadata?.cacheReadPrice)}`}</span>
                <span>{`${t('settings.llm.model_cache_write_price')}: ${formatOptionalNumber(entry.metadata?.cacheWritePrice)}`}</span>
                {entry.metadata?.knowledgeCutoff ? (
                  <span>{`${t('settings.llm.model_knowledge_cutoff')}: ${entry.metadata.knowledgeCutoff}`}</span>
                ) : null}
                {entry.metadata?.releaseDate ? (
                  <span>{`${t('settings.llm.model_release_date')}: ${entry.metadata.releaseDate}`}</span>
                ) : null}
                {entry.metadata?.lastUpdated ? (
                  <span>{`${t('settings.llm.model_last_updated')}: ${entry.metadata.lastUpdated}`}</span>
                ) : null}
                {inputModalities ? (
                  <span>{`${t('settings.llm.model_input_modalities')}: ${inputModalities}`}</span>
                ) : null}
                {outputModalities ? (
                  <span>{`${t('settings.llm.model_output_modalities')}: ${outputModalities}`}</span>
                ) : null}
                {metadataSources ? (
                  <span>{`${t('settings.llm.model_metadata_sources')}: ${metadataSources}`}</span>
                ) : null}
              </div>
              {editingModelId === entry.id ? (
                <ModelMetadataEditor
                  entry={entry}
                  onCancel={handleCancelMetadataEdit}
                  onSave={(metadata) => handleSaveMetadata(entry, metadata)}
                  t={t}
                />
              ) : null}
              {testState.status === 'error' && testState.message ? (
                <div className="connection-error-detail">
                  <X size={12} />
                  <span>{testState.message}</span>
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </PanelModal>
  );
});
