import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';

interface SettingsModelsTabProps {
    installedModels: Set<string>;
    downloadingId: string | null;
    deletingId: string | null;
    progress: number;
    statusMessage: string;
    vadBufferSize: number;
    setVadBufferSize: (size: number) => void;
    onLoad: (model: ModelInfo) => void;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: () => void;
    isModelSelected: (model: ModelInfo) => boolean;
}

export function SettingsModelsTab({
    installedModels,
    downloadingId,
    deletingId,
    progress,
    statusMessage,
    vadBufferSize,
    setVadBufferSize,
    onLoad,
    onDelete,
    onDownload,
    onCancelDownload,
    isModelSelected
}: SettingsModelsTabProps) {
    const { t } = useTranslation();

    function renderModelSection(title: string, type: 'streaming' | 'offline' | 'punctuation' | 'vad') {
        const models = PRESET_MODELS.filter(m => m.type === type);

        return (
            <>
                <div className="settings-section-subtitle" style={{ marginTop: 30, marginBottom: 10, fontWeight: 'bold' }}>
                    {title}
                </div>
                {models.map(model => (
                    <ModelCard
                        key={model.id}
                        model={model}
                        isInstalled={installedModels.has(model.id)}
                        isSelected={isModelSelected(model)}
                        downloadingId={downloadingId}
                        deletingId={deletingId}
                        progress={downloadingId === model.id ? progress : 0}
                        statusMessage={downloadingId === model.id ? statusMessage : ''}
                        onLoad={onLoad}
                        onDelete={onDelete}
                        onDownload={onDownload}
                        onCancelDownload={onCancelDownload}
                    />
                ))}
            </>
        );
    }

    return (
        <div
            className="model-list"
            role="tabpanel"
            id="settings-panel-models"
            aria-labelledby="settings-tab-models"
            tabIndex={0}
        >
            {renderModelSection(t('settings.streaming_models'), 'streaming')}
            {renderModelSection(t('settings.offline_models'), 'offline')}
            {renderModelSection(t('settings.punctuation_models'), 'punctuation')}
            {renderModelSection(t('settings.vad_models'), 'vad')}

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
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
        </div>
    );
}
