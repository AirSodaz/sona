import { Search, X } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ModelManagerContextType } from '../../hooks/useModelManager';
import type { LocalAsrEngine } from '../../types/asr';
import type {
  ModelCatalogModel,
  ModelCatalogSectionType,
  ModelLabel,
} from '../../types/modelCatalog';
import { resolveModelLabels } from '../../types/modelCatalog';
import { markSettingsPerf } from '../../utils/settingsPerf';
import { Dropdown } from '../Dropdown';
import { RestoreIcon } from '../Icons';
import { MirrorDownloadPicker } from './MirrorDownloadPicker';
import { ModelCard } from './ModelCard';
import { SettingsAccordion, SettingsSection } from './SettingsLayout';

function scheduleAfterFrame(callback: () => void): () => void {
  if (typeof requestAnimationFrame === 'function') {
    const frameId = requestAnimationFrame(() => callback());
    return () => cancelAnimationFrame(frameId);
  }

  const timeoutId = window.setTimeout(callback, 0);
  return () => window.clearTimeout(timeoutId);
}

export interface LocalModelManagementSectionProps {
  catalogLoadState: ModelManagerContextType['catalogLoadState'];
  catalogLoadError: ModelManagerContextType['catalogLoadError'];
  sectionProps: Pick<
    ModelManagerContextType,
    'installedModels' | 'downloads' | 'handleDelete' | 'handleDownload' | 'handleCancelDownload'
  >;
  localModelActionsDisabled: boolean;
  getSectionGroups: (
    type: ModelCatalogSectionType
  ) => ModelManagerContextType['modelCatalog']['sections'][number]['groups'];
  getSectionStatus: (type: ModelCatalogSectionType) => { type: string; text: string };
  t: (key: string, options?: Record<string, unknown>) => string;
}

export function LocalModelManagementSection({
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
    { value: 'all', label: t('settings.model_filter_engine_all', { defaultValue: 'All Engines' }) },
    { value: 'sherpa-onnx', label: 'ONNX' },
    { value: 'llama-cpp', label: 'GGUF' },
  ];

  const labelFilterOptions = [
    { value: 'all', label: t('settings.model_filter_tag_all', { defaultValue: 'All Tags' }) },
    { value: 'accurate', label: t('settings.model_tag_accurate', { defaultValue: 'Accurate' }) },
    { value: 'lite', label: t('settings.model_tag_lite', { defaultValue: 'Lite' }) },
  ];

  const statusFilterOptions = [
    {
      value: 'all',
      label: t('settings.model_filter_status_all', { defaultValue: 'All Statuses' }),
    },
    {
      value: 'installed',
      label: t('settings.model_filter_status_installed', { defaultValue: 'Installed' }),
    },
    {
      value: 'not-installed',
      label: t('settings.model_filter_status_not_installed', { defaultValue: 'Not Installed' }),
    },
    {
      value: 'downloading',
      label: t('settings.model_filter_status_downloading', { defaultValue: 'Downloading' }),
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
      title={t('settings.batch_model_management', { defaultValue: 'Local Model Management' })}
      icon={<RestoreIcon />}
      actions={<MirrorDownloadPicker />}
    >
      {isCatalogReady && (
        <div className="settings-model-toolbar">
          <Dropdown
            id="settings-model-engine-filter"
            aria-label={t('settings.model_filter_engine_label', {
              defaultValue: 'Filter by engine',
            })}
            value={engineFilter}
            onChange={(value) => setEngineFilter(value as 'all' | LocalAsrEngine)}
            options={engineFilterOptions}
            style={{ width: '130px' }}
          />
          <Dropdown
            id="settings-model-status-filter"
            aria-label={t('settings.model_filter_status_label', {
              defaultValue: 'Filter by status',
            })}
            value={statusFilter}
            onChange={(value) => setStatusFilter(value as typeof statusFilter)}
            options={statusFilterOptions}
            style={{ width: '130px' }}
          />
          <Dropdown
            id="settings-model-label-filter"
            aria-label={t('settings.model_filter_tag_label', { defaultValue: 'Filter by tag' })}
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
                defaultValue: 'Search models…',
              })}
              aria-label={t('settings.model_filter_search_placeholder', {
                defaultValue: 'Search models…',
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
                aria-label={t('settings.model_search_clear', { defaultValue: 'Clear search' })}
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
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
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
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
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
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
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
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
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
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
              </div>
            )}
          </SettingsAccordion>

          <SettingsAccordion
            title={t('settings.alignment_models', {
              defaultValue: 'CTC Alignment Models',
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
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
              </div>
            )}
          </SettingsAccordion>
        </>
      )}
    </SettingsSection>
  );
}
