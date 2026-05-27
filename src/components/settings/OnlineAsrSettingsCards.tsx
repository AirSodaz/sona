import React from 'react';
import { useTranslation } from 'react-i18next';
import { useModelConfig, useSetConfig } from '../../stores/configStore';
import { SettingsItem, SettingsAccordion } from './SettingsLayout';
import { Dropdown } from '../Dropdown';
import {
    VOLCENGINE_DOUBAO_PROVIDER_ID,
    GROQ_WHISPER_PROVIDER_ID,
    VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
    VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
    isVolcengineFlashBatchMode,
    getOnlineProviderConfig,
    type OnlineAsrProviderDefinition,
} from '../../services/onlineAsrProviders';
import { syncOnlineAsrProviderConfig } from '../../services/asrConfigService';

interface ProviderSettingsProps {
    provider: OnlineAsrProviderDefinition;
}

function useProviderStatus(providerId: string, isConfigured: boolean) {
    const { t } = useTranslation();
    const modelConfig = useModelConfig();
    const isEnabled = Object.values(modelConfig.asr?.selections ?? {}).some(
        (selection) => selection.engine === 'online' && selection.providerId === providerId,
    );

    if (isEnabled) {
        return { type: 'ready', text: t('settings.asr.active', { defaultValue: '已启用' }) };
    }
    if (isConfigured) {
        return { type: 'off', text: t('settings.asr.configured', { defaultValue: '已就绪' }) };
    }
    return { type: 'off', text: t('settings.asr.not_configured', { defaultValue: '未配置' }) };
}

export function VolcengineSettingsCard({ provider }: ProviderSettingsProps) {
    const { t } = useTranslation();
    const modelConfig = useModelConfig();
    const updateConfig = useSetConfig();
    const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
    const status = useProviderStatus(provider.id, provider.isConfigured(config, 'batch')); // Check if batch or streaming is configured

    const updateVolcengineConfig = (updates: Record<string, string | number | boolean>) => {
        updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, updates));
    };

    const volcengineBatchUrlOnlyUnavailable = t('settings.asr.volcengine_batch_mode_url_only_unavailable', {
        defaultValue: '需要公网音频 URL，当前本地批量导入暂不支持。',
    });

    return (
        <SettingsAccordion
            title={t(provider.titleKey, { defaultValue: provider.titleDefault })}
            status={<span className={`status-badge ${status.type}`}>{status.text}</span>}
            defaultOpen={true}
        >
            <SettingsItem
                title={t('settings.asr.api_key', { defaultValue: 'API Key' })}
                hint={t('settings.asr.api_key_hint', { defaultValue: '新版控制台的 X-Api-Key；不会写入日志。' })}
            >
                <div style={{ width: '320px' }}>
                    <input
                        id="settings-volcengine-api-key"
                        type="password"
                        className="settings-input"
                        value={config.apiKey as string}
                        onChange={(event) => updateVolcengineConfig({ apiKey: event.target.value })}
                        placeholder="X-Api-Key"
                    />
                </div>
            </SettingsItem>
            <SettingsItem
                title={t('settings.asr.volcengine_batch_mode_label', { defaultValue: 'Recording File Mode' })}
                hint={t('settings.asr.volcengine_batch_mode_hint', { defaultValue: '普通和闲时为异步任务；极速为同步直回，适合快速转录。' })}
            >
                <div style={{ width: '220px' }}>
                    <Dropdown
                        id="settings-volcengine-batch-mode"
                        value={isVolcengineFlashBatchMode(config) ? 'flash' : 'flash'}
                        onChange={(value) => {
                            if (value === 'flash') {
                                updateVolcengineConfig({
                                    batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
                                    batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
                                });
                            }
                        }}
                        options={[
                            {
                                value: 'standard',
                                label: t('settings.asr.volcengine_batch_mode_standard', { defaultValue: '普通 (异步轮询)' }),
                                description: volcengineBatchUrlOnlyUnavailable,
                                disabled: true,
                            },
                            { value: 'flash', label: t('settings.asr.volcengine_batch_mode_flash', { defaultValue: '急速 (同步直回)' }) },
                            {
                                value: 'offpeak',
                                label: t('settings.asr.volcengine_batch_mode_offpeak', { defaultValue: '闲时 (特惠异步)' }),
                                description: volcengineBatchUrlOnlyUnavailable,
                                disabled: true,
                            },
                        ]}
                    />
                </div>
            </SettingsItem>
        </SettingsAccordion>
    );
}

export function GroqWhisperSettingsCard({ provider }: ProviderSettingsProps) {
    const { t } = useTranslation();
    const modelConfig = useModelConfig();
    const updateConfig = useSetConfig();
    const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
    const status = useProviderStatus(provider.id, provider.isConfigured(config, 'batch'));

    const updateGroqConfig = (updates: Record<string, string | number | boolean>) => {
        updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, updates));
    };

    return (
        <SettingsAccordion
            title={t(provider.titleKey, { defaultValue: provider.titleDefault })}
            status={<span className={`status-badge ${status.type}`}>{status.text}</span>}
        >
            <SettingsItem
                title={t('settings.asr.api_key', { defaultValue: 'API Key' })}
                hint={t('settings.asr.api_key_hint', { defaultValue: '不写入日志' })}
            >
                <div style={{ width: '320px' }}>
                    <input
                        id="settings-groq-api-key"
                        type="password"
                        className="settings-input"
                        value={config.apiKey as string}
                        onChange={(event) => updateGroqConfig({ apiKey: event.target.value })}
                        placeholder="gsk_..."
                    />
                </div>
            </SettingsItem>
            <SettingsItem
                title={t('settings.asr.groq_model_label', { defaultValue: 'Whisper 模型' })}
            >
                <div style={{ width: '220px' }}>
                    <Dropdown
                        id="settings-groq-model"
                        value={config.model as string}
                        onChange={(value) => updateGroqConfig({ model: value })}
                        options={[
                            { value: 'whisper-large-v3-turbo', label: 'whisper-large-v3-turbo' },
                            { value: 'whisper-large-v3', label: 'whisper-large-v3' },
                        ]}
                    />
                </div>
            </SettingsItem>
        </SettingsAccordion>
    );
}

export function DynamicProviderSettings({ provider }: ProviderSettingsProps) {
    const { t } = useTranslation();
    const modelConfig = useModelConfig();
    const updateConfig = useSetConfig();
    const config = getOnlineProviderConfig(modelConfig.asr?.providers, provider.id);
    const status = useProviderStatus(provider.id, provider.isConfigured(config, 'batch'));

    const updateConfigValue = (key: string, value: string | number | boolean) => {
        updateConfig(syncOnlineAsrProviderConfig(modelConfig, provider.id, { [key]: value }));
    };

    return (
        <SettingsAccordion
            title={t(provider.titleKey, { defaultValue: provider.titleDefault })}
            status={<span className={`status-badge ${status.type}`}>{status.text}</span>}
        >
            {(provider.manifestEntry.ui.fields || []).map((field: { name: string; labelKey: string; labelDefault: string; type?: string }) => (
                <SettingsItem
                    key={field.name}
                    title={t(field.labelKey, { defaultValue: field.labelDefault })}
                >
                    <div style={{ width: '320px' }}>
                        <input
                            id={`settings-${provider.id}-${field.name}`}
                            type={field.type === 'password' ? 'password' : 'text'}
                            className="settings-input"
                            value={(config[field.name] as string) || ''}
                            onChange={(event) => updateConfigValue(field.name, event.target.value)}
                        />
                    </div>
                </SettingsItem>
            ))}
        </SettingsAccordion>
    );
}

export const CUSTOM_PROVIDER_COMPONENTS: Record<string, React.ComponentType<ProviderSettingsProps>> = {
    [VOLCENGINE_DOUBAO_PROVIDER_ID]: VolcengineSettingsCard,
    [GROQ_WHISPER_PROVIDER_ID]: GroqWhisperSettingsCard,
};
