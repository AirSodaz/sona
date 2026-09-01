import React, { useState, useMemo } from 'react';
import { Check, Loader2, Pencil, Trash2, X } from 'lucide-react';
import { LlmProvider, LlmProviderSetting } from '../../../types/transcript';
import { LlmAssistantConfig } from '../../../types/config';
import type { LlmGenerateCommandRequest } from '../../../types/dashboard';
import { normalizeError } from '../../../utils/errorUtils';
import {
  buildLlmConfig,
  createProviderSetting,
  getProviderDefinition,
} from '../../../services/llm/providers';
import { generateLlmText } from '../../../services/tauri/llm';
import { getCurrentLlmSettings, getModelPlaceholder, isProviderConfiguredForConfig } from './helpers';
import { SettingsAccordion, SettingsItem } from '../SettingsLayout';

interface ProviderAccordionItemProps {
  provider: LlmProvider;
  config: LlmAssistantConfig;
  isOpen: boolean;
  onToggle: () => void;
  applyProviderUpdates: (updates: Partial<LlmProviderSetting>) => void;
  onOpenDetails?: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

export const ProviderAccordionItem = React.memo(function ProviderAccordionItem({
  provider,
  config,
  isOpen,
  onToggle,
  applyProviderUpdates,
  onOpenDetails,
  onEdit,
  onDelete,
  t,
}: ProviderAccordionItemProps) {
  const currentLlmState = getCurrentLlmSettings(config);
  const def = getProviderDefinition(provider, currentLlmState.customProviders);
  const setting = currentLlmState.providers[provider];
  const isConfigured = isProviderConfiguredForConfig(config, provider, setting);
  const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [testMessage, setTestMessage] = useState('');

  const handleTestConnection = async () => {
    const effectiveSetting = setting || createProviderSetting(provider, currentLlmState.customProviders);
    setTestStatus('loading');
    setTestMessage('');
    try {
      const providerConfig = buildLlmConfig(provider, effectiveSetting, currentLlmState.customProviders);
      const entryId = currentLlmState.modelOrder.find(id => currentLlmState.models[id].provider === provider);
      const testModel = entryId ? currentLlmState.models[entryId].model : getModelPlaceholder(provider);
      const testProviderConfig = { ...providerConfig, model: testModel };

      await generateLlmText({
        config: testProviderConfig,
        input: 'Hello, this is a connection test.',
        source: 'connection_test',
      } satisfies LlmGenerateCommandRequest);
      setTestStatus('success');
      setTestMessage(testModel);
      setTimeout(() => {
        setTestStatus('idle');
        setTestMessage('');
      }, 3000);
    } catch (error) {
      setTestStatus('error');
      setTestMessage(normalizeError(error).message);
    }
  };
  const status = useMemo(() => {
    if (isConfigured) {
      return { type: 'ready', text: t('settings.llm.status_ready', { defaultValue: '已就绪' }) };
    }
    if (def.requiresApiKey) {
      return { type: 'missing', text: t('settings.llm.status_missing_api_key', { defaultValue: '缺少 API Key' }) };
    }
    return { type: 'off', text: t('settings.llm.status_off', { defaultValue: '未配置' }) };
  }, [isConfigured, def.requiresApiKey, t]);

  const actions = (onEdit || onDelete) ? (
    <div className="provider-header-actions" onClick={(e) => e.stopPropagation()}>
      {onEdit && (
        <button
          type="button"
          className="btn btn-icon btn-secondary-soft"
          aria-label={t('settings.llm.edit_provider', { defaultValue: 'Edit provider' })}
          onClick={onEdit}
        >
          <Pencil size={14} />
        </button>
      )}
      {onDelete && (
        <button
          type="button"
          className="btn btn-icon btn-secondary-soft"
          aria-label={t('settings.llm.delete_provider', { defaultValue: 'Delete provider' })}
          onClick={onDelete}
        >
          <Trash2 size={14} />
        </button>
      )}
    </div>
  ) : undefined;

  return (
    <SettingsAccordion
      title={t(def.labelKey, { defaultValue: def.labelDefault })}
      status={<span className={`status-badge ${status.type}`}>{status.text}</span>}
      actions={actions}
      isOpen={isOpen}
      onToggle={onToggle}
      contentTestId={`provider-accordion-content-${provider}`}
    >
      {def.id === 'google_translate_free' ? (
        <div className="settings-hint provider-free-hint">
          {t('settings.llm.free_service_hint')}
        </div>
      ) : (
        <>
          <SettingsItem
            title={def.apiHostLabelKey ? t(def.apiHostLabelKey, { defaultValue: def.apiHostLabelDefault }) : t('settings.llm.base_url')}
          >
            <div style={{ width: '320px' }}>
              {def.editableApiHost === false ? (
                <input
                  id={`llm-${def.id}-host`}
                  type="text"
                  className="settings-input"
                  value={setting?.apiHost || def.defaultApiHost}
                  readOnly
                  disabled
                />
              ) : (
                <input
                  id={`llm-${def.id}-host`}
                  type="text"
                  className="settings-input"
                  value={setting?.apiHost || ''}
                  onChange={(e) => applyProviderUpdates({ apiHost: e.target.value })}
                  placeholder={def.defaultApiHost}
                />
              )}
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('settings.llm.api_key')}
          >
            <div style={{ width: '320px' }}>
              <input
                id={`llm-${def.id}-key`}
                type="password"
                className="settings-input"
                value={setting?.apiKey || ''}
                onChange={(e) => applyProviderUpdates({ apiKey: e.target.value })}
                placeholder={def.requiresApiKey ? 'sk-...' : t('settings.llm.optional_api_key')}
              />
            </div>
          </SettingsItem>

          {setting?.apiVersion !== undefined && (
            <SettingsItem
              title={t('settings.llm.api_version')}
            >
              <div style={{ width: '320px' }}>
                <input
                  id={`llm-${def.id}-version`}
                  type="text"
                  className="settings-input"
                  value={setting.apiVersion}
                  onChange={(e) => applyProviderUpdates({ apiVersion: e.target.value })}
                  placeholder={def.defaultApiVersion || ''}
                />
              </div>
            </SettingsItem>
          )}

          {setting?.apiPath !== undefined && (
            <SettingsItem
              title={t('settings.llm.api_path')}
            >
              <div style={{ width: '320px' }}>
                <input
                  id={`llm-${def.id}-path`}
                  type="text"
                  className="settings-input"
                  value={setting.apiPath}
                  onChange={(e) => applyProviderUpdates({ apiPath: e.target.value })}
                  readOnly={def.editableApiHost === false || provider === 'open_ai_responses' || provider === 'volcengine' || provider === 'perplexity'}
                />
              </div>
            </SettingsItem>
          )}

          {def.id !== 'google_translate' && (
            <SettingsItem
              title={t('settings.llm.models_management_title', { defaultValue: '模型管理' })}
              hint={t('settings.llm.models_management_hint', { defaultValue: '管理和测试该供应商下的模型列表' })}
            >
              <button
                type="button"
                className="btn btn-secondary btn-loading-wrapper"
                style={{ width: 'fit-content', minWidth: '120px' }}
                onClick={onOpenDetails}
                disabled={!onOpenDetails}
              >
                <div className="btn-content-inner" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
                  <span>{t('settings.llm.details')}</span>
                </div>
              </button>
            </SettingsItem>
          )}

          {def.id === 'google_translate' && (
            <SettingsItem
              title={t('settings.llm.test_connection_title', { defaultValue: '连接测试' })}
            >
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: '8px' }}>
                {(() => {
                  let testBtnClass = 'btn-secondary';
                  let icon = null;
                  let label = t('settings.llm.test_connection');

                  if (testStatus === 'loading') {
                    icon = <Loader2 className="animate-spin" size={16} />;
                    label = t('settings.llm.testing');
                  } else if (testStatus === 'success') {
                    testBtnClass = 'btn-success-flash';
                    icon = <Check size={16} />;
                    label = t('settings.llm.connection_success');
                  } else if (testStatus === 'error') {
                    testBtnClass = 'btn-error-flash';
                    icon = <X size={16} />;
                    label = t('settings.llm.connection_failed');
                  }

                  return (
                    <button
                      type="button"
                      className={`btn ${testBtnClass} btn-loading-wrapper`}
                      style={{ width: 'fit-content', minWidth: '120px' }}
                      onClick={handleTestConnection}
                      disabled={testStatus === 'loading'}
                    >
                      <div className="btn-content-inner" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
                        {icon}
                        <span>{label}</span>
                      </div>
                    </button>
                  );
                })()}

                {testStatus === 'error' && testMessage && (
                  <div className="connection-error-detail">
                    <X size={12} />
                    <span>{testMessage}</span>
                  </div>
                )}
              </div>
            </SettingsItem>
          )}
        </>
      )}
    </SettingsAccordion>
  );
});
