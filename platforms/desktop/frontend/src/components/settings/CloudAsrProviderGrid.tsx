import { ChevronRight, Globe, Radio } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { syncOnlineAsrProviderConfig } from '../../services/asrConfigService';
import {
  getOnlineProviderConfig,
  isVolcengineFlashBatchMode,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  type OnlineAsrProviderDefinition,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
} from '../../services/onlineAsrProviders';
import { useModelConfig, useSetConfig } from '../../stores/configStore';
import { Dropdown } from '../Dropdown';
import { ModelBrandLogo } from '../icons/ModelLogos';
import { SettingsItem } from './SettingsLayout';

type ProviderStatus = 'active' | 'configured' | 'unconfigured';

function useProviderStatuses(): Map<string, ProviderStatus> {
  const modelConfig = useModelConfig();

  return useMemo(() => {
    const statuses = new Map<string, ProviderStatus>();
    const activeProviderIds = new Set(
      Object.values(modelConfig.asr?.selections ?? {})
        .filter((s) => s.engine === 'online')
        .map((s) => s.providerId)
        .filter(Boolean)
    );

    for (const provider of ONLINE_ASR_PROVIDER_DEFINITIONS) {
      const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
      const isActive = activeProviderIds.has(provider.id);
      const isConfiguredBatch = provider.isConfigured(config, 'batch');
      const isConfiguredStreaming =
        provider.manifestEntry.streaming?.supported !== false &&
        provider.isConfigured(config, 'streaming');

      if (isActive) {
        statuses.set(provider.id, 'active');
      } else if (isConfiguredBatch || isConfiguredStreaming) {
        statuses.set(provider.id, 'configured');
      } else {
        statuses.set(provider.id, 'unconfigured');
      }
    }
    return statuses;
  }, [modelConfig.asr?.selections, modelConfig.asr?.providers]);
}

function ProviderStatusIndicator({ status }: { status: ProviderStatus }) {
  return <span className={`cloud-provider-status-indicator ${status}`} aria-hidden="true" />;
}

function ProviderCapabilityBadges({
  provider,
  t,
}: {
  provider: OnlineAsrProviderDefinition;
  t: (key: string, opts?: Record<string, unknown>) => string;
}) {
  const supportsStreaming = provider.manifestEntry.streaming?.supported !== false;
  const langCount = provider.manifestEntry.languages?.length ?? 0;

  return (
    <span className="cloud-provider-badges">
      {supportsStreaming && (
        <span
          className="cloud-provider-badge streaming"
          data-tooltip={t('settings.asr.supports_streaming', {
            defaultValue: 'Supports real-time streaming',
          })}
          data-tooltip-pos="top"
        >
          <Radio size={11} />
          {t('settings.asr.streaming_badge', { defaultValue: 'Streaming' })}
        </span>
      )}
      {langCount > 0 && (
        <span className="cloud-provider-badge lang">
          <Globe size={11} />
          {langCount}
        </span>
      )}
    </span>
  );
}

interface ProviderConfigPanelProps {
  provider: OnlineAsrProviderDefinition;
}

function VolcengineConfigPanel({ provider }: ProviderConfigPanelProps) {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();
  const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);

  const update = (updates: Record<string, string | number | boolean>) => {
    updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, updates));
  };

  const batchUnavailableMsg = t('settings.asr.volcengine_batch_mode_url_only_unavailable', {
    defaultValue: '需要公网音频 URL，当前本地批量导入暂不支持。',
  });

  return (
    <>
      <SettingsItem
        title={t('settings.asr.api_key', { defaultValue: 'API Key' })}
        hint={t('settings.asr.volcengine_api_key_hint', {
          defaultValue: '新版控制台的 X-Api-Key；不会写入日志。',
        })}
      >
        <div style={{ width: '280px' }}>
          <input
            id="settings-volcengine-api-key"
            type="password"
            className="settings-input"
            value={config.apiKey as string}
            onChange={(e) => update({ apiKey: e.target.value })}
            placeholder="X-Api-Key"
          />
        </div>
      </SettingsItem>
      <SettingsItem
        title={t('settings.asr.volcengine_batch_mode_label', {
          defaultValue: 'Recording File Mode',
        })}
        hint={t('settings.asr.volcengine_batch_mode_hint', {
          defaultValue: '普通和闲时为异步任务；极速为同步直回，适合快速转录。',
        })}
      >
        <div style={{ width: '200px' }}>
          <Dropdown
            id="settings-volcengine-batch-mode"
            value={isVolcengineFlashBatchMode(config) ? 'flash' : 'flash'}
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
                value: 'standard',
                label: t('settings.asr.volcengine_batch_mode_standard', {
                  defaultValue: '普通 (异步轮询)',
                }),
                description: batchUnavailableMsg,
                disabled: true,
              },
              {
                value: 'flash',
                label: t('settings.asr.volcengine_batch_mode_flash', {
                  defaultValue: '急速 (同步直回)',
                }),
              },
              {
                value: 'offpeak',
                label: t('settings.asr.volcengine_batch_mode_offpeak', {
                  defaultValue: '闲时 (特惠异步)',
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

function GroqConfigPanel({ provider }: ProviderConfigPanelProps) {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();
  const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);

  const update = (updates: Record<string, string | number | boolean>) => {
    updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, updates));
  };

  return (
    <>
      <SettingsItem
        title={t('settings.asr.api_key', { defaultValue: 'API Key' })}
        hint={t('settings.asr.api_key_hint', { defaultValue: '不会写入日志。' })}
      >
        <div style={{ width: '280px' }}>
          <input
            id="settings-groq-api-key"
            type="password"
            className="settings-input"
            value={config.apiKey as string}
            onChange={(e) => update({ apiKey: e.target.value })}
            placeholder="gsk_..."
          />
        </div>
      </SettingsItem>
      <SettingsItem title={t('settings.asr.groq_model_label', { defaultValue: 'Whisper 模型' })}>
        <div style={{ width: '200px' }}>
          <Dropdown
            id="settings-groq-model"
            value={config.model as string}
            onChange={(value) => update({ model: value })}
            options={[
              { value: 'whisper-large-v3-turbo', label: 'whisper-large-v3-turbo' },
              { value: 'whisper-large-v3', label: 'whisper-large-v3' },
            ]}
          />
        </div>
      </SettingsItem>
    </>
  );
}

function GenericConfigPanel({ provider }: ProviderConfigPanelProps) {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();
  const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);

  const update = (key: string, value: string) => {
    updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, { [key]: value }));
  };

  return (
    <>
      {(provider.manifestEntry.ui.fields || []).map(
        (field: { name: string; labelKey: string; labelDefault: string; type?: string }) => (
          <SettingsItem
            key={field.name}
            title={t(field.labelKey, { defaultValue: field.labelDefault })}
          >
            <div style={{ width: '280px' }}>
              <input
                id={`settings-${provider.id}-${field.name}`}
                type={field.type === 'password' ? 'password' : 'text'}
                className="settings-input"
                value={(config[field.name] as string) || ''}
                onChange={(e) => update(field.name, e.target.value)}
              />
            </div>
          </SettingsItem>
        )
      )}
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
  const statuses = useProviderStatuses();

  const handleToggle = useCallback((providerId: string) => {
    setExpandedId((prev) => (prev === providerId ? null : providerId));
  }, []);

  // Sort: active first, then configured, then unconfigured
  const sortedProviders = useMemo(() => {
    const order: Record<ProviderStatus, number> = { active: 0, configured: 1, unconfigured: 2 };
    return [...ONLINE_ASR_PROVIDER_DEFINITIONS].sort(
      (a, b) =>
        (order[statuses.get(a.id) ?? 'unconfigured'] ?? 2) -
        (order[statuses.get(b.id) ?? 'unconfigured'] ?? 2)
    );
  }, [statuses]);

  return (
    <>
      {sortedProviders.map((provider) => {
        const status = statuses.get(provider.id) ?? 'unconfigured';
        const isExpanded = expandedId === provider.id;
        const displayName = t(provider.titleKey, { defaultValue: provider.titleDefault });
        const ConfigPanel = CUSTOM_CONFIG_PANELS[provider.id] || GenericConfigPanel;

        const statusLabel =
          status === 'active'
            ? t('settings.asr.active', { defaultValue: '使用中' })
            : status === 'configured'
              ? t('settings.asr.configured', { defaultValue: '已配置' })
              : t('settings.asr.not_configured', { defaultValue: '未配置' });

        return (
          <div
            key={provider.id}
            className={`cloud-provider-card ${status}${isExpanded ? ' expanded' : ''}`}
            role="listitem"
          >
            <button
              type="button"
              className="cloud-provider-card-header"
              onClick={() => handleToggle(provider.id)}
              aria-expanded={isExpanded}
              aria-label={`${displayName} — ${statusLabel}`}
            >
              <div className="cloud-provider-card-left">
                <ChevronRight
                  size={16}
                  className={`cloud-provider-chevron ${isExpanded ? 'open' : ''}`}
                />
                <span className="cloud-provider-logo">
                  <ModelBrandLogo model={{ id: provider.id, name: displayName }} size={32} />
                </span>
                <div className="cloud-provider-card-info">
                  <span className="cloud-provider-card-name">{displayName}</span>
                  <ProviderCapabilityBadges provider={provider} t={t} />
                </div>
              </div>
              <div className="cloud-provider-card-status">
                <span className={`cloud-provider-status-label ${status}`}>{statusLabel}</span>
                <ProviderStatusIndicator status={status} />
              </div>
            </button>

            {isExpanded && (
              <div className="cloud-provider-config-panel">
                <ConfigPanel provider={provider} />
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}
