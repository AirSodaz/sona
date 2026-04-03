import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { HardDrive, PlaySquare } from 'lucide-react';
import { Switch } from '../Switch';
import { ItnModelList } from './ItnModelList';
import { ModelInfo } from '../../services/modelService';
import { RestoreIcon } from '../Icons';
import { AppConfig } from '../../types/transcript';
import { SettingsTabContainer, SettingsSection, SettingsItem } from './SettingsLayout';

interface SettingsLocalTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    downloads: Record<string, { progress: number; status: string }>;
    onDownloadITN: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
    installedModels: Set<string>;
    onRestoreDefaults: () => void;
}

export function SettingsLocalTab({
    config,
    updateConfig,
    downloads,
    onDownloadITN,
    onCancelDownload,
    installedModels,
    onRestoreDefaults
}: SettingsLocalTabProps): React.JSX.Element {
    const { t } = useTranslation();

    const vadBufferSize = config.vadBufferSize || 5;
    const maxConcurrent = config.maxConcurrent || 2;
    const enableITN = config.enableITN ?? true;
    const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];

    const enabledITNModels = useMemo(() => new Set(config.enabledITNModels || []), [config.enabledITNModels]);

    const setEnabledITNModels = (action: React.SetStateAction<Set<string>>) => {
        const currentSet = new Set(config.enabledITNModels || []);
        const newSet = typeof action === 'function'
            ? (action as (prev: Set<string>) => Set<string>)(currentSet)
            : action;

        updateConfig({
            enabledITNModels: Array.from(newSet)
        });
    };

    const setItnRulesOrder = (action: React.SetStateAction<string[]>) => {
        const currentOrder = config.itnRulesOrder || ['itn-zh-number'];
        const newOrder = typeof action === 'function'
            ? (action as (prev: string[]) => string[])(currentOrder)
            : action;
        updateConfig({ itnRulesOrder: newOrder });
    };

    return (
        <SettingsTabContainer id="settings-panel-local" ariaLabelledby="settings-tab-local">
            <SettingsSection
                title={t('settings.transcription_settings', { defaultValue: 'Transcription Settings' })}
                icon={<PlaySquare size={20} />}
                description={t('settings.transcription_settings_hint', { defaultValue: 'Configure local processing behavior.' })}
            >
                <SettingsItem
                    title={t('settings.vad_buffer_size')}
                    hint={t('settings.vad_buffer_hint')}
                >
                    <div style={{ width: '120px' }}>
                        <input
                            id="settings-vad-buffer"
                            type="number"
                            className="settings-input"
                            value={vadBufferSize}
                            onChange={(e) => updateConfig({ vadBufferSize: Number(e.target.value) })}
                            min={0}
                            max={30}
                            step={0.5}
                            style={{ textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.max_concurrent_label', { defaultValue: 'Max Concurrent Transcriptions' })}
                    hint={t('settings.max_concurrent_hint', { defaultValue: 'Number of files to transcribe in parallel (1-4).' })}
                >
                    <div style={{ width: '120px' }}>
                        <input
                            id="settings-max-concurrent"
                            type="number"
                            className="settings-input"
                            value={maxConcurrent}
                            onChange={(e) => {
                                const val = Number(e.target.value);
                                if (val > 0) {
                                    updateConfig({ maxConcurrent: val });
                                }
                            }}
                            min={1}
                            max={4}
                            step={1}
                            style={{ textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <SettingsSection
                title={t('settings.itn_title', { defaultValue: 'Inverse Text Normalization (ITN)' })}
                icon={<HardDrive size={20} />}
                description={t('settings.itn_description', { defaultValue: 'Convert spoken numbers and formats into standardized written forms.' })}
            >
                <SettingsItem
                    title={t('settings.enable_itn', { defaultValue: 'Enable ITN' })}
                    hint={t('settings.enable_itn_hint', { defaultValue: 'Apply normalization rules globally.' })}
                >
                    <Switch
                        checked={enableITN}
                        onChange={(c) => updateConfig({ enableITN: c })}
                    />
                </SettingsItem>

                <div style={{ padding: '0 24px 24px 24px', background: 'var(--color-bg-primary)' }}>
                    <ItnModelList
                        itnRulesOrder={itnRulesOrder}
                        setItnRulesOrder={setItnRulesOrder}
                        enabledITNModels={enabledITNModels}
                        setEnabledITNModels={setEnabledITNModels}
                        installedITNModels={installedModels}
                        downloads={downloads}
                        onDownload={onDownloadITN}
                        onCancelDownload={onCancelDownload}
                    />
                </div>
            </SettingsSection>

            <div style={{ display: 'flex', justifyContent: 'flex-start', marginTop: '8px' }}>
                <button
                    className="btn btn-restore-defaults"
                    onClick={onRestoreDefaults}
                    aria-label={t('settings.restore_defaults')}
                >
                    <RestoreIcon />
                    {t('settings.restore_defaults')}
                </button>
            </div>
        </SettingsTabContainer>
    );
}
