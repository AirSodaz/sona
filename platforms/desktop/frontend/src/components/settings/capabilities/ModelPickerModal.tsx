import { Download, Loader2, Search, X } from 'lucide-react';
import type React from 'react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import type { ModelCatalogModel } from '../../../types/modelCatalog';
import { resolveModelLabels } from '../../../types/modelCatalog';
import { Modal } from '../../Modal';
import { LARGE_DOWNLOAD_BYTES, resolveModelDownloadBytes } from '../../../utils/modelDownloadSize';

export interface ModelPickerModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called after a model is selected (and download kicked off if needed). */
  onSelected?: (model: ModelCatalogModel) => void;
}

function getModelBytes(model: ModelCatalogModel): number | null {
  return resolveModelDownloadBytes(model);
}

function pickPreferredVariant(models: ModelCatalogModel[]): ModelCatalogModel {
  const recommended = models.find((model) => model.isRecommended && model.isInstalled);
  if (recommended) {
    return recommended;
  }
  const installed = models.find((model) => model.isInstalled);
  if (installed) {
    return installed;
  }
  const recommendedAny = models.find((model) => model.isRecommended);
  if (recommendedAny) {
    return recommendedAny;
  }
  return [...models].sort((a, b) => {
    const aBytes = getModelBytes(a) ?? Number.MAX_SAFE_INTEGER;
    const bBytes = getModelBytes(b) ?? Number.MAX_SAFE_INTEGER;
    return aBytes - bBytes;
  })[0];
}

/**
 * Task-oriented ASR picker: select = download (if needed) + bind.
 * Defaults to Recommended + Installed, one preferred size per family.
 */
export function ModelPickerModal({
  isOpen,
  onClose,
  onSelected,
}: ModelPickerModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const { modelCatalog, downloads, handleDownload, handleLoad, catalogLoadState } =
    useModelManagerContext();
  const [query, setQuery] = useState('');
  const [showAllSizes, setShowAllSizes] = useState<Record<string, boolean>>({});
  const [busyId, setBusyId] = useState<string | null>(null);

  const asrGroups = useMemo(() => {
    const section = modelCatalog.sections?.find((item) => item.type === 'asr');
    return section?.groups ?? [];
  }, [modelCatalog.sections]);

  const families = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return asrGroups
      .map((group) => {
        const preferred = pickPreferredVariant(group.models);
        const hasRecommended = group.models.some((model) => model.isRecommended);
        const hasInstalled = group.models.some((model) => model.isInstalled);
        const searchable = `${group.key} ${group.models.map((model) => `${model.name} ${model.id} ${model.versionLabel ?? ''}`).join(' ')}`.toLowerCase();
        const matchesQuery = !normalizedQuery || searchable.includes(normalizedQuery);
        const matchesDefaultView =
          Boolean(normalizedQuery) || hasRecommended || hasInstalled || preferred.isInstalled;
        return {
          key: group.key,
          preferred,
          models: group.models,
          hasRecommended,
          hasInstalled,
          matchesQuery,
          matchesDefaultView,
        };
      })
      .filter((family) => family.matchesQuery && family.matchesDefaultView)
      .sort((a, b) => {
        if (a.hasInstalled !== b.hasInstalled) {
          return a.hasInstalled ? -1 : 1;
        }
        if (a.hasRecommended !== b.hasRecommended) {
          return a.hasRecommended ? -1 : 1;
        }
        return a.preferred.name.localeCompare(b.preferred.name);
      });
  }, [asrGroups, query]);

  if (!isOpen) {
    return null;
  }

  const selectModel = async (model: ModelCatalogModel) => {
    setBusyId(model.id);
    try {
      if (model.isInstalled) {
        await handleLoad(model);
      } else {
        await handleDownload(model);
      }
      onSelected?.(model);
      onClose();
    } finally {
      setBusyId(null);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('settings.capabilities.pick_speech_model', {
        defaultValue: 'Choose a speech model',
      })}
      size="lg"
    >
      <div className="capability-picker">
        <label className="capability-picker-search">
          <Search size={16} aria-hidden />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('settings.capabilities.picker_search_placeholder', {
              defaultValue: 'Search models…',
            })}
            autoFocus
          />
          {query && (
            <button
              type="button"
              className="capability-picker-clear"
              onClick={() => setQuery('')}
              aria-label={t('common.clear', { defaultValue: 'Clear' })}
            >
              <X size={14} />
            </button>
          )}
        </label>

        <p className="settings-hint">
          {t('settings.capabilities.picker_hint', {
            defaultValue:
              'Selecting a model downloads it if needed and sets it for Live and Batch recognition.',
          })}
        </p>

        {catalogLoadState === 'loading' && (
          <div className="capability-picker-empty" role="status">
            <Loader2 className="spin" size={18} />
            {t('settings.models_checking_local', { defaultValue: 'Checking local models...' })}
          </div>
        )}

        {catalogLoadState === 'ready' && families.length === 0 && (
          <div className="capability-picker-empty">
            {t('settings.capabilities.picker_empty', {
              defaultValue: 'No matching models. Try clearing filters or search.',
            })}
          </div>
        )}

        <ul className="capability-picker-list">
          {families.map((family) => {
            const expanded = showAllSizes[family.key];
            const modelsToShow = expanded ? family.models : [family.preferred];
            return (
              <li key={family.key} className="capability-picker-family">
                {modelsToShow.map((model) => {
                  const downloading = Boolean(downloads[model.id]);
                  const busy = busyId === model.id || downloading;
                  const bytes = getModelBytes(model);
                  const large = bytes !== null && bytes >= LARGE_DOWNLOAD_BYTES;
                  const labels = resolveModelLabels(model) ?? [];
                  return (
                    <button
                      key={model.id}
                      type="button"
                      className="capability-picker-row"
                      disabled={busy}
                      onClick={() => void selectModel(model)}
                    >
                      <span className="capability-picker-row-main">
                        <span className="capability-picker-row-title">
                          {model.name}
                          {model.versionLabel ? (
                            <span className="capability-picker-row-version">
                              {model.versionLabel}
                            </span>
                          ) : null}
                        </span>
                        <span className="capability-picker-row-meta">
                          {model.size}
                          {model.isRecommended
                            ? ` · ${t('common.recommended', { defaultValue: 'Recommended' })}`
                            : ''}
                          {model.isInstalled
                            ? ` · ${t('settings.model_filter_status_installed', { defaultValue: 'Installed' })}`
                            : ''}
                          {large && !model.isInstalled
                            ? ` · ${t('settings.capabilities.large_download', { defaultValue: 'Large download' })}`
                            : ''}
                          {labels.length > 0 ? ` · ${labels.join(', ')}` : ''}
                        </span>
                      </span>
                      <span className="capability-picker-row-action">
                        {busy ? (
                          <Loader2 className="spin" size={16} />
                        ) : model.isInstalled ? (
                          t('common.select', { defaultValue: 'Select' })
                        ) : (
                          <>
                            <Download size={14} />
                            {t('common.download', { defaultValue: 'Download' })}
                          </>
                        )}
                      </span>
                    </button>
                  );
                })}
                {family.models.length > 1 && (
                  <button
                    type="button"
                    className="capability-picker-more"
                    onClick={() =>
                      setShowAllSizes((prev) => ({
                        ...prev,
                        [family.key]: !prev[family.key],
                      }))
                    }
                  >
                    {expanded
                      ? t('settings.capabilities.hide_sizes', {
                          defaultValue: 'Hide other sizes',
                        })
                      : t('settings.capabilities.show_sizes', {
                          defaultValue: 'More sizes ({{count}})',
                          count: family.models.length - 1,
                        })}
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      </div>
    </Modal>
  );
}
