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
import { isPrimarilySectionAliasQuery, resolveCatalogSectionAliases } from './catalogSearch';
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

const CATALOG_SECTION_ORDER: ModelCatalogSectionType[] = [
  'asr',
  'punctuation',
  'vad',
  'speaker-segmentation',
  'speaker-embedding',
  'alignment',
];

/** Stable DOM ids for Advanced catalog groups (also scanned by search-registry integrity). */
export const SETTINGS_CATALOG_SECTION_IDS: Record<ModelCatalogSectionType, string> = {
  asr: 'settings-catalog-asr',
  punctuation: 'settings-catalog-punctuation',
  vad: 'settings-catalog-vad',
  'speaker-segmentation': 'settings-catalog-speaker-segmentation',
  'speaker-embedding': 'settings-catalog-speaker-embedding',
  alignment: 'settings-catalog-alignment',
};

export interface LocalModelManagementSectionProps {
  catalogLoadState: ModelManagerContextType['catalogLoadState'];
  catalogLoadError: ModelManagerContextType['catalogLoadError'];
  sectionProps: Pick<
    ModelManagerContextType,
    | 'installedModels'
    | 'downloads'
    | 'handleDelete'
    | 'handleDownload'
    | 'handleCancelDownload'
    | 'handleDiscardDownload'
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
  // Default toward Recommended + Installed; users can switch to All Tags for power browsing.
  const [labelFilter, setLabelFilter] = useState<'all' | ModelLabel | 'recommended'>('recommended');
  const [searchQuery, setSearchQuery] = useState('');
  const [forcedOpenSections, setForcedOpenSections] = useState<
    Partial<Record<ModelCatalogSectionType, boolean>>
  >({});

  const engineFilterOptions = [
    { value: 'all', label: t('settings.model_filter_engine_all', { defaultValue: 'All Engines' }) },
    { value: 'sherpa-onnx', label: 'ONNX' },
    { value: 'llama-cpp', label: 'GGUF' },
  ];

  const labelFilterOptions = [
    { value: 'all', label: t('settings.model_filter_tag_all', { defaultValue: 'All Tags' }) },
    {
      value: 'recommended',
      label: t('common.recommended', { defaultValue: 'Recommended' }),
    },
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

  const aliasMatchedSections = useMemo(
    () => resolveCatalogSectionAliases(searchQuery),
    [searchQuery]
  );
  const aliasOnlyQuery = useMemo(() => isPrimarilySectionAliasQuery(searchQuery), [searchQuery]);
  const isSearching = searchQuery.trim().length > 0;

  const filterModels = useCallback(
    (
      models: ModelCatalogModel[],
      sectionType: ModelCatalogSectionType,
      options?: { applyRecommended?: boolean }
    ) => {
      const query = searchQuery.trim().toLowerCase();
      const sectionAliasHit = aliasMatchedSections.includes(sectionType);
      const applyRecommended = options?.applyRecommended !== false;
      return models.filter((model) => {
        if (engineFilter !== 'all' && model.engine !== engineFilter) return false;
        // Active search bypasses the Recommended default so aliases like "silero" still hit.
        if (!query && applyRecommended) {
          if (labelFilter === 'recommended') {
            if (!model.isRecommended && !sectionProps.installedModels.has(model.id)) {
              return false;
            }
          } else if (
            labelFilter !== 'all' &&
            !(resolveModelLabels(model) ?? []).includes(labelFilter)
          ) {
            return false;
          }
        }
        const isInstalled = sectionProps.installedModels.has(model.id);
        const isDownloading = !!sectionProps.downloads[model.id];
        if (statusFilter === 'installed' && !isInstalled) return false;
        if (statusFilter === 'not-installed' && (isInstalled || isDownloading)) return false;
        if (statusFilter === 'downloading' && !isDownloading) return false;
        if (query) {
          // Pure section-alias queries (e.g. "vad") show every model in that section.
          if (aliasOnlyQuery && sectionAliasHit) {
            return true;
          }
          const haystack =
            `${model.id} ${model.name} ${model.versionLabel ?? ''} ${model.groupId ?? ''} ${model.type} ${sectionType}`.toLowerCase();
          if (!haystack.includes(query) && !sectionAliasHit) {
            return false;
          }
          // Alias hit without id/name match: still include so the group expands.
          if (!haystack.includes(query) && sectionAliasHit && !aliasOnlyQuery) {
            // For mixed queries like "silero vad", rely on haystack; alias alone is insufficient.
            if (!query.split(/\s+/).some((token) => haystack.includes(token))) {
              return false;
            }
          }
        }
        return true;
      });
    },
    [
      aliasMatchedSections,
      aliasOnlyQuery,
      engineFilter,
      labelFilter,
      searchQuery,
      statusFilter,
      sectionProps.downloads,
      sectionProps.installedModels,
    ]
  );

  const filteredGroupsByType = useMemo(() => {
    return new Map(
      CATALOG_SECTION_ORDER.map((type) => {
        const sourceGroups = getSectionGroups(type);
        let groups = sourceGroups
          .map((group) => ({
            ...group,
            models: filterModels(group.models as ModelCatalogModel[], type),
          }))
          .filter((group) => group.models.length > 0);

        // Recommended default must not blank companion sections that have no
        // recommended flags yet — fall back to browsing all models there.
        if (
          !searchQuery.trim() &&
          labelFilter === 'recommended' &&
          groups.length === 0 &&
          type !== 'asr'
        ) {
          groups = sourceGroups
            .map((group) => ({
              ...group,
              models: filterModels(group.models as ModelCatalogModel[], type, {
                applyRecommended: false,
              }),
            }))
            .filter((group) => group.models.length > 0);
        }

        return [type, groups] as const;
      })
    );
  }, [filterModels, getSectionGroups, labelFilter, searchQuery]);

  useEffect(() => {
    if (!isSearching) {
      setForcedOpenSections({});
      return;
    }
    const next: Partial<Record<ModelCatalogSectionType, boolean>> = {};
    for (const [type, groups] of filteredGroupsByType.entries()) {
      if (groups.length > 0 || aliasMatchedSections.includes(type)) {
        next[type] = true;
      }
    }
    setForcedOpenSections(next);
  }, [aliasMatchedSections, filteredGroupsByType, isSearching]);

  const hasLoadedSections = useMemo(
    () => Array.from(filteredGroupsByType.values()).some((groups) => groups.length > 0),
    [filteredGroupsByType]
  );

  const isCatalogLoading = catalogLoadState === 'loading';
  const isCatalogReady = catalogLoadState === 'ready';

  useEffect(() => {
    markSettingsPerf('settings.models.local.content.commit');
    const cancelFrame = scheduleAfterFrame(() => {
      markSettingsPerf('settings.models.local.content.raf');
    });
    return cancelFrame;
  }, []);

  const sectionTitle = (type: ModelCatalogSectionType): string => {
    switch (type) {
      case 'asr':
        return t('settings.recognition_models');
      case 'punctuation':
        return t('settings.punctuation_models');
      case 'vad':
        return t('settings.vad_models');
      case 'speaker-segmentation':
        return t('settings.speaker_segmentation_models', {
          defaultValue: 'Speaker Segmentation Models',
        });
      case 'speaker-embedding':
        return t('settings.speaker_embedding_models', {
          defaultValue: 'Speaker Embedding Models',
        });
      case 'alignment':
        return t('settings.alignment_models', {
          defaultValue: 'CTC Alignment Models',
        });
      default:
        return type;
    }
  };

  const renderSection = (type: ModelCatalogSectionType) => {
    const groups = filteredGroupsByType.get(type) ?? [];
    const hasMatches = groups.length > 0;
    // While searching, hide empty Recognition (and peers) instead of "No matching models"
    // sitting open above a collapsed VAD hit.
    if (isSearching && !hasMatches) {
      return null;
    }

    const openProp = isSearching
      ? Boolean(forcedOpenSections[type] || hasMatches)
      : type === 'asr'
        ? undefined
        : forcedOpenSections[type]
          ? true
          : undefined;

    return (
      <div key={type} id={SETTINGS_CATALOG_SECTION_IDS[type]}>
        <SettingsAccordion
          title={sectionTitle(type)}
          status={
            <span className={`status-badge ${getSectionStatus(type).type}`}>
              {getSectionStatus(type).text}
            </span>
          }
          defaultOpen={type === 'asr'}
          isOpen={openProp}
        >
          {groups.map((group) => (
            <ModelCard
              key={group.key}
              models={group.models}
              isAsr={type === 'asr'}
              installedModels={sectionProps.installedModels}
              downloads={sectionProps.downloads}
              onDelete={sectionProps.handleDelete}
              onDownload={sectionProps.handleDownload}
              onCancelDownload={sectionProps.handleCancelDownload}
              onDiscardDownload={sectionProps.handleDiscardDownload}
              actionsDisabled={localModelActionsDisabled}
            />
          ))}
          {!hasMatches && (
            <div className="settings-model-empty">
              {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
            </div>
          )}
        </SettingsAccordion>
      </div>
    );
  };

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
            onChange={(value) => setLabelFilter(value as 'all' | ModelLabel | 'recommended')}
            options={labelFilterOptions}
            style={{ width: '140px' }}
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
      {isCatalogLoading && !hasLoadedSections && (
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
      {(isCatalogReady || hasLoadedSections) && (
        <>
          {CATALOG_SECTION_ORDER.map((type) => renderSection(type))}
          {isSearching &&
            CATALOG_SECTION_ORDER.every(
              (type) => (filteredGroupsByType.get(type)?.length ?? 0) === 0
            ) && (
              <div className="settings-model-empty">
                {t('settings.model_filter_no_match', { defaultValue: 'No matching models' })}
              </div>
            )}
        </>
      )}
    </SettingsSection>
  );
}
