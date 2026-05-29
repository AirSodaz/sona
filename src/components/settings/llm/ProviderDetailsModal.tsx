import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  BrainCircuit,
  Check,
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
  LlmModelMetadata,
  LlmProvider,
} from '../../../types/transcript';
import { normalizeError } from '../../../utils/errorUtils';
import {
  addLlmModel,
  getProviderLlmModels,
  isProviderModelDiscoveryExpired,
  removeLlmModel,
  syncProviderDiscoveredModels,
  updateLlmModelMetadata,
} from '../../../services/llm/state';
import {
  buildLlmConfig,
  createProviderSetting,
  getProviderDefinition,
} from '../../../services/llm/providers';
import { generateLlmText, listLlmModels } from '../../../services/tauri/llm';
import { PanelModal } from '../../PanelModal';
import { getCurrentLlmSettings } from './helpers';

type ModelTestState = {
  status: 'idle' | 'loading' | 'success' | 'error';
  message: string;
};

type ModelMetadataDraft = {
  contextWindow: string;
  maxOutputTokens: string;
  inputPrice: string;
  outputPrice: string;
  supportsMultimodal: boolean;
  supportsTools: boolean;
  supportsReasoning: boolean;
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

function createModelMetadataDraft(entry: LlmModelEntry): ModelMetadataDraft {
  return {
    contextWindow: formatDraftNumber(entry.metadata?.contextWindow),
    maxOutputTokens: formatDraftNumber(entry.metadata?.maxOutputTokens),
    inputPrice: formatDraftNumber(entry.metadata?.inputPrice),
    outputPrice: formatDraftNumber(entry.metadata?.outputPrice),
    supportsMultimodal: Boolean(entry.metadata?.supportsMultimodal),
    supportsTools: Boolean(entry.metadata?.supportsTools),
    supportsReasoning: Boolean(entry.metadata?.supportsReasoning),
  };
}

function formatDraftNumber(value: number | undefined): string {
  return typeof value === 'number' ? String(value) : '';
}

function parseOptionalNonNegativeNumber(value: string): number | undefined | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }

  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
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
  const [metadataDraft, setMetadataDraft] = useState<ModelMetadataDraft>(() => createModelMetadataDraft({
    id: '',
    provider,
    model: '',
  }));
  const [metadataDraftError, setMetadataDraftError] = useState('');
  const autoRefreshKeyRef = useRef<string | null>(null);
  const savedModelCount = providerModels.length;

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
      applyLlmSettings(syncProviderDiscoveredModels(currentLlmState, provider, result, fetchedAt));
      setRefreshState('idle');
    } catch (error) {
      setRefreshState('error');
      setRefreshMessage(normalizeError(error).message);
    }
  }, [
    applyLlmSettings,
    currentLlmState,
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

    applyLlmSettings(addLlmModel(currentLlmState, {
      provider,
      model,
      source: 'manual',
    }));
    setDraftModelName('');
  };

  const handleDeleteModel = (entry: LlmModelEntry) => {
    applyLlmSettings(removeLlmModel(currentLlmState, entry.id));
  };

  const handleEditMetadata = (entry: LlmModelEntry) => {
    setEditingModelId(entry.id);
    setMetadataDraft(createModelMetadataDraft(entry));
    setMetadataDraftError('');
  };

  const handleCancelMetadataEdit = () => {
    setEditingModelId(null);
    setMetadataDraftError('');
  };

  const handleMetadataNumberChange = (field: keyof Pick<ModelMetadataDraft, 'contextWindow' | 'maxOutputTokens' | 'inputPrice' | 'outputPrice'>, value: string) => {
    setMetadataDraft((current) => ({
      ...current,
      [field]: value,
    }));
    setMetadataDraftError('');
  };

  const handleMetadataCapabilityChange = (field: keyof Pick<ModelMetadataDraft, 'supportsMultimodal' | 'supportsTools' | 'supportsReasoning'>, checked: boolean) => {
    setMetadataDraft((current) => ({
      ...current,
      [field]: checked,
    }));
  };

  const handleSaveMetadata = () => {
    if (!editingModelId) {
      return;
    }

    const contextWindow = parseOptionalNonNegativeNumber(metadataDraft.contextWindow);
    const maxOutputTokens = parseOptionalNonNegativeNumber(metadataDraft.maxOutputTokens);
    const inputPrice = parseOptionalNonNegativeNumber(metadataDraft.inputPrice);
    const outputPrice = parseOptionalNonNegativeNumber(metadataDraft.outputPrice);
    if (
      contextWindow === null
      || maxOutputTokens === null
      || inputPrice === null
      || outputPrice === null
    ) {
      setMetadataDraftError(t('settings.llm.model_metadata_invalid_number'));
      return;
    }

    const metadata: Partial<LlmModelMetadata> = {
      contextWindow,
      maxOutputTokens,
      inputPrice,
      outputPrice,
      supportsMultimodal: metadataDraft.supportsMultimodal,
      supportsTools: metadataDraft.supportsTools,
      supportsReasoning: metadataDraft.supportsReasoning,
    };
    applyLlmSettings(updateLlmModelMetadata(currentLlmState, editingModelId, metadata));
    setEditingModelId(null);
    setMetadataDraftError('');
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
          return (
            <div className="provider-model-card" key={entry.id}>
              <div className="provider-model-card-header">
                <div className="provider-model-heading">
                  <div className="provider-model-name-row">
                    <div className="provider-model-name">{entry.model}</div>
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
                <span>{`Context: ${formatOptionalNumber(entry.metadata?.contextWindow)}`}</span>
                <span>{`Max Output: ${formatOptionalNumber(entry.metadata?.maxOutputTokens)}`}</span>
                <span>{`Input: ${formatOptionalNumber(entry.metadata?.inputPrice)}`}</span>
                <span>{`Output: ${formatOptionalNumber(entry.metadata?.outputPrice)}`}</span>
              </div>
              {editingModelId === entry.id ? (
                <div className="provider-model-metadata-editor">
                  {metadataDraftError ? (
                    <div className="connection-error-detail">
                      <X size={12} />
                      <span>{metadataDraftError}</span>
                    </div>
                  ) : null}
                  <div className="provider-model-metadata-editor-grid">
                    <label className="provider-model-metadata-field">
                      <span className="settings-label">{t('settings.llm.model_context_window')}</span>
                      <input
                        className="settings-input"
                        type="number"
                        min={0}
                        value={metadataDraft.contextWindow}
                        onChange={(event) => handleMetadataNumberChange('contextWindow', event.target.value)}
                      />
                    </label>
                    <label className="provider-model-metadata-field">
                      <span className="settings-label">{t('settings.llm.model_max_output_tokens')}</span>
                      <input
                        className="settings-input"
                        type="number"
                        min={0}
                        value={metadataDraft.maxOutputTokens}
                        onChange={(event) => handleMetadataNumberChange('maxOutputTokens', event.target.value)}
                      />
                    </label>
                    <label className="provider-model-metadata-field">
                      <span className="settings-label">{t('settings.llm.model_input_price')}</span>
                      <input
                        className="settings-input"
                        type="number"
                        min={0}
                        step="any"
                        value={metadataDraft.inputPrice}
                        onChange={(event) => handleMetadataNumberChange('inputPrice', event.target.value)}
                      />
                    </label>
                    <label className="provider-model-metadata-field">
                      <span className="settings-label">{t('settings.llm.model_output_price')}</span>
                      <input
                        className="settings-input"
                        type="number"
                        min={0}
                        step="any"
                        value={metadataDraft.outputPrice}
                        onChange={(event) => handleMetadataNumberChange('outputPrice', event.target.value)}
                      />
                    </label>
                  </div>
                  <div className="provider-model-capability-editor">
                    <label className="provider-model-checkbox">
                      <input
                        type="checkbox"
                        checked={metadataDraft.supportsMultimodal}
                        onChange={(event) => handleMetadataCapabilityChange('supportsMultimodal', event.target.checked)}
                      />
                      <span>{t('settings.llm.model_supports_multimodal')}</span>
                    </label>
                    <label className="provider-model-checkbox">
                      <input
                        type="checkbox"
                        checked={metadataDraft.supportsTools}
                        onChange={(event) => handleMetadataCapabilityChange('supportsTools', event.target.checked)}
                      />
                      <span>{t('settings.llm.model_supports_tools')}</span>
                    </label>
                    <label className="provider-model-checkbox">
                      <input
                        type="checkbox"
                        checked={metadataDraft.supportsReasoning}
                        onChange={(event) => handleMetadataCapabilityChange('supportsReasoning', event.target.checked)}
                      />
                      <span>{t('settings.llm.model_supports_reasoning')}</span>
                    </label>
                  </div>
                  <div className="provider-model-editor-actions">
                    <button
                      type="button"
                      className="btn btn-primary"
                      onClick={handleSaveMetadata}
                    >
                      {t('settings.llm.save_model_metadata')}
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={handleCancelMetadataEdit}
                    >
                      {t('common.cancel')}
                    </button>
                  </div>
                </div>
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
