import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '../Switch';
import { useDialogStore } from '../../stores/dialogStore';
import { ItnModelList } from './ItnModelList';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';
import { RestoreIcon } from '../Icons';
import { AppConfig } from '../../types/transcript';

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
    const { alert } = useDialogStore();

    const ctcModelPath = config.ctcModelPath || '';
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


    const handleToggle = async (type: 'punctuation' | 'vad' | 'ctc', checked: boolean) => {
        if (!checked) {
            if (type === 'punctuation') updateConfig({ punctuationModelPath: '' });
            else if (type === 'vad') updateConfig({ vadModelPath: '' });
            else if (type === 'ctc') updateConfig({ ctcModelPath: '' });
            return;
        }

        const model = PRESET_MODELS.find(m => m.type === type);
        if (model) {
            if (!installedModels.has(model.id)) {
                await alert(t('settings.model_not_installed', { defaultValue: 'Please download the model first from the Model Hub.' }));
                return;
            }

            try {
                const path = await modelService.getModelPath(model.id);
                if (type === 'punctuation') updateConfig({ punctuationModelPath: path });
                else if (type === 'vad') updateConfig({ vadModelPath: path });
                else if (type === 'ctc') updateConfig({ ctcModelPath: path });
            } catch (e) {
                console.error(`Failed to get path for ${type} model`, e);
            }
        }
    };

    return (
        <div
            className="settings-group"
            role="tabpanel"
            id="settings-panel-local"
            aria-labelledby="settings-tab-local"
            tabIndex={0}
        >
            <div className="settings-item">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <label className="settings-label" style={{ marginBottom: 0 }}>{t('settings.ctc_path_label', { defaultValue: 'CTC Model' })}</label>
                    <Switch
                        checked={!!ctcModelPath}
                        onChange={(c) => handleToggle('ctc', c)}
                    />
                </div>
            </div>

            <div className="settings-item">
                <label htmlFor="settings-vad-buffer" className="settings-label">{t('settings.vad_buffer_size')}</label>
                <div style={{ maxWidth: 300 }}>
                    <input
                        id="settings-vad-buffer"
                        type="number"
                        className="settings-input"
                        value={vadBufferSize}
                        onChange={(e) => updateConfig({ vadBufferSize: Number(e.target.value) })}
                        min={0}
                        max={30}
                        step={0.5}
                        style={{ width: '100%' }}
                    />
                </div>
                <div className="settings-hint">
                    {t('settings.vad_buffer_hint')}
                </div>
            </div>

            <div className="settings-item">
                <label htmlFor="settings-max-concurrent" className="settings-label">{t('settings.max_concurrent_label', { defaultValue: 'Max Concurrent Transcriptions' })}</label>
                <div style={{ maxWidth: 300 }}>
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
                        step={1}
                        style={{ width: '100%' }}
                    />
                </div>
                <div className="settings-hint">
                    {t('settings.max_concurrent_hint', { defaultValue: 'Number of files to transcribe in parallel (1-4).' })}
                </div>
            </div>

            <div className="settings-item with-divider">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                        <div style={{ fontWeight: 500 }}>{t('settings.enable_itn', { defaultValue: 'Enable ITN' })}</div>
                        <div className="settings-hint">{t('settings.itn_title')}</div>
                    </div>
                    <Switch
                        checked={enableITN}
                        onChange={(c) => updateConfig({ enableITN: c })}
                    />
                </div>
            </div>

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

            <div className="settings-item with-divider">
                <button
                    className="btn btn-restore-defaults"
                    onClick={onRestoreDefaults}
                    aria-label={t('settings.restore_defaults')}
                >
                    <RestoreIcon />
                    {t('settings.restore_defaults')}
                </button>
            </div>
        </div >
    );
}
