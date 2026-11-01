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
} from './speechAiLogic';

const FILTERS: Array<{ id: LanguageFilter; key: string }> = [
  { id: 'any', key: 'settings.speech_ai.filter_any' },
  { id: 'zh', key: 'settings.speech_ai.filter_chinese' },
  { id: 'en', key: 'settings.speech_ai.filter_english' },
  { id: 'many', key: 'settings.speech_ai.filter_many' },
];

export function SpeechModelPicker({
  groups,
  scope,
  language,
  installedModels,
  downloads,
  actionsDisabled,
  onDownload,
  onDelete,
  onCancelDownload,
  onDiscardDownload,
}: {
  groups: PickerGroup[];
  scope: SpeechScope;
  language: string | undefined;
  installedModels: Set<string>;
  downloads: Record<string, { progress: number; status: string }>;
  actionsDisabled?: boolean;
  onDownload: (model: ModelInfo) => void;
  onDelete: (model: ModelInfo) => void;
  onCancelDownload: (modelId: string) => void;
  onDiscardDownload?: (model: ModelInfo) => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<LanguageFilter>('any');
  const [showAll, setShowAll] = useState(false);
  const recommendedKey = recommendedGroupId(language);
  const visible = useMemo(() => {
    const ordered = orderPickerGroups(groups, language, filter, showAll).map((group) => ({
      ...group,
      models: group.models.map((model, index) => ({
        ...model,
        isRecommended: group.key === recommendedKey && index === 0,
      })),
    }));
    if (ordered[0]?.key === recommendedKey) return ordered;
    return ordered;
  }, [filter, groups, language, recommendedKey, showAll]);
  const hiddenCount = useMemo(
    () =>
      orderPickerGroups(groups, language, filter, true).length -
      orderPickerGroups(groups, language, filter, false).length,
    [filter, groups, language]
  );
  const scopeLabel =
    scope === 'live' ? t('settings.speech_ai.while_recording') : t('settings.speech_ai.when_file');

  return (
    <div className="speech-ai" id="settings-speech-picker">
      <p className="settings-page-description">
        {scope === 'both'
          ? t('settings.speech_ai.picker_description')
          : t('settings.speech_ai.picker_description_scoped', { mode: scopeLabel })}
      </p>
      {scope !== 'both' ? (
        <span className="speech-ai-pill">
          {t('settings.speech_ai.picker_applying', { mode: scopeLabel })}
        </span>
      ) : null}
      <div
        className="speech-ai-filters"
        role="tablist"
        aria-label={t('settings.speech_ai.filter_any')}
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
      </div>
      {visible.map((group) => (
        <ModelCard
          key={group.key}
          models={group.models}
          installedModels={installedModels}
          downloads={downloads}
          onDelete={onDelete}
          onDownload={onDownload}
          onCancelDownload={onCancelDownload}
          onDiscardDownload={onDiscardDownload}
          actionsDisabled={actionsDisabled}
        />
      ))}
      {hiddenCount > 0 ? (
        <button
          type="button"
          className="btn btn-text speech-ai-show-all"
          onClick={() => setShowAll((value) => !value)}
        >
          {showAll ? t('settings.speech_ai.show_less') : t('settings.speech_ai.show_all')}
        </button>
      ) : null}
    </div>
  );
}
