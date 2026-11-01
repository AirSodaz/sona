import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ModelInfo } from '../../../types/modelCatalog';
import { ModelCard } from '../ModelCard';
import {
  type LanguageFilter,
  orderPickerGroups,
  type PickerGroup,
  recommendedGroupId,
  type SpeechScope,
  versionRowLabels,
} from './speechAiLogic';

const FILTERS: Array<{ id: LanguageFilter; key: string; fallback: string }> = [
  { id: 'any', key: 'settings.speech_ai.filter_any', fallback: 'Any language' },
  { id: 'zh', key: 'settings.speech_ai.filter_chinese', fallback: 'Chinese' },
  { id: 'en', key: 'settings.speech_ai.filter_english', fallback: 'English' },
  { id: 'many', key: 'settings.speech_ai.filter_many', fallback: 'Many languages' },
];

export function SpeechModelPicker({
  groups,
  scope,
  language,
  installedModels,
  downloads,
  activeModelId,
  actionsDisabled,
  onDownload,
  onCancelDownload,
  onUse,
}: {
  groups: PickerGroup[];
  scope: SpeechScope;
  language: string | undefined;
  installedModels: Set<string>;
  downloads: Record<string, { progress: number; status: string }>;
  activeModelId: string | null;
  actionsDisabled?: boolean;
  onDownload: (model: ModelInfo) => void;
  onCancelDownload: (modelId: string) => void;
  onUse: (model: ModelInfo) => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<LanguageFilter>('any');
  const [showAll, setShowAll] = useState(false);
  const recommendedKey = recommendedGroupId(language);
  const visible = useMemo(
    () => orderPickerGroups(groups, language, filter, showAll),
    [filter, groups, language, showAll]
  );
  const hiddenCount = useMemo(
    () =>
      orderPickerGroups(groups, language, filter, true).length -
      orderPickerGroups(groups, language, filter, false).length,
    [filter, groups, language]
  );
  const scopeLabel =
    scope === 'live'
      ? t('settings.speech_ai.while_recording', { defaultValue: 'While recording' })
      : t('settings.speech_ai.when_file', { defaultValue: 'When opening a file' });

  return (
    <div className="speech-ai" id="settings-speech-picker">
      <p className="settings-page-description">
        {scope === 'both'
          ? t('settings.speech_ai.picker_description', {
              defaultValue:
                'All models run on this computer. Selecting one sets it for recording and files.',
            })
          : t('settings.speech_ai.picker_description_scoped', {
              mode: scopeLabel,
              defaultValue: `All models run on this computer. This choice applies only to ${scopeLabel}.`,
            })}
      </p>
      {scope !== 'both' ? (
        <span className="speech-ai-pill">
          {t('settings.speech_ai.picker_applying', {
            mode: scopeLabel,
            defaultValue: `Applying to: ${scopeLabel}`,
          })}
        </span>
      ) : null}
      <div
        className="speech-ai-filters"
        role="tablist"
        aria-label={t('settings.speech_ai.filter_any', { defaultValue: 'Any language' })}
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
            {t(item.key, { defaultValue: item.fallback })}
          </button>
        ))}
      </div>
      {visible.map((group) => {
        const base = group.models[0];
        const curated = group.curated;
        const isRecommended = group.key === recommendedKey;
        const primaryId = isRecommended ? (curated?.defaultVariantId ?? base.id) : null;
        const rowLabels = new Map(versionRowLabels(group.models, t).map((row) => [row.id, row]));
        return (
          <ModelCard
            key={group.key}
            models={group.models}
            installedModels={installedModels}
            downloads={downloads}
            onDelete={() => undefined}
            onDownload={onDownload}
            onCancelDownload={onCancelDownload}
            actionsDisabled={actionsDisabled}
            displayName={
              curated ? t(curated.titleKey, { defaultValue: curated.titleDefault }) : base.name
            }
            description={
              curated ? t(curated.blurbKey, { defaultValue: curated.blurbDefault }) : undefined
            }
            recommended={isRecommended}
            actionVariant="labeled"
            onUse={onUse}
            activeModelId={activeModelId}
            primaryModelId={primaryId}
            versionMeta={(model) => {
              const row = rowLabels.get(model.id);
              return {
                title: row?.title ?? model.versionLabel ?? model.name,
                hint: row?.hint,
              };
            }}
          />
        );
      })}
      {hiddenCount > 0 ? (
        <button
          type="button"
          className="btn btn-text speech-ai-show-all"
          onClick={() => setShowAll((value) => !value)}
        >
          {showAll
            ? t('settings.speech_ai.show_less', { defaultValue: 'Show recommended models' })
            : t('settings.speech_ai.show_all', { defaultValue: 'Show all models' })}
        </button>
      ) : null}
    </div>
  );
}
