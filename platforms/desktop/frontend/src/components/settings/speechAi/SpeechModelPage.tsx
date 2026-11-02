import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { asrConfigService, buildModelSelectionPatch } from '../../../services/asrConfigService';
import { useConfigStore, useSetConfig } from '../../../stores/configStore';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import type { AppConfig } from '../../../types/config';
import type { ModelInfo } from '../../../types/modelCatalog';
import { buildLanguagePickerOptions } from '../../../utils/languages';
import {
  buildScenarioLanguagePatch,
  getScenarioLanguage,
  getScenarioModelPath,
  scenarioModelFieldKey,
} from '../../../utils/scenarioModels';
import { Dropdown } from '../../Dropdown';
import { ModelCard } from '../ModelCard';
import { SwitchRow } from './SettingRows';
import {
  companionKind,
  companionType,
  groupAsrModels,
  type LanguageFilter,
  type ModelPageScope,
  matchesLanguageFilter,
  modelForPath,
  modesAreLinked,
  orderPickerGroups,
  recommendedGroupId,
  scopeScenario,
} from './speechAiLogic';

const FILTERS: Array<{ id: LanguageFilter; key: string }> = [
  { id: 'any', key: 'settings.speech_ai.filter_all' },
  { id: 'zh', key: 'settings.speech_ai.filter_chinese' },
  { id: 'en', key: 'settings.speech_ai.filter_english' },
  { id: 'many', key: 'settings.speech_ai.filter_many' },
  { id: 'installed', key: 'settings.speech_ai.filter_downloaded' },
];

export function SpeechModelPage({ scope }: { scope: ModelPageScope }): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const {
    installedModels,
    modelCatalog,
    downloads,
    handleDownload,
    handleDelete,
    handleCancelDownload,
    handleDiscardDownload,
    catalogLoadState,
  } = useModelManagerContext();
  const language = i18n.resolvedLanguage || i18n.language;
  const [filter, setFilter] = useState<LanguageFilter>(
    scope === 'downloaded' ? 'installed' : 'any'
  );
  const scenario = scopeScenario(scope);
  const kind = companionKind(scope);
  const type = companionType(scope);
  const linked = modesAreLinked(config);
  const catalogModels = modelCatalog.models;

  const selectedAsrId = useMemo(() => {
    if (!scenario || kind) return undefined;
    const selection = config.asr?.selections[scenario];
    if (!selection) return undefined;
    if (selection.engine === 'online') {
      return selection.modelId
        ? `${selection.providerId ?? ''}::${selection.modelId}`
        : (selection.providerId ?? undefined);
    }
    return selection.modelId ?? undefined;
  }, [config.asr?.selections, kind, scenario]);

  const selectedCompanionId = useMemo(() => {
    if (!scenario || !kind) return undefined;
    return modelForPath(getScenarioModelPath(config, kind, scenario), catalogModels)?.id;
  }, [catalogModels, config, kind, scenario]);

  const applyAsr = (model: ModelInfo, target: 'live' | 'batch') => {
    const patch = buildModelSelectionPatch(config, target, model.id, modelCatalog);
    const merged = { ...config, ...patch };
    const coerced = asrConfigService.coerceConfiguredLanguage(
      merged,
      target,
      getScenarioLanguage(merged, target)
    );
    const languagePatch = buildScenarioLanguagePatch(target, coerced);
    if (target === 'live' && modesAreLinked(config)) {
      const batchPatch = buildModelSelectionPatch(
        { ...config, ...patch },
        'batch',
        model.id,
        modelCatalog
      );
      updateConfig({ ...patch, ...batchPatch, ...languagePatch, speechModesLinked: true });
      return;
    }
    updateConfig({ ...patch, ...languagePatch });
  };

  const applyModel = (model: ModelInfo) => {
    if (!installedModels.has(model.id)) return;
    if (scenario && kind) {
      const install = installPathFor(model.id, modelCatalog.modelPathById, catalogModels);
      if (!install) return;
      updateConfig({ [scenarioModelFieldKey(kind, scenario)]: install });
      return;
    }
    if (scenario === 'live' || scenario === 'batch') applyAsr(model, scenario);
  };

  const downloadModel = (model: ModelInfo) => {
    const snapshot = modelIdForSelectionSnapshot(config, scope);
    void handleDownload(model).then(() => {
      const completed =
        useModelDownloadStore.getState().downloads[model.id]?.status === 'completed';
      if (!completed) return;
      const current = useConfigStore.getState().config;
      if (modelIdForSelectionSnapshot(current, scope) !== snapshot) return;
      if (scenario && kind) {
        const install = installPathFor(model.id, modelCatalog.modelPathById, catalogModels);
        if (install) updateConfig({ [scenarioModelFieldKey(kind, scenario)]: install });
        return;
      }
      if (scenario === 'live' || scenario === 'batch') applyAsr(model, scenario);
    });
  };

  const setLinked = (next: boolean) => {
    if (!next) {
      updateConfig({ speechModesLinked: false });
      return;
    }
    const live = config.asr?.selections.live;
    const liveId =
      live?.engine === 'online'
        ? live.modelId
          ? `${live.providerId ?? ''}::${live.modelId}`
          : (live.providerId ?? '')
        : (live?.modelId ?? '');
    const patch = liveId ? buildModelSelectionPatch(config, 'batch', liveId, modelCatalog) : {};
    updateConfig({ ...patch, speechModesLinked: true });
  };

  const groups = useMemo(() => {
    if (type) {
      const models = catalogModels.filter((model) => model.type === type);
      return models.length > 0 ? [{ key: type, models }] : [];
    }
    return groupAsrModels(catalogModels);
  }, [catalogModels, type]);

  const visible = useMemo(() => {
    const recommendedKey = recommendedGroupId(language);
    return orderPickerGroups(groups, language, filter, true)
      .map((group) => ({
        ...group,
        models: group.models
          .filter((model) => matchesLanguageFilter(model, filter))
          .filter((model) => filter !== 'installed' || installedModels.has(model.id))
          .map((model, index) => ({
            ...model,
            isRecommended: group.key === recommendedKey && index === 0 && !type,
          })),
      }))
      .filter((group) => group.models.length > 0);
  }, [filter, groups, installedModels, language, type]);

  const helpers = useMemo(() => {
    if (scope !== 'downloaded') return [];
    return catalogModels.filter(
      (model) =>
        (model.type === 'speaker-segmentation' || model.type === 'speaker-embedding') &&
        installedModels.has(model.id)
    );
  }, [catalogModels, installedModels, scope]);

  const showCards = scope !== 'batch' || !linked;
  const showLanguage = scenario === 'live' || (scenario === 'batch' && !linked && !kind);

  const languageValue = scenario ? getScenarioLanguage(config, scenario) : 'auto';
  const languageOptions = useMemo(() => {
    if (!scenario || kind) return [];
    const capability = asrConfigService.resolveActiveLanguageCapability(config, scenario);
    return buildLanguagePickerOptions(capability, language).map((option) =>
      option.group === 'common' ? { ...option, group: t('languages.common') } : option
    );
  }, [config, kind, language, scenario, t]);

  return (
    <div className="speech-ai" id="settings-speech-picker">
      {scope === 'batch' ? (
        <div className="speech-ai-card">
          <SwitchRow
            id="settings-speech-link-toggle"
            title={t('settings.speech_ai.same_as_live')}
            hint={t('settings.speech_ai.same_as_live_hint')}
            checked={linked}
            onChange={setLinked}
          />
        </div>
      ) : null}
      {showLanguage ? (
        <div className="speech-ai-card">
          <div className="settings-item-container layout-horizontal" id="settings-speech-language">
            <div className="settings-item-info">
              <div className="settings-item-title">{t('settings.speech_ai.language')}</div>
              <div className="settings-item-hint">{t('settings.speech_ai.language_hint')}</div>
            </div>
            <div className="settings-item-action speech-ai-control">
              <Dropdown
                id="settings-speech-language-dropdown"
                aria-label={t('settings.speech_ai.language')}
                value={languageValue}
                options={languageOptions}
                onChange={(value) => {
                  if (scenario) updateConfig(buildScenarioLanguagePatch(scenario, value));
                }}
                style={{ width: 190 }}
              />
            </div>
          </div>
        </div>
      ) : null}
      {showCards ? (
        <>
          <div
            className="speech-ai-filters"
            role="tablist"
            aria-label={t('settings.speech_ai.filter_group_aria')}
          >
            {FILTERS.map((item) => (
              <button
                key={item.id}
                type="button"
                role="tab"
                aria-selected={filter === item.id}
                className={`speech-ai-filter${filter === item.id ? ' is-active' : ''}`}
                onClick={() => setFilter(item.id)}
              >
                {t(item.key)}
              </button>
            ))}
            <span className="speech-ai-filter-note">{t('settings.speech_ai.filter_note')}</span>
          </div>
          {kind && scenario ? (
            <button
              type="button"
              className={`speech-ai-clear${selectedCompanionId ? '' : ' is-current'}`}
              onClick={() => updateConfig({ [scenarioModelFieldKey(kind, scenario)]: '' })}
            >
              <span>{t('settings.speech_ai.off')}</span>
              {selectedCompanionId ? null : <span>{t('settings.speech_ai.in_use')}</span>}
            </button>
          ) : null}
          {visible.map((group) => (
            <ModelCard
              key={group.key}
              models={group.models}
              installedModels={installedModels}
              downloads={downloads}
              onDelete={handleDelete}
              onDownload={downloadModel}
              onCancelDownload={handleCancelDownload}
              onDiscardDownload={handleDiscardDownload}
              onSelect={scope === 'downloaded' ? undefined : applyModel}
              selectedModelId={kind ? selectedCompanionId : selectedAsrId}
              actionsDisabled={catalogLoadState !== 'ready'}
            />
          ))}
          {helpers.length > 0 ? (
            <>
              <div className="speech-ai-section-label">{t('settings.speech_ai.helpers_title')}</div>
              {helpers.map((model) => (
                <ModelCard
                  key={model.id}
                  models={[model]}
                  installedModels={installedModels}
                  downloads={downloads}
                  onDelete={handleDelete}
                  onDownload={handleDownload}
                  onCancelDownload={handleCancelDownload}
                  onDiscardDownload={handleDiscardDownload}
                  actionsDisabled={catalogLoadState !== 'ready'}
                />
              ))}
            </>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function installPathFor(
  modelId: string,
  paths: Record<string, string>,
  models: Array<ModelInfo & { installPath?: string }>
): string {
  return paths[modelId] || models.find((item) => item.id === modelId)?.installPath || '';
}

function modelIdForSelectionSnapshot(config: AppConfig, scope: ModelPageScope): string {
  const scenario = scopeScenario(scope);
  if (!scenario) return '';
  return config.asr?.selections[scenario]?.modelId ?? '';
}
