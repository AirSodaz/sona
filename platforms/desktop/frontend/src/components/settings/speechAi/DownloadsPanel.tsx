import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CudaAddonInspection } from '../../../bindings';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { cudaAddonService } from '../../../services/cudaAddonService';
import { useSetConfig, useTranscriptionConfig } from '../../../stores/configStore';
import { useDialogStore } from '../../../stores/dialogStore';
import type { ModelCatalogSectionType } from '../../../types/modelCatalog';
import { Dropdown } from '../../Dropdown';
import { RestoreIcon } from '../../Icons';
import { Switch } from '../../Switch';
import { CloudAsrProviderGrid } from '../CloudAsrProviderGrid';
import { LocalModelManagementSection } from '../LocalModelManagementSection';

export function DownloadsPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const updateConfig = useSetConfig();
  const transcription = useTranscriptionConfig();
  const {
    modelCatalog,
    catalogLoadState,
    catalogLoadError,
    installedModels,
    downloads,
    handleDelete,
    handleDownload,
    handleCancelDownload,
    handleDiscardDownload,
    restoreDefaultModelSettings,
  } = useModelManagerContext();
  const [cudaStatus, setCudaStatus] = useState<CudaAddonInspection | null>(null);
  const [cudaDownloading, setCudaDownloading] = useState(false);
  const [cudaProgress, setCudaProgress] = useState(0);

  const sectionGroupsByType = useMemo(
    () => new Map(modelCatalog.sections.map((section) => [section.type, section.groups])),
    [modelCatalog.sections]
  );
  const getSectionGroups = useCallback(
    (type: ModelCatalogSectionType) => sectionGroupsByType.get(type) ?? [],
    [sectionGroupsByType]
  );
  const getSectionStatus = useCallback(
    (type: ModelCatalogSectionType) => {
      const allModels = (sectionGroupsByType.get(type) ?? []).flatMap((group) => group.models);
      const downloadingModel = allModels.find((model) => downloads[model.id]);
      if (downloadingModel) {
        return {
          type: 'pending',
          text: t('settings.downloading_progress', {
            progress: Math.round(downloads[downloadingModel.id].progress),
          }),
        };
      }
      const installedCount = allModels.filter((model) => installedModels.has(model.id)).length;
      if (installedCount > 0) {
        return { type: 'ready', text: t('settings.installed_count', { count: installedCount }) };
      }
      return { type: 'off', text: t('settings.not_installed') };
    },
    [downloads, installedModels, sectionGroupsByType, t]
  );

  const onGpu = async (value: string) => {
    if (value === 'cuda') {
      const status = cudaStatus ?? (await cudaAddonService.getStatus().catch(() => null));
      setCudaStatus(status);
      if (status && !status.isInstalled) {
        const confirmed = await useDialogStore
          .getState()
          .confirm(t('settings.cuda_addon_download_confirm'), {
            title: t('settings.cuda_addon_download_title'),
            confirmLabel: t('common.download'),
          });
        if (!confirmed) return;
        setCudaDownloading(true);
        setCudaProgress(0);
        try {
          const next = await cudaAddonService.downloadAndInstall({
            onProgress: (progress) => setCudaProgress(progress.progressPercent),
          });
          setCudaStatus(next);
          updateConfig({ gpuAcceleration: 'cuda' });
        } catch (error) {
          useDialogStore.getState().alert(String(error), {
            title: t('settings.cuda_addon_download_title'),
            variant: 'error',
          });
        } finally {
          setCudaDownloading(false);
        }
        return;
      }
    }
    updateConfig({ gpuAcceleration: value as 'auto' | 'cpu' | 'vulkan' | 'metal' | 'cuda' });
  };

  return (
    <div className="speech-ai" id="settings-speech-downloads">
      <LocalModelManagementSection
        catalogLoadState={catalogLoadState}
        catalogLoadError={catalogLoadError}
        sectionProps={{
          installedModels,
          downloads,
          handleDelete,
          handleDownload,
          handleCancelDownload,
          handleDiscardDownload,
        }}
        localModelActionsDisabled={catalogLoadState !== 'ready'}
        getSectionGroups={getSectionGroups}
        getSectionStatus={getSectionStatus}
        t={t}
      />
      <div className="speech-ai-section-label">{t('settings.online_model_management')}</div>
      <div className="speech-ai-card" style={{ padding: 12 }}>
        <CloudAsrProviderGrid />
      </div>
      <div className="speech-ai-section-label" id="settings-transcription-settings">
        {t('settings.transcription_settings')}
      </div>
      <div className="speech-ai-card">
        <div className="settings-item-container layout-horizontal">
          <div className="settings-item-info">
            <div className="settings-item-title">{t('settings.enable_itn')}</div>
            <div className="settings-item-hint">{t('settings.enable_itn_hint')}</div>
          </div>
          <div className="settings-item-action">
            <Switch
              checked={transcription.enableITN ?? true}
              onChange={(checked) => updateConfig({ enableITN: checked })}
            />
          </div>
        </div>
        <div className="settings-item-container layout-horizontal">
          <div className="settings-item-info">
            <div className="settings-item-title">{t('settings.max_concurrent_label')}</div>
            <div className="settings-item-hint">{t('settings.max_concurrent_hint')}</div>
          </div>
          <div className="settings-item-action">
            <input
              id="settings-max-concurrent"
              type="number"
              className="settings-input"
              min={1}
              max={4}
              value={transcription.maxConcurrent || 2}
              onChange={(event) => {
                const value = Number(event.target.value);
                if (value > 0) updateConfig({ maxConcurrent: value });
              }}
            />
          </div>
        </div>
        <div className="settings-item-container layout-horizontal" id="settings-gpu-acceleration">
          <div className="settings-item-info">
            <div className="settings-item-title">{t('settings.gpu_acceleration_label')}</div>
            <div className="settings-item-hint">{t('settings.gpu_acceleration_hint')}</div>
          </div>
          <div className="settings-item-action" style={{ width: 160 }}>
            <Dropdown
              id="settings-gpu-acceleration-dropdown"
              aria-label={t('settings.gpu_acceleration_label')}
              value={transcription.gpuAcceleration ?? 'auto'}
              disabled={cudaDownloading}
              onChange={(value) => void onGpu(value)}
              options={[
                {
                  value: 'auto',
                  label: t('settings.gpu_acceleration_auto'),
                },
                { value: 'cpu', label: t('settings.value_off') },
                { value: 'vulkan', label: 'Vulkan' },
                { value: 'metal', label: 'Metal' },
                { value: 'cuda', label: 'CUDA' },
              ]}
            />
            {cudaDownloading ? (
              <div className="speech-ai-note">
                {t('settings.cuda_addon_downloading', {
                  percent: cudaProgress,
                })}
              </div>
            ) : null}
          </div>
        </div>
      </div>
      <button
        type="button"
        className="btn btn-restore-defaults"
        onClick={() => void restoreDefaultModelSettings()}
        disabled={catalogLoadState !== 'ready'}
        aria-label={t('settings.restore_defaults')}
      >
        <RestoreIcon />
        {t('settings.restore_defaults')}
      </button>
    </div>
  );
}
