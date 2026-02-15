import React, { Dispatch, SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '../Switch';
import { Dropdown } from '../Dropdown';
import { useDialogStore } from '../../stores/dialogStore';
import { ItnModelList } from './ItnModelList';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';

interface SettingsLocalTabProps {
    offlineModelPath: string;
    setOfflineModelPath: (path: string) => void;
    punctuationModelPath: string;
    setPunctuationModelPath: (path: string) => void;
    vadModelPath: string;
    setVadModelPath: (path: string) => void;
    ctcModelPath: string;
    setCtcModelPath: (path: string) => void;
    vadBufferSize: number;
    setVadBufferSize: (size: number) => void;
    maxConcurrent: number;
    setMaxConcurrent: (size: number) => void;
    handleBrowse: (type: 'offline' | 'punctuation' | 'vad' | 'ctc') => Promise<void>;

    // ITN Props
    itnRulesOrder: string[];
    setItnRulesOrder: Dispatch<SetStateAction<string[]>>;
    enabledITNModels: Set<string>;
    setEnabledITNModels: Dispatch<SetStateAction<Set<string>>>;
    enableITN: boolean;
    setEnableITN: (enabled: boolean) => void;
    installedITNModels: Set<string>;

    // downloadingId: string | null;
    // progress: number;
    downloads: Record<string, { progress: number; status: string }>;
    onDownloadITN: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
    installedModels: Set<string>;
}

export function SettingsLocalTab({
    offlineModelPath,
    setOfflineModelPath,
    punctuationModelPath,
    setPunctuationModelPath,
    vadModelPath,
    setVadModelPath,
    ctcModelPath,
    setCtcModelPath,
    vadBufferSize,
    setVadBufferSize,
    itnRulesOrder,
    setItnRulesOrder,
    enabledITNModels,
    setEnabledITNModels,
    enableITN,
    setEnableITN,
    installedITNModels,
    downloads,
    onDownloadITN,
    onCancelDownload,
    maxConcurrent,
    setMaxConcurrent,
    installedModels
}: SettingsLocalTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const { alert } = useDialogStore();

    const handleToggle = async (type: 'punctuation' | 'vad' | 'ctc', checked: boolean) => {
        if (!checked) {
            if (type === 'punctuation') setPunctuationModelPath('');
            else if (type === 'vad') setVadModelPath('');
            else if (type === 'ctc') setCtcModelPath('');
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
                if (type === 'punctuation') setPunctuationModelPath(path);
                else if (type === 'vad') setVadModelPath(path);
                else if (type === 'ctc') setCtcModelPath(path);
            } catch (e) {
                console.error(`Failed to get path for ${type} model`, e);
            }
        }
    };

    const [selectedOfflineModelId, setSelectedOfflineModelId] = React.useState<string>('');

    // Sync offlineModelPath with selected model ID
    React.useEffect(() => {
        const findModel = async () => {
            if (!offlineModelPath) {
                setSelectedOfflineModelId('');
                return;
            }

            // Try to find which model ID corresponds to this path
            for (const model of PRESET_MODELS) {
                if (model.type === 'offline') {
                    const path = await modelService.getModelPath(model.id);
                    // Simple check if paths match (might need normalization in real world, but strict equality for now)
                    // On Windows, paths might differ by slashes, but usually consistency is maintained if set via the same service.
                    if (path === offlineModelPath) {
                        setSelectedOfflineModelId(model.id);
                        return;
                    }
                }
            }
            // If no match found (e.g. custom path), set to empty or handle gracefully
            // For now, we leave it empty which will show "Select a model..." or we could add a "Custom" option logic later.
            setSelectedOfflineModelId('');
        };
        findModel();
    }, [offlineModelPath]);

    const handleOfflineModelChange = async (modelId: string) => {
        setSelectedOfflineModelId(modelId);
        try {
            const path = await modelService.getModelPath(modelId);
            setOfflineModelPath(path);
        } catch (e) {
            console.error('Failed to get offline model path', e);
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

            <div className="settings-item" style={{ marginTop: 16 }}>
                <label htmlFor="settings-offline-path" className="settings-label">{t('settings.offline_path_label', { defaultValue: 'Recognition Model' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <Dropdown
                        id="settings-offline-path"
                        value={selectedOfflineModelId}
                        onChange={(value) => handleOfflineModelChange(value)}
                        placeholder={t('settings.select_model', { defaultValue: 'Select a model...' })}
                        options={PRESET_MODELS.filter(m => m.type === 'offline').map(model => ({
                            value: model.id,
                            label: `${model.name}${!installedModels.has(model.id) ? t('settings.not_installed', { defaultValue: ' (Not Downloaded)' }) : ''}`,
                            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } : undefined
                        }))}
                        style={{ flex: 1 }}
                    />
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 16 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <label className="settings-label" style={{ marginBottom: 0 }}>{t('settings.punctuation_path_label')}</label>
                    <Switch
                        checked={!!punctuationModelPath}
                        onChange={(c) => handleToggle('punctuation', c)}
                    />
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 16 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <label className="settings-label" style={{ marginBottom: 0 }}>{t('settings.vad_path_label', { defaultValue: 'VAD Model' })}</label>
                    <Switch
                        checked={!!vadModelPath}
                        onChange={(c) => handleToggle('vad', c)}
                    />
                </div>
            </div>



            <div className="settings-item" style={{ marginTop: 16 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <label className="settings-label" style={{ marginBottom: 0 }}>{t('settings.ctc_path_label', { defaultValue: 'CTC Model' })}</label>
                    <Switch
                        checked={!!ctcModelPath}
                        onChange={(c) => handleToggle('ctc', c)}
                    />
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 16 }}>
                <label htmlFor="settings-vad-buffer" className="settings-label">{t('settings.vad_buffer_size')}</label>
                <div style={{ maxWidth: 300 }}>
                    <input
                        id="settings-vad-buffer"
                        type="number"
                        className="settings-input"
                        value={vadBufferSize}
                        onChange={(e) => setVadBufferSize(Number(e.target.value))}
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

            <div className="settings-item" style={{ marginTop: 16 }}>
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
                                setMaxConcurrent(val);
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

            <div className="settings-item" style={{ marginTop: 24, borderTop: '1px solid var(--color-border)', paddingTop: 24 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                        <div style={{ fontWeight: 500, marginBottom: 4 }}>{t('settings.enable_itn', { defaultValue: 'Enable ITN' })}</div>
                        <div style={{ fontSize: '0.875rem', color: 'var(--color-text-muted)' }}>{t('settings.itn_title')}</div>
                    </div>
                    <Switch
                        checked={enableITN}
                        onChange={(c) => setEnableITN(c)}
                    />
                </div>
            </div>

            <ItnModelList
                itnRulesOrder={itnRulesOrder}
                setItnRulesOrder={setItnRulesOrder}
                enabledITNModels={enabledITNModels}
                setEnabledITNModels={setEnabledITNModels}
                installedITNModels={installedITNModels}
                downloads={downloads}
                // downloadingId={downloadingId}
                // progress={progress}
                onDownload={onDownloadITN}
                onCancelDownload={onCancelDownload}
            />
        </div >
    );
}
