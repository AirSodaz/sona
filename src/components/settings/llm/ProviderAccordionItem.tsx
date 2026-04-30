import { useState } from 'react';
import { Check, ChevronDown, ChevronRight, Loader2, X } from 'lucide-react';
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
import { getCurrentLlmSettings, getModelPlaceholder, isProviderConfigured } from './helpers';

interface ProviderAccordionItemProps {
  provider: LlmProvider;
  config: LlmAssistantConfig;
  isOpen: boolean;
  onToggle: () => void;
  applyProviderUpdates: (updates: Partial<LlmProviderSetting>) => void;
  t: (key: string) => string;
}

export function ProviderAccordionItem({
  provider,
  config,
  isOpen,
  onToggle,
  applyProviderUpdates,
  t,
}: ProviderAccordionItemProps) {
  const currentLlmState = getCurrentLlmSettings(config);
  const def = getProviderDefinition(provider);
  const setting = currentLlmState.providers[provider];
  const isConfigured = isProviderConfigured(provider, setting);
  const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [testMessage, setTestMessage] = useState('');

  const handleTestConnection = async () => {
    const effectiveSetting = setting || createProviderSetting(provider);
    setTestStatus('loading');
    setTestMessage('');
    try {
      const providerConfig = buildLlmConfig(provider, effectiveSetting);
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

  return (
    <div className="accordion-item">
      <div className="accordion-header" onClick={onToggle}>
        <div className="accordion-title-container">
          {isOpen ? <ChevronDown size={18} /> : <ChevronRight size={18} />}
          <span>{def.label}</span>
        </div>
        <div className="accordion-header-status">
          {isConfigured && (
            <span className="status-badge ready"><Check size={12}/> {t('settings.llm.status_ready')}</span>
          )}
          {!isConfigured && def.requiresApiKey && (
             <span className="status-badge missing"><X size={12}/> {t('settings.llm.status_missing_api_key')}</span>
          )}
        </div>
      </div>
      {isOpen && (
        <div className="accordion-content" data-testid={`provider-accordion-content-${provider}`}>
          {def.id === 'google_translate_free' ? (
            <div className="settings-hint provider-free-hint">
              {t('settings.llm.free_service_hint')}
            </div>
          ) : (
            <>
              <div className="settings-item">
                <label className="settings-label" htmlFor={`llm-${def.id}-host`}>{def.apiHostLabel || t('settings.llm.base_url')}</label>
                {def.editableApiHost === false ? (
                  <div className="settings-input provider-readonly-field" id={`llm-${def.id}-host`}>
                    {setting?.apiHost || def.defaultApiHost}
                  </div>
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

              <div className="settings-item">
                <label className="settings-label" htmlFor={`llm-${def.id}-key`}>{t('settings.llm.api_key')}</label>
                <input
                  id={`llm-${def.id}-key`}
                  type="password"
                  className="settings-input"
                  value={setting?.apiKey || ''}
                  onChange={(e) => applyProviderUpdates({ apiKey: e.target.value })}
                  placeholder={def.requiresApiKey ? 'sk-...' : t('settings.llm.optional_api_key')}
                />
              </div>

              {setting?.apiVersion !== undefined && (
                <div className="settings-item">
                  <label className="settings-label" htmlFor={`llm-${def.id}-version`}>{t('settings.llm.api_version')}</label>
                  <input
                    id={`llm-${def.id}-version`}
                    type="text"
                    className="settings-input"
                    value={setting.apiVersion}
                    onChange={(e) => applyProviderUpdates({ apiVersion: e.target.value })}
                    placeholder={def.defaultApiVersion || ''}
                  />
                </div>
              )}

              {setting?.apiPath !== undefined && (
                <div className="settings-item">
                  <label className="settings-label" htmlFor={`llm-${def.id}-path`}>{t('settings.llm.api_path')}</label>
                  <input
                    id={`llm-${def.id}-path`}
                    type="text"
                    className="settings-input"
                    value={setting.apiPath}
                    onChange={(e) => applyProviderUpdates({ apiPath: e.target.value })}
                    readOnly={provider === 'open_ai_responses' || provider === 'volcengine' || provider === 'perplexity'}
                  />
                </div>
              )}
            </>
          )}

          <div className="feature-field provider-test-actions">
            <div className="provider-test-stack">
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
          </div>
        </div>
      )}
    </div>
  );
}
