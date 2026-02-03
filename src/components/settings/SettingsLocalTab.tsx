import { Dispatch, SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderIcon } from '../Icons';
import { ItnModelList } from './ItnModelList';

interface SettingsLocalTabProps {
    streamingModelPath: string;
    setStreamingModelPath: (path: string) => void;
    offlineModelPath: string;
    setOfflineModelPath: (path: string) => void;
    punctuationModelPath: string;
    setPunctuationModelPath: (path: string) => void;
    vadModelPath: string;
    setVadModelPath: (path: string) => void;
    handleBrowse: (type: 'streaming' | 'offline' | 'punctuation' | 'vad') => Promise<void>;

    // ITN Props
    itnRulesOrder: string[];
    setItnRulesOrder: Dispatch<SetStateAction<string[]>>;
    enabledITNModels: Set<string>;
    setEnabledITNModels: Dispatch<SetStateAction<Set<string>>>;
    installedITNModels: Set<string>;
    downloadingId: string | null;
    progress: number;
    onDownloadITN: (id: string) => void;
    onCancelDownload: () => void;
}

export function SettingsLocalTab({
    streamingModelPath,
    setStreamingModelPath,
    offlineModelPath,
    setOfflineModelPath,
    punctuationModelPath,
    setPunctuationModelPath,
    vadModelPath,
    setVadModelPath,
    handleBrowse,

    itnRulesOrder,
    setItnRulesOrder,
    enabledITNModels,
    setEnabledITNModels,
    installedITNModels,
    downloadingId,
    progress,
    onDownloadITN,
    onCancelDownload
}: SettingsLocalTabProps) {
    const { t } = useTranslation();

    return (
        <div
            className="settings-group"
            role="tabpanel"
            id="settings-panel-local"
            aria-labelledby="settings-tab-local"
            tabIndex={0}
        >
            <div className="settings-item">
                <label htmlFor="settings-streaming-path" className="settings-label">{t('settings.streaming_path_label', { defaultValue: 'Streaming Model Path' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <input
                        id="settings-streaming-path"
                        type="text"
                        title={streamingModelPath}
                        className="settings-input"
                        value={streamingModelPath}
                        onChange={(e) => setStreamingModelPath(e.target.value)}
                        placeholder={t('settings.path_placeholder')}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('streaming')}
                        aria-label={t('settings.browse')}
                    >
                        <FolderIcon />
                    </button>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 16 }}>
                <label htmlFor="settings-offline-path" className="settings-label">{t('settings.offline_path_label', { defaultValue: 'Offline Model Path' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <input
                        id="settings-offline-path"
                        type="text"
                        title={offlineModelPath}
                        className="settings-input"
                        value={offlineModelPath}
                        onChange={(e) => setOfflineModelPath(e.target.value)}
                        placeholder={t('settings.path_placeholder')}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('offline')}
                        aria-label={t('settings.browse')}
                    >
                        <FolderIcon />
                    </button>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 16 }}>
                <label htmlFor="settings-punctuation-path" className="settings-label">{t('settings.punctuation_path_label')}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <input
                        id="settings-punctuation-path"
                        type="text"
                        title={punctuationModelPath}
                        className="settings-input"
                        value={punctuationModelPath}
                        onChange={(e) => setPunctuationModelPath(e.target.value)}
                        placeholder={t('settings.path_placeholder')}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('punctuation')}
                        aria-label={t('settings.browse')}
                    >
                        <FolderIcon />
                    </button>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 16 }}>
                <label htmlFor="settings-vad-path" className="settings-label">{t('settings.vad_path_label', { defaultValue: 'VAD Model Path' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <input
                        id="settings-vad-path"
                        type="text"
                        title={vadModelPath}
                        className="settings-input"
                        value={vadModelPath}
                        onChange={(e) => setVadModelPath(e.target.value)}
                        placeholder={t('settings.path_placeholder')}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('vad')}
                        aria-label={t('settings.browse')}
                    >
                        <FolderIcon />
                    </button>
                </div>
            </div>

            <ItnModelList
                itnRulesOrder={itnRulesOrder}
                setItnRulesOrder={setItnRulesOrder}
                enabledITNModels={enabledITNModels}
                setEnabledITNModels={setEnabledITNModels}
                installedITNModels={installedITNModels}
                downloadingId={downloadingId}
                progress={progress}
                onDownload={onDownloadITN}
                onCancelDownload={onCancelDownload}
            />
        </div>
    );
}
