import { Mic, PlaySquare, Search, Settings2, X } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CudaAddonInspection } from '../../bindings';
import { useModelManagerContext } from '../../hooks/useModelManager';
import {
  GROQ_WHISPER_PROVIDER_ID,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  syncLegacyAsrSelectionFields,
  syncLiveAsrSelectionFields,
  syncLiveOnlineAsrSelectionFields,
  syncOnlineAsrSelectionFields,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
} from '../../services/asrConfigService';
import { cudaAddonService } from '../../services/cudaAddonService';
import { modelService } from '../../services/modelService';
import { isOnlineAsrProviderId } from '../../services/onlineAsrProviders';
import { useModelConfig, useSetConfig, useTranscriptionConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import type { LocalAsrEngine } from '../../types/asr';
import type {
  ModelCatalogModel,
  ModelCatalogSectionType,
  ModelLabel,
  ModelSelectionOption,
} from '../../types/modelCatalog';
import { PRESET_MODELS_MAP, resolveModelLabels } from '../../types/modelCatalog';
import { findSelectedModelByMode } from '../../utils/modelSelection';
import {
  getScenarioVadBufferSize,
  type ScenarioModelKind,
  scenarioModelFieldKey,
} from '../../utils/scenarioModels';
import { markSettingsPerf } from '../../utils/settingsPerf';
import { Dropdown, type DropdownOption } from '../Dropdown';
import { ModelIcon, OnlineIcon, RestoreIcon } from '../Icons';
import { ModelBrandLogo } from '../icons/ModelLogos';
import { Switch } from '../Switch';
import { ModelCard } from './ModelCard';
import {
  DynamicProviderSettings,
  GroqWhisperSettingsCard,
  type ProviderSettingsProps,
  VolcengineSettingsCard,
} from './OnlineAsrSettingsCards';
import {
  SettingsAccordion,
  SettingsItem,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';

type ModelScenario = 'live' | 'batch';

const CUSTOM_PROVIDER_COMPONENTS: Record<string, React.ComponentType<ProviderSettingsProps>> = {
  [VOLCENGINE_DOUBAO_PROVIDER_ID]: VolcengineSettingsCard,
  [GROQ_WHISPER_PROVIDER_ID]: GroqWhisperSettingsCard,
};

const onlineAsrProvider = ONLINE_ASR_PROVIDER_DEFINITIONS[0];
const VOLCENGINE_DOUBAO_OPTION_ID = onlineAsrProvider.id;

interface SettingsModelsTabProps {
  isActive?: boolean;
}

function scheduleAfterFrame(callback: () => void): () => void {
  if (typeof requestAnimationFrame === 'function') {
    const frameId = requestAnimationFrame(() => callback());
    return () => cancelAnimationFrame(frameId);
  }

  const timeoutId = window.setTimeout(callback, 0);
  return () => window.clearTimeout(timeoutId);
}

function toDropdownOptions(
  options: ModelSelectionOption[],
  selectedId: string,
  withIcon = false
): Array<{ value: string; label: React.ReactNode; ariaLabel?: string }> {
  return options
    .filter((option) => option.isInstalled || option.id === selectedId)
    .map((option) => ({
      value: option.id,
      ariaLabel: option.label,
      label: withIcon ? (
        <span className="model-dropdown-option">
          <span className="model-dropdown-option-icon">
            <ModelBrandLogo model={{ id: option.id, name: option.label }} size={16} />
          </span>
          <span>{option.label}</span>
        </span>
      ) : (
        option.label
      ),
    }));
}

/**
 * Section-level download-mirror picker. Rendered in the section header rather
 * than the filter toolbar because it is a persistent download preference,
 * not a filter criterion.
 */
function MirrorDownloadPicker(): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();

  const mirrorOptions: DropdownOption[] = useMemo(
    () => [
      {
        value: 'auto',
        label: t('settings.model_download_mirror_auto', { defaultValue: '自动' }),
      },
      {
        value: 'direct',
        label: t('settings.model_download_mirror_direct', { defaultValue: '官方直连' }),
      },
      {
        value: 'ghproxy',
        label: t('settings.model_download_mirror_ghproxy'),
        group: t('settings.model_download_mirror_group_github', { defaultValue: 'GitHub' }),
      },
      {
        value: 'ghnet',
        label: t('settings.model_download_mirror_ghnet'),
        group: t('settings.model_download_mirror_group_github', { defaultValue: 'GitHub' }),
      },
      {
        value: 'hf-mirror',
        label: t('settings.model_download_mirror_hfmirror', {
          defaultValue: '镜像站 (hf-mirror.com)',
        }),
        group: t('settings.model_download_mirror_group_hf', { defaultValue: 'Hugging Face' }),
      },
    ],
    [t]
  );

  return (
    <div className="settings-mirror-picker">
      <span className="settings-mirror-picker-label">{t('settings.model_download_mirror')}</span>
      <Dropdown
        id="settings-download-mirror"
        aria-label={t('settings.model_download_mirror')}
        value={modelConfig.modelDownloadMirror || 'auto'}
        onChange={(value) => updateConfig({ modelDownloadMirror: value })}
        options={mirrorOptions}
        style={{ width: '150px' }}
      />
    </div>
  );
}

interface LocalModelManagementSectionProps {
  catalogLoadState: ReturnType<typeof useModelManagerContext>['catalogLoadState'];
  catalogLoadError: ReturnType<typeof useModelManagerContext>['catalogLoadError'];
  sectionProps: Pick<
    ReturnType<typeof useModelManagerContext>,
    'installedModels' | 'downloads' | 'handleDelete' | 'handleDownload' | 'handleCancelDownload'
  >;
  localModelActionsDisabled: boolean;
  getSectionGroups: (
    type: ModelCatalogSectionType
  ) => ReturnType<typeof useModelManagerContext>['modelCatalog']['sections'][number]['groups'];
  getSectionStatus: (type: ModelCatalogSectionType) => { type: string; text: string };
  t: ReturnType<typeof useTranslation>['t'];
}

function LocalModelManagementSection({
  catalogLoadState,
  catalogLoadError,
  sectionProps,
  localModelActionsDisabled,
  getSectionGroups,
  getSectionStatus,
  t,
}: LocalModelManagementSectionProps): React.JSX.Element {
  const [engineFilter, setEngineFilter] = useState<'all' | LocalAsrEngine>('all');
  const [statusFilter, setStatusFilter] = useState<
    'all' | 'installed' | 'not-installed' | 'downloading'
  >('all');
  const [labelFilter, setLabelFilter] = useState<'all' | ModelLabel>('all');
  const [searchQuery, setSearchQuery] = useState('');

  const engineFilterOptions = [
    { value: 'all', label: t('settings.model_filter_engine_all', { defaultValue: '全部引擎' }) },
    { value: 'sherpa-onnx', label: 'ONNX' },
    { value: 'llama-cpp', label: 'GGUF' },
  ];

  const labelFilterOptions = [
    { value: 'all', label: t('settings.model_filter_tag_all', { defaultValue: '全部标签' }) },
    { value: 'accurate', label: t('settings.model_tag_accurate', { defaultValue: '准确' }) },
    { value: 'lite', label: t('settings.model_tag_lite', { defaultValue: '轻量' }) },
  ];

  const statusFilterOptions = [
    { value: 'all', label: t('settings.model_filter_status_all', { defaultValue: '全部状态' }) },
    {
      value: 'installed',
      label: t('settings.model_filter_status_installed', { defaultValue: '已安装' }),
    },
    {
      value: 'not-installed',
      label: t('settings.model_filter_status_not_installed', { defaultValue: '未安装' }),
    },
    {
      value: 'downloading',
      label: t('settings.model_filter_status_downloading', { defaultValue: '下载中' }),
    },
  ];

  const filterModels = useCallback(
    (models: ModelCatalogModel[]) => {
      const query = searchQuery.trim().toLowerCase();
      return models.filter((model) => {
        if (engineFilter !== 'all' && model.engine !== engineFilter) return false;
        // Snapshot models lack curated labels; fall back to the preset JSON.
        if (labelFilter !== 'all' && !(resolveModelLabels(model) ?? []).includes(labelFilter)) {
          return false;
        }
        const isInstalled = sectionProps.installedModels.has(model.id);
        const isDownloading = !!sectionProps.downloads[model.id];
        if (statusFilter === 'installed' && !isInstalled) return false;
        if (statusFilter === 'not-installed' && (isInstalled || isDownloading)) return false;
        if (statusFilter === 'downloading' && !isDownloading) return false;
        if (
          query &&
          !model.name.toLowerCase().includes(query) &&
          !(model.versionLabel ?? '').toLowerCase().includes(query)
        ) {
          return false;
        }
        return true;
      });
    },
    [
      engineFilter,
      labelFilter,
      searchQuery,
      statusFilter,
      sectionProps.downloads,
      sectionProps.installedModels,
    ]
  );

  const filteredGroupsByType = useMemo(() => {
    const types: ModelCatalogSectionType[] = [
      'asr',
      'punctuation',
      'vad',
      'speaker-segmentation',
      'speaker-embedding',
      'alignment',
    ];
    return new Map(
      types.map((type) => [
        type,
        getSectionGroups(type)
          .map((group) => ({ ...group, models: filterModels(group.models as ModelCatalogModel[]) }))
          .filter((group) => group.models.length > 0),
      ])
    );
  }, [filterModels, getSectionGroups]);

  const isCatalogLoading = catalogLoadState === 'loading';
  const isCatalogReady = catalogLoadState === 'ready';

  useEffect(() => {
    markSettingsPerf('settings.models.local.content.commit');
    const cancelFrame = scheduleAfterFrame(() => {
      markSettingsPerf('settings.models.local.content.raf');
    });
    return cancelFrame;
  }, []);

  return (
    <SettingsSection
      title={t('settings.batch_model_management', { defaultValue: '离线模型管理' })}
      icon={<RestoreIcon />}
      actions={<MirrorDownloadPicker />}
    >
      {isCatalogReady && (
        <div className="settings-model-toolbar">
          <Dropdown
            id="settings-model-engine-filter"
            aria-label={t('settings.model_filter_engine_label', { defaultValue: '按引擎筛选' })}
            value={engineFilter}
            onChange={(value) => setEngineFilter(value as 'all' | LocalAsrEngine)}
            options={engineFilterOptions}
            style={{ width: '130px' }}
          />
          <Dropdown
            id="settings-model-status-filter"
            aria-label={t('settings.model_filter_status_label', { defaultValue: '按状态筛选' })}
            value={statusFilter}
            onChange={(value) => setStatusFilter(value as typeof statusFilter)}
            options={statusFilterOptions}
            style={{ width: '130px' }}
          />
          <Dropdown
            id="settings-model-label-filter"
            aria-label={t('settings.model_filter_tag_label', { defaultValue: '按标签筛选' })}
            value={labelFilter}
            onChange={(value) => setLabelFilter(value as 'all' | ModelLabel)}
            options={labelFilterOptions}
            style={{ width: '120px' }}
          />
          <div className="settings-model-search-wrapper">
            <Search size={16} className="settings-model-search-icon" />
            <input
              id="settings-model-search"
              className="settings-model-search"
              type="text"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') {
                  setSearchQuery('');
                }
              }}
              placeholder={t('settings.model_filter_search_placeholder', {
                defaultValue: '搜索模型…',
              })}
              aria-label={t('settings.model_filter_search_placeholder', {
                defaultValue: '搜索模型…',
              })}
            />
            {searchQuery && (
              <button
                type="button"
                className="settings-model-search-clear"
                onClick={() => {
                  setSearchQuery('');
                  document.getElementById('settings-model-search')?.focus();
                }}
                aria-label={t('settings.model_search_clear', { defaultValue: '清除搜索' })}
              >
                <X size={12} strokeWidth={2.25} />
              </button>
            )}
          </div>
        </div>
      )}
      {isCatalogLoading && (
        <div className="settings-hint" role="status">
          {t('settings.models_checking_local', { defaultValue: 'Checking local models...' })}
        </div>
      )}
      {catalogLoadState === 'error' && (
        <div className="settings-hint" role="status">
          {t('settings.models_check_failed', {
            error: catalogLoadError ?? '',
            defaultValue: 'Could not check local models: {{error}}. Download status may be stale.',
          })}
        </div>
      )}
      {isCatalogReady && (
        <>
          <SettingsAccordion
            title={t('settings.recognition_models')}
            status={
              <span className={`status-badge ${getSectionStatus('asr').type}`}>
                {getSectionStatus('asr').text}
              </span>
            }
            defaultOpen={true}
          >
            {(filteredGroupsByType.get('asr') ?? []).map((group) => (
              <ModelCard
                key={group.key}
                models={group.models}
                isAsr={true}
                installedModels={sectionProps.installedModels}
                downloads={sectionProps.downloads}
                onDelete={sectionProps.handleDelete}
                onDownload={sectionProps.handleDownload}
                onCancelDownload={sectionProps.handleCancelDownload}
                actionsDisabled={localModelActionsDisabled}
              />
            ))}
            {(filteredGroupsByType.get('asr')?.length ?? 0) === 0 && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: '没有匹配的模型' })}
              </div>
            )}
          </SettingsAccordion>

          <SettingsAccordion
            title={t('settings.punctuation_models')}
            status={
              <span className={`status-badge ${getSectionStatus('punctuation').type}`}>
                {getSectionStatus('punctuation').text}
              </span>
            }
          >
            {(filteredGroupsByType.get('punctuation') ?? []).map((group) => (
              <ModelCard
                key={group.key}
                models={group.models}
                isAsr={false}
                installedModels={sectionProps.installedModels}
                downloads={sectionProps.downloads}
                onDelete={sectionProps.handleDelete}
                onDownload={sectionProps.handleDownload}
                onCancelDownload={sectionProps.handleCancelDownload}
                actionsDisabled={localModelActionsDisabled}
              />
            ))}
            {(filteredGroupsByType.get('punctuation')?.length ?? 0) === 0 && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: '没有匹配的模型' })}
              </div>
            )}
          </SettingsAccordion>

          <SettingsAccordion
            title={t('settings.vad_models')}
            status={
              <span className={`status-badge ${getSectionStatus('vad').type}`}>
                {getSectionStatus('vad').text}
              </span>
            }
          >
            {(filteredGroupsByType.get('vad') ?? []).map((group) => (
              <ModelCard
                key={group.key}
                models={group.models}
                isAsr={false}
                installedModels={sectionProps.installedModels}
                downloads={sectionProps.downloads}
                onDelete={sectionProps.handleDelete}
                onDownload={sectionProps.handleDownload}
                onCancelDownload={sectionProps.handleCancelDownload}
                actionsDisabled={localModelActionsDisabled}
              />
            ))}
            {(filteredGroupsByType.get('vad')?.length ?? 0) === 0 && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: '没有匹配的模型' })}
              </div>
            )}
          </SettingsAccordion>

          <SettingsAccordion
            title={t('settings.speaker_segmentation_models', {
              defaultValue: 'Speaker Segmentation Models',
            })}
            status={
              <span className={`status-badge ${getSectionStatus('speaker-segmentation').type}`}>
                {getSectionStatus('speaker-segmentation').text}
              </span>
            }
          >
            {(filteredGroupsByType.get('speaker-segmentation') ?? []).map((group) => (
              <ModelCard
                key={group.key}
                models={group.models}
                isAsr={false}
                installedModels={sectionProps.installedModels}
                downloads={sectionProps.downloads}
                onDelete={sectionProps.handleDelete}
                onDownload={sectionProps.handleDownload}
                onCancelDownload={sectionProps.handleCancelDownload}
                actionsDisabled={localModelActionsDisabled}
              />
            ))}
            {(filteredGroupsByType.get('speaker-segmentation')?.length ?? 0) === 0 && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: '没有匹配的模型' })}
              </div>
            )}
          </SettingsAccordion>

          <SettingsAccordion
            title={t('settings.speaker_embedding_models', {
              defaultValue: 'Speaker Embedding Models',
            })}
            status={
              <span className={`status-badge ${getSectionStatus('speaker-embedding').type}`}>
                {getSectionStatus('speaker-embedding').text}
              </span>
            }
          >
            {(filteredGroupsByType.get('speaker-embedding') ?? []).map((group) => (
              <ModelCard
                key={group.key}
                models={group.models}
                isAsr={false}
                installedModels={sectionProps.installedModels}
                downloads={sectionProps.downloads}
                onDelete={sectionProps.handleDelete}
                onDownload={sectionProps.handleDownload}
                onCancelDownload={sectionProps.handleCancelDownload}
                actionsDisabled={localModelActionsDisabled}
              />
            ))}
            {(filteredGroupsByType.get('speaker-embedding')?.length ?? 0) === 0 && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: '没有匹配的模型' })}
              </div>
            )}
          </SettingsAccordion>

          <SettingsAccordion
            title={t('settings.alignment_models', {
              defaultValue: 'CTC 对齐模型',
            })}
            status={
              <span className={`status-badge ${getSectionStatus('alignment').type}`}>
                {getSectionStatus('alignment').text}
              </span>
            }
          >
            {(filteredGroupsByType.get('alignment') ?? []).map((group) => (
              <ModelCard
                key={group.key}
                models={group.models}
                isAsr={false}
                installedModels={sectionProps.installedModels}
                downloads={sectionProps.downloads}
                onDelete={sectionProps.handleDelete}
                onDownload={sectionProps.handleDownload}
                onCancelDownload={sectionProps.handleCancelDownload}
                actionsDisabled={localModelActionsDisabled}
              />
            ))}
            {(filteredGroupsByType.get('alignment')?.length ?? 0) === 0 && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: '没有匹配的模型' })}
              </div>
            )}
          </SettingsAccordion>
        </>
      )}
    </SettingsSection>
  );
}

export function SettingsModelsTab({
  isActive: _isActive = true,
}: SettingsModelsTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const transcriptionConfig = useTranscriptionConfig();
  const updateConfig = useSetConfig();
  const [showLocalModelContent, setShowLocalModelContent] = useState(false);
  const [activeScenario, setActiveScenario] = useState<ModelScenario>('live');
  const {
    installedModels,
    modelCatalog,
    selectedModelIds,
    catalogLoadState,
    catalogLoadError,
    downloads,
    handleDelete,
    handleDownload,
    handleCancelDownload,
    restoreDefaultModelSettings,
  } = useModelManagerContext();

  const maxConcurrent = transcriptionConfig.maxConcurrent || 2;
  const enableITN = transcriptionConfig.enableITN ?? true;
  const batchVadEnabled = transcriptionConfig.batchVadEnabled ?? true;
  const gpuAcceleration = transcriptionConfig.gpuAcceleration ?? 'auto';
  const isCatalogReady = catalogLoadState === 'ready';
  const localModelActionsDisabled = !isCatalogReady;
  const isBatchScenario = activeScenario === 'batch';
  const activeVadBufferSize = getScenarioVadBufferSize(transcriptionConfig, activeScenario);

  const isBatchVadForced = useMemo(() => {
    if (!isBatchScenario) {
      return false;
    }
    const selection = modelConfig.asr?.selections.batch;
    if (selection?.engine !== 'local') {
      return false;
    }
    const modelInfo = selection.modelId
      ? undefined
      : findSelectedModelByMode(selection.modelPath, 'batch');
    const modelId = selection.modelId ?? modelInfo?.id;
    const preset = modelId ? PRESET_MODELS_MAP.get(modelId) : undefined;
    const modelType = (preset?.type || modelInfo?.type || '').toLowerCase();
    const idLower = (modelId || '').toLowerCase();
    const pathLower = (selection.modelPath || '').toLowerCase();

    if (!modelId && !pathLower) {
      return false;
    }

    const isExempt =
      modelType === 'qwen3-asr' ||
      modelType === 'parakeet-tdt' ||
      idLower.includes('qwen3-asr') ||
      idLower.includes('qwen3_asr') ||
      idLower.includes('parakeet-tdt') ||
      idLower.includes('parakeet_tdt') ||
      pathLower.includes('qwen3-asr') ||
      pathLower.includes('qwen3_asr') ||
      pathLower.includes('parakeet-tdt') ||
      pathLower.includes('parakeet_tdt');

    return !isExempt;
  }, [isBatchScenario, modelConfig.asr?.selections.batch]);

  const [cudaStatus, setCudaStatus] = useState<CudaAddonInspection | null>(null);
  const [cudaDownloading, setCudaDownloading] = useState(false);
  const [cudaProgress, setCudaProgress] = useState(0);

  useEffect(() => {
    let mounted = true;
    cudaAddonService
      .getStatus()
      .then((status) => {
        if (mounted) {
          setCudaStatus(status);
        }
      })
      .catch(() => {});
    return () => {
      mounted = false;
    };
  }, []);

  const handleGpuAccelerationChange = useCallback(
    async (value: string) => {
      if (value === 'cuda') {
        const currentStatus = cudaStatus ?? (await cudaAddonService.getStatus().catch(() => null));
        if (currentStatus && !currentStatus.isInstalled) {
          const confirmed = await useDialogStore
            .getState()
            .confirm(t('settings.cuda_addon_download_confirm'), {
              title: t('settings.cuda_addon_download_title'),
              confirmLabel: t('common.download', { defaultValue: 'Download' }),
            });
          if (!confirmed) {
            return;
          }
          setCudaDownloading(true);
          setCudaProgress(0);
          try {
            const newStatus = await cudaAddonService.downloadAndInstall({
              onProgress: (p) => setCudaProgress(p.progressPercent),
            });
            setCudaStatus(newStatus);
            updateConfig({ gpuAcceleration: 'cuda' });
            useDialogStore.getState().alert(t('settings.cuda_addon_install_success'), {
              title: t('settings.cuda_addon_download_title'),
              variant: 'success',
            });
          } catch (err) {
            useDialogStore.getState().alert(String(err), {
              title: t('settings.cuda_addon_download_title'),
              variant: 'error',
            });
          } finally {
            setCudaDownloading(false);
          }
          return;
        }
      }
      updateConfig({
        gpuAcceleration: value as 'auto' | 'cpu' | 'vulkan' | 'metal' | 'cuda',
      });
    },
    [cudaStatus, t, updateConfig]
  );

  useEffect(() => {
    if (!_isActive) {
      setShowLocalModelContent(false);
      return;
    }

    markSettingsPerf('settings.models.tab.commit');
    const cancelTabFrame = scheduleAfterFrame(() => {
      markSettingsPerf('settings.models.tab.raf');
    });

    return cancelTabFrame;
  }, [_isActive]);

  useEffect(() => {
    if (!_isActive) {
      setShowLocalModelContent(false);
      return;
    }

    markSettingsPerf('settings.models.local.defer.start');
    const cancelDeferredMount = scheduleAfterFrame(() => {
      setShowLocalModelContent(true);
      markSettingsPerf('settings.models.local.defer.end');
    });

    return cancelDeferredMount;
  }, [_isActive]);

  const sectionGroupsByType = useMemo(
    () => new Map(modelCatalog.sections.map((section) => [section.type, section.groups])),
    [modelCatalog.sections]
  );
  const selectionOptions = modelCatalog.selectionOptions;

  const getSectionGroups = useCallback(
    (type: ModelCatalogSectionType) => sectionGroupsByType.get(type) ?? [],
    [sectionGroupsByType]
  );

  const selectedLiveModelId = useMemo(
    () =>
      modelConfig.asr?.selections.live.engine === 'online'
        ? (modelConfig.asr.selections.live.providerId ?? VOLCENGINE_DOUBAO_OPTION_ID)
        : (selectedModelIds.streaming ?? ''),
    [modelConfig.asr?.selections.live, selectedModelIds.streaming]
  );
  const selectedBatchModelId = useMemo(
    () =>
      modelConfig.asr?.selections.batch.engine === 'online'
        ? (modelConfig.asr.selections.batch.providerId ?? VOLCENGINE_DOUBAO_OPTION_ID)
        : (selectedModelIds.batch ?? ''),
    [modelConfig.asr?.selections.batch, selectedModelIds.batch]
  );
  const selectedAsrModelId = isBatchScenario ? selectedBatchModelId : selectedLiveModelId;
  const applyDependencyRequests = (modelId: string) => {
    const dependencyUpdates: Partial<typeof modelConfig> = {};
    const dependencies = modelCatalog.dependencyRequestsByModelId[modelId] ?? [];
    for (const dependency of dependencies) {
      const fieldKeys = [
        scenarioModelFieldKey(dependency.configKey, 'live'),
        scenarioModelFieldKey(dependency.configKey, 'batch'),
      ] as const;
      if (fieldKeys.every((key) => modelConfig[key])) {
        continue;
      }
      if (dependency.isInstalled) {
        for (const key of fieldKeys) {
          if (!modelConfig[key]) {
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
  };

  const handleModelChange = async (type: ModelScenario | 'streaming', modelId: string) => {
    const isLive = type === 'live' || type === 'streaming';
    if (!modelId) {
      if (isLive) {
        const patch = syncLiveAsrSelectionFields(modelConfig, {
          modelId: null,
          modelPath: '',
        });
        updateConfig(patch);
        return;
      }
      updateConfig(
        syncLegacyAsrSelectionFields(modelConfig, 'batch', {
          modelId: null,
          modelPath: '',
        })
      );
      return;
    }

    if (isOnlineAsrProviderId(modelId)) {
      if (isLive) {
        updateConfig(syncLiveOnlineAsrSelectionFields(modelConfig, modelId));
      } else {
        updateConfig(syncOnlineAsrSelectionFields(modelConfig, 'batch', modelId));
      }
      return;
    }

    const path =
      modelCatalog.modelPathById[modelId] ||
      modelCatalog.models.find((model) => model.id === modelId)?.installPath ||
      '';
    if (!path) {
      return;
    }
    if (isLive) {
      const patch = syncLiveAsrSelectionFields(modelConfig, {
        modelId,
        modelPath: path,
      });
      updateConfig(patch);
    } else {
      updateConfig(
        syncLegacyAsrSelectionFields(modelConfig, 'batch', {
          modelId,
          modelPath: path,
        })
      );
    }
    applyDependencyRequests(modelId);
  };

  const handleCompanionModelChange = (kind: ScenarioModelKind, modelId: string) => {
    const configKey = scenarioModelFieldKey(kind, activeScenario);
    if (!modelId) {
      updateConfig({ [configKey]: '' });
      return;
    }

    const path =
      modelCatalog.modelPathById[modelId] ||
      modelCatalog.models.find((model) => model.id === modelId)?.installPath ||
      '';
    if (!path) {
      return;
    }
    updateConfig({ [configKey]: path });
  };

  const sectionProps = {
    installedModels,
    downloads,
    handleDelete,
    handleDownload,
    handleCancelDownload,
  };

  const speakerDisabledOption = useMemo(
    () => ({
      value: '',
      label: t('settings.value_off', { defaultValue: 'Off' }),
    }),
    [t]
  );

  const liveOptions = useMemo(() => {
    return [
      ...toDropdownOptions(selectionOptions.streaming, selectedLiveModelId, true),
      ...ONLINE_ASR_PROVIDER_DEFINITIONS.filter(
        (provider) => provider.id !== GROQ_WHISPER_PROVIDER_ID && provider.defaultConfig
      ) // Groq doesn't support streaming. In future, we can check provider.streaming?.supported !== false. Wait! The definition might not have streaming field directly. Let's just filter groq-whisper directly here to be safe and clean since there's no full manifest typed.
        .filter((provider) => {
          if (provider.id === selectedLiveModelId) return true;
          const providerConfig =
            modelConfig.asr?.providers?.online?.[provider.id] ??
            (provider.id === VOLCENGINE_DOUBAO_PROVIDER_ID
              ? modelConfig.asr?.providers?.volcengineDoubao
              : undefined) ??
            provider.defaultConfig;
          return provider.isConfigured(
            providerConfig as typeof provider.defaultConfig,
            'streaming'
          );
        })
        .map((provider) => {
          const labelText = t(provider.optionLabelKey, {
            defaultValue: provider.optionDefaultLabel,
          });
          return {
            value: provider.id,
            ariaLabel: labelText,
            label: (
              <span style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {labelText}
                <OnlineIcon style={{ color: 'var(--color-text-muted)' }} />
              </span>
            ),
          };
        }),
    ];
  }, [selectedLiveModelId, selectionOptions.streaming, t, modelConfig.asr?.providers]);

  const batchOptions = useMemo(() => {
    return [
      ...toDropdownOptions(selectionOptions.batch, selectedBatchModelId, true),
      ...ONLINE_ASR_PROVIDER_DEFINITIONS.filter((provider) => {
        if (provider.id === selectedBatchModelId) return true;
        const providerConfig =
          modelConfig.asr?.providers?.online?.[provider.id] ??
          (provider.id === VOLCENGINE_DOUBAO_PROVIDER_ID
            ? modelConfig.asr?.providers?.volcengineDoubao
            : undefined) ??
          provider.defaultConfig;
        return provider.isConfigured(providerConfig as typeof provider.defaultConfig, 'batch');
      }).map((provider) => {
        const labelText = t(provider.optionLabelKey, { defaultValue: provider.optionDefaultLabel });
        return {
          value: provider.id,
          ariaLabel: labelText,
          label: (
            <span style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              {labelText}
              <OnlineIcon style={{ color: 'var(--color-text-muted)' }} />
            </span>
          ),
        };
      }),
    ];
  }, [selectedBatchModelId, selectionOptions.batch, t, modelConfig.asr?.providers]);

  const isVolcengineSelected = Object.values(modelConfig.asr?.selections ?? {}).some(
    (selection) =>
      selection.engine === 'online' && selection.providerId === VOLCENGINE_DOUBAO_PROVIDER_ID
  );

  const getSectionStatus = (type: ModelCatalogSectionType) => {
    const groups = getSectionGroups(type);
    const allModels = groups.flatMap((group) => group.models);

    const downloadingModel = allModels.find((model) => !!downloads[model.id]);
    if (downloadingModel) {
      const progress = downloads[downloadingModel.id].progress;
      return {
        type: 'pending',
        text: t('settings.downloading_progress', {
          progress: Math.round(progress),
          defaultValue: `正在下载 (${Math.round(progress)}%)`,
        }),
      };
    }

    const installedCount = allModels.filter((model) => installedModels.has(model.id)).length;
    if (installedCount > 0) {
      return {
        type: 'ready',
        text:
          type === 'vad' || type === 'punctuation' || type === 'alignment'
            ? t('common.ready')
            : t('settings.installed_count', {
                count: installedCount,
              }),
      };
    }

    return {
      type: 'off',
      text: t('settings.not_installed', { defaultValue: '未安装' }),
    };
  };

  const speakerSegmentationOptions = useMemo(() => {
    const installedOptions = toDropdownOptions(
      selectionOptions.speakerSegmentation,
      selectedModelIds.liveSpeakerSegmentation ?? ''
    );
    return [speakerDisabledOption, ...installedOptions];
  }, [
    selectedModelIds.liveSpeakerSegmentation,
    selectionOptions.speakerSegmentation,
    speakerDisabledOption,
  ]);

  const speakerEmbeddingOptions = useMemo(() => {
    const installedOptions = toDropdownOptions(
      selectionOptions.speakerEmbedding,
      selectedModelIds.liveSpeakerEmbedding ?? ''
    );
    return [speakerDisabledOption, ...installedOptions];
  }, [
    selectedModelIds.liveSpeakerEmbedding,
    selectionOptions.speakerEmbedding,
    speakerDisabledOption,
  ]);

  const alignmentOptions = useMemo(() => {
    const selectedId =
      (isBatchScenario ? selectedModelIds.batchAlignment : selectedModelIds.liveAlignment) ?? '';
    const installedOptions = toDropdownOptions(selectionOptions.alignment ?? [], selectedId);
    return [speakerDisabledOption, ...installedOptions];
  }, [
    isBatchScenario,
    selectedModelIds.batchAlignment,
    selectedModelIds.liveAlignment,
    selectionOptions.alignment,
    speakerDisabledOption,
  ]);

  const sectionModelDropdownOptions = useCallback(
    (
      type: ModelCatalogSectionType,
      selectedId: string
    ): Array<{ value: string; label: string }> => {
      const models = getSectionGroups(type).flatMap((group) => group.models as ModelCatalogModel[]);
      return models
        .filter((model) => model.isInstalled || model.id === selectedId)
        .map((model) => ({ value: model.id, label: model.name }));
    },
    [getSectionGroups]
  );

  const punctuationOptions = useMemo(
    () => sectionModelDropdownOptions('punctuation', selectedModelIds.livePunctuation ?? ''),
    [sectionModelDropdownOptions, selectedModelIds.livePunctuation]
  );

  const vadOptions = useMemo(
    () => sectionModelDropdownOptions('vad', selectedModelIds.liveVad ?? ''),
    [sectionModelDropdownOptions, selectedModelIds.liveVad]
  );

  const activeAsrRulesBadge = useMemo(() => {
    const selection = isBatchScenario
      ? modelConfig.asr?.selections.batch
      : modelConfig.asr?.selections.live;
    if (selection?.engine !== 'local' || !selection.modelPath.trim()) {
      return null;
    }
    const modelInfo = selection.modelId
      ? undefined
      : findSelectedModelByMode(selection.modelPath, isBatchScenario ? 'batch' : 'live');
    const modelId = selection.modelId ?? modelInfo?.id;
    if (!modelId) {
      return null;
    }
    const rules = modelService.getModelRules(modelId);
    const parts = [
      rules.requiresVad
        ? t('settings.advanced_requires_vad', { defaultValue: '需要 VAD' })
        : t('settings.advanced_no_vad', { defaultValue: '不需要 VAD' }),
      rules.requiresPunctuation
        ? t('settings.advanced_requires_punct', { defaultValue: '需要标点' })
        : t('settings.advanced_no_punct', { defaultValue: '不需要标点' }),
    ];
    return parts.join(' · ');
  }, [isBatchScenario, modelConfig.asr?.selections.batch, modelConfig.asr?.selections.live, t]);

  return (
    <SettingsTabContainer id="settings-panel-models" ariaLabelledby="settings-tab-models">
      <SettingsPageHeader
        icon={<ModelIcon width={28} height={28} />}
        title={t('settings.model_hub')}
        description={t('settings.model_settings_description')}
      />
      <SettingsSection
        title={t('settings.model_selection')}
        description={t('settings.model_selection_desc')}
        icon={<Settings2 size={20} />}
      >
        <div
          id="settings-model-scenario"
          className="settings-scenario-cards"
          role="tablist"
          aria-label={t('settings.scenario_selector_label', { defaultValue: 'Model scenario' })}
        >
          {[
            {
              value: 'live' as ModelScenario,
              icon: <Mic size={18} />,
              label: t('settings.scenario_live', { defaultValue: '实时录音' }),
              description: t('settings.scenario_live_desc', {
                defaultValue: '麦克风说话，实时出字',
              }),
            },
            {
              value: 'batch' as ModelScenario,
              icon: <PlaySquare size={18} />,
              label: t('settings.scenario_batch', { defaultValue: '批量导入' }),
              description: t('settings.scenario_batch_desc', {
                defaultValue: '导入音视频文件，离线批量转写',
              }),
            },
          ].map((option) => (
            <button
              key={option.value}
              type="button"
              role="tab"
              aria-selected={activeScenario === option.value}
              aria-label={option.label}
              className={`settings-scenario-card${activeScenario === option.value ? ' active' : ''}`}
              onClick={() => setActiveScenario(option.value)}
            >
              <span className="settings-scenario-card-icon">{option.icon}</span>
              <span className="settings-scenario-card-text">
                <span className="settings-scenario-card-label">{option.label}</span>
                <span className="settings-scenario-card-description">{option.description}</span>
              </span>
            </button>
          ))}
        </div>

        <SettingsItem
          title={t('settings.asr_model_label', { defaultValue: '识别模型' })}
          hint={
            isBatchScenario ? t('settings.batch_model_hint') : t('settings.streaming_model_hint')
          }
        >
          <div style={{ width: '220px' }}>
            <Dropdown
              id={isBatchScenario ? 'settings-batch-path' : 'settings-streaming-path'}
              value={selectedAsrModelId}
              onChange={(value) => handleModelChange(activeScenario, value)}
              placeholder={
                isBatchScenario
                  ? t('settings.select_batch_model')
                  : t('settings.select_streaming_model')
              }
              options={isBatchScenario ? batchOptions : liveOptions}
              style={{ flex: 1 }}
              disabled={localModelActionsDisabled}
            />
          </div>
        </SettingsItem>

        <SettingsItem
          title={t('settings.speaker_segmentation_model_label', {
            defaultValue: 'Speaker Segmentation Model',
          })}
          hint={t('settings.speaker_segmentation_model_hint', {
            defaultValue: 'Used to split recordings into anonymous speaker turns.',
          })}
        >
          <div style={{ width: '220px' }}>
            <Dropdown
              id="settings-speaker-segmentation-path"
              value={
                (isBatchScenario
                  ? selectedModelIds.batchSpeakerSegmentation
                  : selectedModelIds.liveSpeakerSegmentation) ?? ''
              }
              onChange={(value) =>
                handleCompanionModelChange('speakerSegmentationModelPath', value)
              }
              placeholder={t('settings.select_speaker_segmentation_model', {
                defaultValue: 'Select speaker segmentation model',
              })}
              options={speakerSegmentationOptions}
              style={{ flex: 1 }}
              aria-label={t('settings.speaker_segmentation_model_label', {
                defaultValue: 'Speaker Segmentation Model',
              })}
              disabled={localModelActionsDisabled}
            />
          </div>
        </SettingsItem>

        <SettingsItem
          title={t('settings.speaker_embedding_model_label', {
            defaultValue: 'Speaker Embedding Model',
          })}
          hint={t('settings.speaker_embedding_model_hint', {
            defaultValue: 'Used to match diarized speakers against your known speaker profiles.',
          })}
        >
          <div style={{ width: '220px' }}>
            <Dropdown
              id="settings-speaker-embedding-path"
              value={
                (isBatchScenario
                  ? selectedModelIds.batchSpeakerEmbedding
                  : selectedModelIds.liveSpeakerEmbedding) ?? ''
              }
              onChange={(value) => handleCompanionModelChange('speakerEmbeddingModelPath', value)}
              placeholder={t('settings.select_speaker_embedding_model', {
                defaultValue: 'Select speaker embedding model',
              })}
              options={speakerEmbeddingOptions}
              style={{ flex: 1 }}
              aria-label={t('settings.speaker_embedding_model_label', {
                defaultValue: 'Speaker Embedding Model',
              })}
              disabled={localModelActionsDisabled}
            />
          </div>
        </SettingsItem>

        <SettingsItem
          title={t('settings.alignment_model_label', {
            defaultValue: 'CTC 对齐模型',
          })}
          hint={t('settings.alignment_model_hint', {
            defaultValue: '用于生成字/词级时间戳并优化说话人切分边界。',
          })}
        >
          <div style={{ width: '220px' }}>
            <Dropdown
              id="settings-alignment-path"
              value={
                (isBatchScenario
                  ? selectedModelIds.batchAlignment
                  : selectedModelIds.liveAlignment) ?? ''
              }
              onChange={(value) => handleCompanionModelChange('alignmentModelPath', value)}
              placeholder={t('settings.select_alignment_model', {
                defaultValue: '选择对齐模型...',
              })}
              options={alignmentOptions}
              style={{ flex: 1 }}
              aria-label={t('settings.alignment_model_label', {
                defaultValue: 'CTC 对齐模型',
              })}
              disabled={localModelActionsDisabled}
            />
          </div>
        </SettingsItem>

        <SettingsAccordion
          title={t('settings.advanced_settings_title', { defaultValue: '高级设置' })}
          status={
            activeAsrRulesBadge ? (
              <span className="status-badge ready">{activeAsrRulesBadge}</span>
            ) : undefined
          }
        >
          <SettingsItem
            title={t('settings.punctuation_model_label', { defaultValue: '标点模型' })}
            hint={t('settings.punctuation_rule_hint', {
              defaultValue: '仅当所选识别模型需要标点时才会启用。',
            })}
          >
            <div style={{ width: '220px' }}>
              <Dropdown
                id="settings-punctuation-path"
                value={
                  (isBatchScenario
                    ? selectedModelIds.batchPunctuation
                    : selectedModelIds.livePunctuation) ?? ''
                }
                onChange={(value) => handleCompanionModelChange('punctuationModelPath', value)}
                placeholder={t('settings.select_punctuation_model', {
                  defaultValue: 'Select punctuation model',
                })}
                options={punctuationOptions}
                style={{ flex: 1 }}
                aria-label={t('settings.punctuation_model_label', { defaultValue: '标点模型' })}
                disabled={localModelActionsDisabled}
              />
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('settings.vad_model_label', { defaultValue: 'VAD 模型' })}
            hint={t('settings.vad_rule_hint', {
              defaultValue: '仅当所选识别模型需要 VAD 时才会启用。',
            })}
          >
            <div style={{ width: '220px' }}>
              <Dropdown
                id="settings-vad-path"
                value={
                  (isBatchScenario ? selectedModelIds.batchVad : selectedModelIds.liveVad) ?? ''
                }
                onChange={(value) => handleCompanionModelChange('vadModelPath', value)}
                placeholder={t('settings.select_vad_model', { defaultValue: 'Select VAD model' })}
                options={vadOptions}
                style={{ flex: 1 }}
                aria-label={t('settings.vad_model_label', { defaultValue: 'VAD 模型' })}
                disabled={localModelActionsDisabled}
              />
            </div>
          </SettingsItem>

          <SettingsItem title={t('settings.vad_buffer_size')} hint={t('settings.vad_buffer_hint')}>
            <div style={{ width: '120px' }}>
              <input
                id="settings-vad-buffer"
                type="number"
                className="settings-input"
                value={activeVadBufferSize}
                onChange={(e) =>
                  updateConfig(
                    isBatchScenario
                      ? { batchVadBufferSize: Number(e.target.value) }
                      : { liveVadBufferSize: Number(e.target.value) }
                  )
                }
                min={0}
                max={30}
                step={0.5}
                style={{ textAlign: 'center' }}
              />
            </div>
          </SettingsItem>

          {isBatchScenario && (
            <SettingsItem
              title={t('settings.batch_vad_enabled')}
              hint={
                isBatchVadForced
                  ? t('settings.batch_vad_forced_hint')
                  : t('settings.batch_vad_enabled_hint')
              }
            >
              <Switch
                checked={isBatchVadForced || batchVadEnabled}
                disabled={isBatchVadForced}
                onChange={(checked) => updateConfig({ batchVadEnabled: checked })}
              />
            </SettingsItem>
          )}
        </SettingsAccordion>

        {isVolcengineSelected && (
          <div className="settings-hint">
            {t(onlineAsrProvider.onlineUploadHintKey, {
              defaultValue: onlineAsrProvider.onlineUploadHintDefault,
            })}
          </div>
        )}
      </SettingsSection>

      {showLocalModelContent ? (
        <LocalModelManagementSection
          catalogLoadState={catalogLoadState}
          catalogLoadError={catalogLoadError}
          sectionProps={sectionProps}
          localModelActionsDisabled={localModelActionsDisabled}
          getSectionGroups={getSectionGroups}
          getSectionStatus={getSectionStatus}
          t={t}
        />
      ) : (
        <SettingsSection
          title={t('settings.batch_model_management', { defaultValue: '离线模型管理' })}
          icon={<RestoreIcon />}
        >
          {catalogLoadState === 'loading' && (
            <div className="settings-hint" role="status">
              {t('settings.models_checking_local', { defaultValue: 'Checking local models...' })}
            </div>
          )}
          {catalogLoadState === 'error' && (
            <div className="settings-hint" role="status">
              {t('settings.models_check_failed', {
                error: catalogLoadError ?? '',
                defaultValue:
                  'Could not check local models: {{error}}. Download status may be stale.',
              })}
            </div>
          )}
        </SettingsSection>
      )}

      <SettingsSection
        title={t('settings.online_model_management', { defaultValue: '在线模型管理' })}
        icon={<Settings2 size={20} />}
      >
        {ONLINE_ASR_PROVIDER_DEFINITIONS.map((provider) => {
          const Component = CUSTOM_PROVIDER_COMPONENTS[provider.id] || DynamicProviderSettings;
          return <Component key={provider.id} provider={provider} />;
        })}
      </SettingsSection>

      <SettingsSection
        title={t('settings.transcription_settings')}
        icon={<PlaySquare size={20} />}
        description={t('settings.transcription_settings_hint')}
      >
        <SettingsItem title={t('settings.enable_itn')} hint={t('settings.enable_itn_hint')}>
          <Switch
            checked={enableITN}
            onChange={(checked) => updateConfig({ enableITN: checked })}
          />
        </SettingsItem>

        <SettingsItem
          title={t('settings.max_concurrent_label')}
          hint={t('settings.max_concurrent_hint')}
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

        <SettingsItem
          title={t('settings.gpu_acceleration_label', { defaultValue: 'GPU Acceleration' })}
          hint={t('settings.gpu_acceleration_hint', {
            defaultValue:
              'Hardware acceleration for local models. Vulkan is for Windows/Linux, Metal is for Apple Silicon.',
          })}
        >
          {(() => {
            const isMac =
              typeof navigator !== 'undefined' &&
              /Mac|iPhone|iPad|iPod/i.test(navigator.platform || navigator.userAgent || '');
            return (
              <div style={{ width: '140px' }}>
                <Dropdown
                  id="settings-gpu-acceleration"
                  value={gpuAcceleration}
                  disabled={cudaDownloading}
                  onChange={handleGpuAccelerationChange}
                  options={[
                    {
                      value: 'auto',
                      label: t('settings.gpu_acceleration_auto', { defaultValue: 'Auto' }),
                    },
                    {
                      value: 'cpu',
                      label: t('settings.value_off', { defaultValue: 'Off' }),
                    },
                    {
                      value: 'vulkan',
                      label: 'Vulkan',
                      description: isMac
                        ? t('settings.gpu_acceleration_not_supported_on_macos', {
                            defaultValue: 'Not supported on macOS',
                          })
                        : t('settings.gpu_acceleration_vulkan_desc', {
                            defaultValue: 'Cross-vendor GPU acceleration',
                          }),
                      disabled: isMac,
                    },
                    {
                      value: 'metal',
                      label: 'Metal',
                      description: isMac
                        ? t('settings.gpu_acceleration_metal_desc', {
                            defaultValue: 'Apple native GPU acceleration',
                          })
                        : t('settings.gpu_acceleration_macos_only', { defaultValue: 'macOS only' }),
                      disabled: !isMac,
                    },
                    {
                      value: 'cuda',
                      label: 'CUDA',
                      description: isMac
                        ? t('settings.gpu_acceleration_not_supported_on_macos', {
                            defaultValue: 'Not supported on macOS',
                          })
                        : cudaStatus?.isInstalled
                          ? `${t('settings.gpu_acceleration_cuda_desc', { defaultValue: 'NVIDIA dedicated acceleration' })} (${t('settings.cuda_addon_status_installed', { defaultValue: 'Ready' })})`
                          : `${t('settings.gpu_acceleration_cuda_desc', { defaultValue: 'NVIDIA dedicated acceleration' })} (${t('settings.cuda_addon_status_not_installed', { defaultValue: 'Not Installed (Click to Download)' })})`,
                      disabled: isMac,
                    },
                  ]}
                  style={{ flex: 1 }}
                />
                {cudaDownloading && (
                  <div
                    style={{
                      marginTop: '6px',
                      fontSize: '0.75rem',
                      color: 'var(--color-text-muted)',
                      width: '220px',
                    }}
                  >
                    {t('settings.cuda_addon_downloading', {
                      percent: cudaProgress,
                      defaultValue: `Downloading (${cudaProgress}%)...`,
                    })}
                    <div
                      style={{
                        width: '100%',
                        height: '4px',
                        background: 'var(--border)',
                        borderRadius: '2px',
                        marginTop: '4px',
                        overflow: 'hidden',
                      }}
                    >
                      <div
                        style={{
                          width: `${cudaProgress}%`,
                          height: '100%',
                          background: 'var(--color-primary)',
                          transition: 'width 0.2s ease',
                        }}
                      />
                    </div>
                  </div>
                )}
              </div>
            );
          })()}
        </SettingsItem>
      </SettingsSection>

      <div style={{ display: 'flex', justifyContent: 'flex-start', marginTop: '8px' }}>
        <button
          className="btn btn-restore-defaults"
          onClick={restoreDefaultModelSettings}
          disabled={localModelActionsDisabled}
          aria-label={t('settings.restore_defaults')}
        >
          <RestoreIcon />
          {t('settings.restore_defaults')}
        </button>
      </div>
    </SettingsTabContainer>
  );
}
