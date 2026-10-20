import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  buildModelPathConfigPatch,
  buildModelRemovalConfigPatch,
  buildRestoreDefaultModelConfigPatch,
} from '../services/modelConfigPatches';
import { modelService } from '../services/modelService';
import { cleanInterruptedDownload } from '../services/tauri/app';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useModelDownloadStore } from '../stores/modelDownloadStore';
import type {
  ModelCatalogModel,
  ModelCatalogSnapshot,
  ModelInfo,
  ProgressCallback,
  ScenarioSelectedModelIds,
} from '../types/modelCatalog';
import { EMPTY_SCENARIO_SELECTED_MODEL_IDS } from '../types/modelCatalog';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { scenarioModelFieldKey } from '../utils/scenarioModels';
import { getSettingsPerfErrorDetail, markSettingsPerf } from '../utils/settingsPerf';
import { formatBytes } from './notifications/modelDownloadAdapter';

type DownloadState = {
  progress: number;
  status: string;
  controller: AbortController;
};

type ModelCatalogLoadState = 'idle' | 'loading' | 'ready' | 'error';

const EMPTY_MODEL_CATALOG_SNAPSHOT: ModelCatalogSnapshot = {
  modelsDir: '',
  models: [],
  sections: [],
  selectionOptions: {
    streaming: [],
    batch: [],
    speakerSegmentation: [],
    speakerEmbedding: [],
  },
  modelPathById: {},
  modelIdByNormalizedPath: {},
  pathMatchTokens: [],
  dependencyRequestsByModelId: {},
  restoreDefaults: {
    punctuationModelPath: '',
    speakerSegmentationModelPath: '',
    speakerEmbeddingModelPath: '',
    enableITN: true,
    batchVadEnabled: true,
    vadBufferSize: 5,
    maxConcurrent: 2,
  },
};

export interface UiSelectedModelIds extends ScenarioSelectedModelIds {
  streaming: string | null;
  batch: string | null;
}

const EMPTY_SELECTED_MODEL_IDS: UiSelectedModelIds = {
  ...EMPTY_SCENARIO_SELECTED_MODEL_IDS,
  streaming: null,
  batch: null,
};

export type ModelManagerContextType = ReturnType<typeof useModelManager>;

export const ModelManagerContext = createContext<ModelManagerContextType | null>(null);

function scheduleAfterFrame(callback: () => void): () => void {
  if (typeof requestAnimationFrame === 'function') {
    const frameId = requestAnimationFrame(() => callback());
    return () => cancelAnimationFrame(frameId);
  }

  const timeoutId = window.setTimeout(callback, 0);
  return () => window.clearTimeout(timeoutId);
}

export function useModelManagerContext() {
  const context = useContext(ModelManagerContext);
  if (!context) {
    throw new Error('useModelManagerContext must be used within a ModelManagerContext.Provider');
  }
  return context;
}

/**
 * Hook encapsulating all model lifecycle operations:
 * installation checks, downloads, deletion, and path management.
 */
export function useModelManager(isOpen: boolean) {
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const alert = useDialogStore((state) => state.alert);
  const { t } = useTranslation();

  const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});
  const inFlightModelsRef = useRef<Set<string>>(new Set());
  const activeDownloadsRef = useRef<
    Map<string, { downloadId: string; controller: AbortController }>
  >(new Map());
  const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogSnapshot>(
    EMPTY_MODEL_CATALOG_SNAPSHOT
  );
  const [catalogLoadState, setCatalogLoadState] = useState<ModelCatalogLoadState>('idle');
  const [catalogLoadError, setCatalogLoadError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const catalogLoadGenerationRef = useRef(0);
  const effectiveCatalogLoadState = isOpen ? catalogLoadState : 'idle';
  const effectiveCatalogLoadError = isOpen ? catalogLoadError : null;

  const updateConfig = useCallback(
    (updates: Partial<typeof config>) => {
      setConfig(updates);
    },
    [setConfig]
  );

  const applyModelCatalogSnapshot = useCallback((snapshot: ModelCatalogSnapshot) => {
    setModelCatalog(snapshot);
    setInstalledModels(
      new Set(snapshot.models.filter((model) => model.isInstalled).map((model) => model.id))
    );
  }, []);

  const refreshModelCatalogSnapshot = useCallback(
    async (loadGeneration?: number) => {
      const isCurrentLoad = () =>
        loadGeneration === undefined || loadGeneration === catalogLoadGenerationRef.current;
      setCatalogLoadState('loading');
      setCatalogLoadError(null);
      markSettingsPerf('settings.models.catalog.load.start');
      try {
        const snapshot = await modelService.getModelCatalogSnapshot();
        if (!isCurrentLoad()) {
          markSettingsPerf('settings.models.catalog.load.end', {
            modelCount: snapshot.models.length,
            stale: true,
          });
          return snapshot;
        }
        applyModelCatalogSnapshot(snapshot);
        setCatalogLoadState('ready');
        markSettingsPerf('settings.models.catalog.load.end', {
          modelCount: snapshot.models.length,
        });
        return snapshot;
      } catch (error) {
        if (!isCurrentLoad()) {
          markSettingsPerf('settings.models.catalog.load.fail', {
            ...getSettingsPerfErrorDetail(error),
            stale: true,
          });
          throw error;
        }
        setCatalogLoadState('error');
        setCatalogLoadError(extractErrorMessage(error));
        markSettingsPerf('settings.models.catalog.load.fail', getSettingsPerfErrorDetail(error));
        throw error;
      }
    },
    [applyModelCatalogSnapshot]
  );

  const getCatalogModel = useCallback(
    (modelId: string): ModelCatalogModel | undefined => {
      return modelCatalog.models.find((model) => model.id === modelId);
    },
    [modelCatalog.models]
  );

  const getModelInstallPath = useCallback(
    async (model: ModelInfo): Promise<string> => {
      const catalogModel = getCatalogModel(model.id);
      if (catalogModel?.installPath) {
        return catalogModel.installPath;
      }
      return await modelService.getModelPath(model.id);
    },
    [getCatalogModel]
  );

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const loadGeneration = catalogLoadGenerationRef.current + 1;
    catalogLoadGenerationRef.current = loadGeneration;
    const cancelScheduledLoad = scheduleAfterFrame(() => {
      void refreshModelCatalogSnapshot(loadGeneration).catch((error) => {
        logger.warn('[useModelManager] Failed to load model catalog snapshot:', error);
      });
    });

    return () => {
      if (catalogLoadGenerationRef.current === loadGeneration) {
        catalogLoadGenerationRef.current += 1;
      }
      cancelScheduledLoad();
    };
  }, [isOpen, refreshModelCatalogSnapshot]);

  const selectedModelIds = useMemo(() => {
    if (!isOpen || catalogLoadState !== 'ready') {
      return EMPTY_SELECTED_MODEL_IDS;
    }

    return {
      ...modelService.resolveAsrSelectedModelIdsFromSnapshot(modelCatalog, config),
      ...modelService.resolveScenarioSelectedModelIdsFromSnapshot(modelCatalog, config),
    };
  }, [catalogLoadState, config, isOpen, modelCatalog]);

  const setModelPath = useCallback(
    (model: ModelInfo, path: string) => {
      const updates = buildModelPathConfigPatch(config, model, path);
      if (Object.keys(updates).length > 0) {
        updateConfig(updates);
      }
    },
    [config, updateConfig]
  );

  // Listen for background download requests from components
  useEffect(() => {
    const handleBackgroundDownload = async (event: Event) => {
      const customEvent = event as CustomEvent;
      if (customEvent.detail?.modelId) {
        const modelId = customEvent.detail.modelId;
        const model = getCatalogModel(modelId);
        if (model && !downloads[modelId]) {
          try {
            const path = await modelService.downloadModel(
              modelId,
              undefined,
              undefined,
              config.modelDownloadMirror
            );
            await refreshModelCatalogSnapshot();
            setModelPath(model, path);
          } catch (e) {
            logger.error(`Background download failed for ${modelId}:`, e);
          }
        }
      }
    };

    document.addEventListener('download-background-model', handleBackgroundDownload);
    return () => {
      document.removeEventListener('download-background-model', handleBackgroundDownload);
    };
  }, [
    downloads,
    getCatalogModel,
    refreshModelCatalogSnapshot,
    setModelPath,
    config.modelDownloadMirror,
  ]);

  async function executeDownload(
    modelId: string,
    downloadFn: (
      id: string,
      callback: ProgressCallback,
      signal: AbortSignal,
      downloadId: string
    ) => Promise<string>,
    onSuccess: (path: string) => void | Promise<void>
  ) {
    if (activeDownloadsRef.current.has(modelId)) return;

    const controller = new AbortController();
    const downloadId = `${modelId}-${Date.now()}-${Math.random().toString(36).substring(7)}`;
    activeDownloadsRef.current.set(modelId, { downloadId, controller });

    setDownloads((prev) => ({
      ...prev,
      [modelId]: { progress: 0, status: '', controller },
    }));

    const catalogModel = getCatalogModel(modelId);
    const modelName = catalogModel?.name ?? modelId;

    useModelDownloadStore.getState().startDownload({
      modelId,
      modelName,
      modelType: 'asr',
      downloadId,
      onCancel: () => {
        controller.abort();
      },
      onRetry: () => {
        const targetModel = getCatalogModel(modelId);
        if (targetModel && !activeDownloadsRef.current.has(modelId)) {
          void handleDownload(targetModel);
        }
      },
    });

    let downloadedPath: string;
    try {
      try {
        downloadedPath = await downloadFn(
          modelId,
          (pct, status) => {
            setDownloads((prev) => {
              if (!prev[modelId]) return prev;
              return {
                ...prev,
                [modelId]: { ...prev[modelId], progress: pct, status: status },
              };
            });
            useModelDownloadStore.getState().updateProgress(downloadId, { progress: pct });
          },
          controller.signal,
          downloadId
        );
      } catch (error) {
        const errorMsg = extractErrorMessage(error);
        if (errorMsg === 'Download cancelled' || controller.signal.aborted) {
          logger.info('Download cancelled by user');
          void useModelDownloadStore.getState().cancelDownload(modelId, downloadId);
        } else {
          useModelDownloadStore.getState().failDownload(modelId, errorMsg, downloadId);
          if (errorMsg.includes('Download already in progress')) {
            await alert(t('errors.model.already_in_progress'));
          } else {
            await showError({
              code: 'model.download_failed',
              messageKey: 'errors.model.download_failed',
              cause: error,
            });
          }
        }
        await refreshModelCatalogSnapshot();
        return;
      }

      useModelDownloadStore.getState().completeDownload(modelId, downloadId);
      try {
        await refreshModelCatalogSnapshot();
        await onSuccess(downloadedPath);
      } catch (postError) {
        logger.error(
          'Failed to refresh catalog snapshot or complete download callback:',
          postError
        );
      }
    } finally {
      if (activeDownloadsRef.current.get(modelId)?.downloadId === downloadId) {
        activeDownloadsRef.current.delete(modelId);
        inFlightModelsRef.current.delete(modelId);
        setDownloads((prev) => {
          const next = { ...prev };
          delete next[modelId];
          return next;
        });
      }
    }
  }

  function handleCancelDownload(modelId: string) {
    const active = activeDownloadsRef.current.get(modelId);
    if (active) {
      active.controller.abort();
      void useModelDownloadStore.getState().cancelDownload(modelId, active.downloadId);
    } else {
      const download = downloads[modelId];
      if (download) {
        download.controller.abort();
        void useModelDownloadStore.getState().cancelDownload(modelId);
      }
      inFlightModelsRef.current.delete(modelId);
    }
    setDownloads((prev) => {
      const next = { ...prev };
      delete next[modelId];
      return next;
    });
    void refreshModelCatalogSnapshot();
  }

  async function handleDownload(model: ModelInfo) {
    const isDownloadingInStore =
      useModelDownloadStore.getState().downloads[model.id]?.status === 'downloading';
    if (
      inFlightModelsRef.current.has(model.id) ||
      activeDownloadsRef.current.has(model.id) ||
      isDownloadingInStore
    ) {
      return;
    }
    inFlightModelsRef.current.add(model.id);

    try {
      const { compatible, reason } = await modelService.checkHardware(model.id);
      if (!compatible) {
        const confirmed = await confirm(t('settings.hardware_warning_confirm', { reason }), {
          title: t('settings.hardware_warning_title'),
          variant: 'warning',
        });
        if (!confirmed) {
          inFlightModelsRef.current.delete(model.id);
          return;
        }
      }

      if (model.modes && model.modes.length > 0) {
        const dependencyUpdates: Partial<typeof config> = {};
        const dependencies = modelCatalog.dependencyRequestsByModelId[model.id] ?? [];
        for (const dependency of dependencies) {
          const fieldKeys = [
            scenarioModelFieldKey(dependency.configKey, 'live'),
            scenarioModelFieldKey(dependency.configKey, 'batch'),
          ];
          if (fieldKeys.every((key) => config[key])) {
            continue;
          }
          if (dependency.isInstalled) {
            for (const key of fieldKeys) {
              if (!config[key]) {
                dependencyUpdates[key] = dependency.installPath;
              }
            }
          } else {
            document.dispatchEvent(
              new CustomEvent('download-background-model', {
                detail: { modelId: dependency.modelId },
              })
            );
          }
        }
        if (Object.keys(dependencyUpdates).length > 0) {
          updateConfig(dependencyUpdates);
        }
      }

      await executeDownload(
        model.id,
        (id, cb, sig, dlId) =>
          modelService.downloadModel(id, cb, sig, config.modelDownloadMirror, dlId),
        (path) => {
          setModelPath(model, path);
        }
      );
    } catch (err) {
      inFlightModelsRef.current.delete(model.id);
      throw err;
    }
  }

  async function handleLoad(model: ModelInfo) {
    try {
      const path = await getModelInstallPath(model);
      setModelPath(model, path);
    } catch (error) {
      await showError({
        code: 'model.load_failed',
        messageKey: 'errors.model.load_failed',
        messageParams: { name: model.name },
        cause: error,
      });
    }
  }

  async function handleDelete(model: ModelInfo) {
    if (deletingId) return;

    const confirmed = await confirm(t('settings.delete_confirm_message', { name: model.name }), {
      title: t('settings.delete_confirm_title'),
      variant: 'warning',
    });

    if (!confirmed) return;

    setDeletingId(model.id);
    try {
      const deletedPath = await getModelInstallPath(model);
      await modelService.deleteModel(model.id);
      await refreshModelCatalogSnapshot();
      updateConfig(buildModelRemovalConfigPatch(config, deletedPath));
    } catch (error) {
      await showError({
        code: 'model.delete_failed',
        messageKey: 'errors.model.delete_failed',
        messageParams: { name: model.name },
        cause: error,
      });
    } finally {
      setDeletingId(null);
    }
  }

  async function handleDiscardDownload(model: ModelInfo) {
    if (inFlightModelsRef.current.has(model.id) || activeDownloadsRef.current.has(model.id)) {
      return;
    }

    const catalogModel = getCatalogModel(model.id);
    const interruptedDownload =
      catalogModel?.interruptedDownload ?? (model as ModelCatalogModel).interruptedDownload;
    if (!interruptedDownload) return;

    inFlightModelsRef.current.add(model.id);
    try {
      const downloadedFormatted = formatBytes(interruptedDownload.downloadedBytes) || '0 MB';
      const totalFormatted = formatBytes(interruptedDownload.totalBytes ?? undefined);
      const sizeText = totalFormatted
        ? `${downloadedFormatted} / ${totalFormatted}`
        : downloadedFormatted;
      const pctText =
        interruptedDownload.percentage != null
          ? `${Math.round(interruptedDownload.percentage)}%`
          : '';
      const progressDetail = pctText ? `${sizeText} (${pctText})` : sizeText;

      const confirmed = await confirm(
        t('settings.discard_download_confirm_message', {
          name: model.name,
          progress: progressDetail,
          defaultValue: `Are you sure you want to discard the incomplete download for ${model.name} (${progressDetail})? This will delete partial files and free up disk space.`,
        }),
        {
          title: t('settings.discard_download_confirm_title', {
            defaultValue: 'Discard Incomplete Download',
          }),
          confirmLabel: t('settings.discard_download_confirm_button', {
            defaultValue: 'Discard Files',
          }),
          cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
          variant: 'warning',
        }
      );

      if (!confirmed) return;

      if (activeDownloadsRef.current.has(model.id)) return;

      try {
        await cleanInterruptedDownload(model.id);
        useModelDownloadStore.getState().dismissDownload(model.id);
        await refreshModelCatalogSnapshot();
      } catch (cleanErr) {
        logger.warn('Failed to clean interrupted download:', cleanErr);
        await showError({
          code: 'model.clean_failed',
          messageKey: 'errors.model.clean_failed',
          cause: cleanErr,
        });
      }
    } finally {
      inFlightModelsRef.current.delete(model.id);
    }
  }

  async function restoreDefaultModelSettings() {
    const confirmed = await confirm(t('settings.restore_defaults_confirm'), {
      title: t('settings.restore_defaults'),
      variant: 'warning',
    });
    if (!confirmed) return;

    try {
      const snapshot = await refreshModelCatalogSnapshot();
      const defaults = snapshot.restoreDefaults;
      updateConfig(buildRestoreDefaultModelConfigPatch(config, defaults));
    } catch (e) {
      logger.warn('Failed to restore default model settings', e);
    }
  }

  return {
    deletingId,
    downloads,
    installedModels,
    modelCatalog,
    selectedModelIds,
    catalogLoadState: effectiveCatalogLoadState,
    catalogLoadError: effectiveCatalogLoadError,

    handleDownload,
    handleCancelDownload,
    handleDiscardDownload,
    handleLoad,
    handleDelete,
    restoreDefaultModelSettings,
  };
}
