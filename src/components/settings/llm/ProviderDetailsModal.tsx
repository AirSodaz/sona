import React, { useCallback, useMemo, useState } from 'react';
import {
  BrainCircuit,
  Check,
  ImageIcon,
  LibraryBig,
  Loader2,
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
  removeLlmModel,
  syncProviderDiscoveredModels,
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
  const savedModelCount = providerModels.length;

  const refreshProviderModels = useCallback(async () => {
    if (!definition.supportsModelListing) {
      return;
    }

    setRefreshState('loading');
    setRefreshMessage('');
    try {
      const result = await listLlmModels({
        provider,
        strategy: definition.strategy,
        baseUrl: setting.apiHost,
        apiKey: setting.apiKey,
      });
      applyLlmSettings(syncProviderDiscoveredModels(currentLlmState, provider, result));
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
      title={<h2>{definition.label}</h2>}
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
                    className="btn btn-secondary"
                    aria-label={`${t('settings.llm.test_connection')} ${entry.model}`}
                    onClick={() => void handleTestModel(entry)}
                    disabled={testState.status === 'loading'}
                  >
                    {testState.status === 'loading'
                      ? <Loader2 size={16} className="animate-spin" />
                      : testState.status === 'success'
                        ? <Check size={16} />
                        : null}
                    <span>
                      {testState.status === 'success'
                        ? t('settings.llm.connection_success')
                        : testState.status === 'error'
                          ? t('settings.llm.connection_failed')
                          : t('settings.llm.test_connection')}
                    </span>
                  </button>
                  {entry.source === 'manual' ? (
                    <button
                      type="button"
                      className="btn btn-icon btn-secondary-soft"
                      aria-label={t('common.delete')}
                      onClick={() => handleDeleteModel(entry)}
                    >
                      <Trash2 size={16} />
                    </button>
                  ) : null}
                </div>
              </div>
              <div className="provider-model-metadata">
                <span>{`Context: ${formatOptionalNumber(entry.metadata?.contextWindow)}`}</span>
                <span>{`Input: ${formatOptionalNumber(entry.metadata?.inputPrice)}`}</span>
                <span>{`Output: ${formatOptionalNumber(entry.metadata?.outputPrice)}`}</span>
              </div>
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
