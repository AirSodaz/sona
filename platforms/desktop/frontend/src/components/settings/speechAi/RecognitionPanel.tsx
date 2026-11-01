import { FileText, Mic } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { buildModelSelectionPatch } from '../../../services/asrConfigService';
import { useConfigStore, useSetConfig, useTranscriptionConfig } from '../../../stores/configStore';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import {
  modelIdForSelection,
  modesAreLinked,
  resolveModelTitle,
  selectionForScope,
  speakerCountLabel,
  speakersEnabled,
} from './speechAiLogic';

export function RecognitionPanel({
  onChangeModel,
  onConfigureSpeakers,
}: {
  onChangeModel: (scope: 'live' | 'batch') => void;
  onConfigureSpeakers: (scope: 'live' | 'batch' | 'both') => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const transcription = useTranscriptionConfig();
  const { modelCatalog, catalogLoadState } = useModelManagerContext();
  const linked = modesAreLinked(config);
  const catalogReady = catalogLoadState === 'ready';
  const count = speakerCountLabel(config.speakerExpectedCount, t);

  const applyInstalled = (scope: 'live' | 'batch', modelId: string) => {
    const patch = buildModelSelectionPatch(config, scope, modelId, modelCatalog);
    if (linked && scope === 'live') {
      const batchPatch = buildModelSelectionPatch(
        { ...config, ...patch },
        'batch',
        modelId,
        modelCatalog
      );
      updateConfig({ ...patch, ...batchPatch, speechModesLinked: true });
      return;
    }
    updateConfig(patch);
  };

  const setLinked = (next: boolean) => {
    if (!next) {
      updateConfig({ speechModesLinked: false });
      return;
    }
    const liveId = config.asr?.selections.live?.modelId;
    const livePatch =
      liveId && config.asr?.selections.live?.engine !== 'online'
        ? buildModelSelectionPatch(config, 'batch', liveId, modelCatalog)
        : {};
    updateConfig({
      ...livePatch,
      speechModesLinked: true,
      batchSpeakerEmbeddingModelPath: config.liveSpeakerEmbeddingModelPath ?? '',
      batchSpeakerSegmentationModelPath: config.liveSpeakerSegmentationModelPath ?? '',
    });
  };

  const punctuationValue =
    (config.livePunctuationModelPath &&
      modelCatalog.models.find((model) => model.installPath === config.livePunctuationModelPath)
        ?.id) ||
    '';
  const punctuationOptions = [
    { value: '', label: t('settings.speech_ai.automatic') },
    ...modelCatalog.models
      .filter((model) => model.type === 'punctuation')
      .map((model) => ({ value: model.id, label: model.name })),
  ];

  const setPunctuation = (modelId: string) => {
    if (!modelId) {
      updateConfig({ livePunctuationModelPath: '', batchPunctuationModelPath: '' });
      return;
    }
    const path = modelCatalog.modelPathById[modelId] ?? '';
    if (!path) return;
    updateConfig({ livePunctuationModelPath: path, batchPunctuationModelPath: path });
  };

  const installedAsr = (slot: 'streaming' | 'batch') =>
    (modelCatalog.selectionOptions[slot] ?? []).filter((option) => option.isInstalled);

  return (
    <div className="speech-ai" id="settings-speech-recognition">
      <p className="speech-ai-detail">
        {linked
          ? t('settings.speech_ai.recognition_same_intro')
          : t('settings.speech_ai.recognition_split_intro')}
      </p>
      <div className="settings-item-container layout-horizontal" id="settings-speech-link-toggle">
        <div className="settings-item-info">
          <div className="settings-item-title">{t('settings.speech_ai.keep_same')}</div>
          <div className="settings-item-hint">
            {linked
              ? t('settings.speech_ai.keep_same_on')
              : t('settings.speech_ai.currently_different')}
          </div>
        </div>
        <div className="settings-item-action">
          <Switch
            checked={linked}
            onChange={setLinked}
            aria-label={t('settings.speech_ai.keep_same')}
          />
        </div>
      </div>

      <ModeCard
        icon={<Mic size={16} />}
        title={t('settings.speech_ai.while_recording')}
        hint={t('settings.speech_ai.while_recording_hint')}
        modelId={modelIdForSelection(selectionForScope(config, 'live')) ?? ''}
        modelLabel={resolveModelTitle(config, 'live', modelCatalog.models, t)}
        options={installedAsr('streaming')}
        disabled={!catalogReady}
        speakers={
          speakersEnabled(config, 'live')
            ? t('settings.speech_ai.speakers_on', {
                number: count,
              })
            : t('settings.speech_ai.off')
        }
        onModel={(modelId) => applyInstalled('live', modelId)}
        onChangeModel={() => onChangeModel('live')}
        onSpeakers={() => onConfigureSpeakers(linked ? 'both' : 'live')}
        readOnly={false}
      />

      <ModeCard
        icon={<FileText size={16} />}
        title={t('settings.speech_ai.when_file')}
        hint={t('settings.speech_ai.when_file_hint')}
        modelId={linked ? '' : (modelIdForSelection(selectionForScope(config, 'batch')) ?? '')}
        modelLabel={
          linked
            ? t('settings.speech_ai.same_as_recording')
            : resolveModelTitle(config, 'batch', modelCatalog.models, t)
        }
        options={installedAsr('batch')}
        disabled={!catalogReady || linked}
        speakers={
          linked
            ? t('settings.speech_ai.same_as_recording')
            : speakersEnabled(config, 'batch')
              ? t('settings.speech_ai.speakers_on', {
                  number: count,
                })
              : t('settings.speech_ai.off')
        }
        onModel={(modelId) => applyInstalled('batch', modelId)}
        onChangeModel={() => onChangeModel('batch')}
        onSpeakers={() => onConfigureSpeakers(linked ? 'both' : 'batch')}
        readOnly={linked}
      />

      <div className="speech-ai-section-label">{t('settings.speech_ai.shared_helpers')}</div>
      <div className="speech-ai-card">
        <div className="settings-item-container layout-horizontal" id="settings-punctuation-path">
          <div className="settings-item-info">
            <div className="settings-item-title">{t('settings.speech_ai.add_punctuation')}</div>
            <div className="settings-item-hint">{t('settings.speech_ai.add_punctuation_hint')}</div>
          </div>
          <div className="settings-item-action" style={{ width: 180 }}>
            <Dropdown
              id="settings-punctuation-dropdown"
              aria-label={t('settings.speech_ai.add_punctuation')}
              value={punctuationValue}
              onChange={setPunctuation}
              options={punctuationOptions}
            />
          </div>
        </div>
        <div className="settings-item-container layout-horizontal" id="settings-vad-path">
          <div className="settings-item-info">
            <div className="settings-item-title">{t('settings.speech_ai.detect_pauses')}</div>
            <div className="settings-item-hint">{t('settings.speech_ai.detect_pauses_hint')}</div>
          </div>
          <div className="settings-item-action" style={{ width: 180 }} id="settings-batch-vad">
            <Dropdown
              id="settings-pause-dropdown"
              aria-label={t('settings.speech_ai.detect_pauses')}
              value={transcription.batchVadEnabled === false ? 'off' : 'automatic'}
              onChange={(value) => updateConfig({ batchVadEnabled: value !== 'off' })}
              options={[
                {
                  value: 'automatic',
                  label: t('settings.speech_ai.automatic'),
                },
                { value: 'off', label: t('settings.speech_ai.off') },
              ]}
            />
          </div>
        </div>
        <div className="settings-item-container layout-horizontal">
          <div className="settings-item-info">
            <div className="settings-item-title">{t('settings.speech_ai.low_power')}</div>
            <div className="settings-item-hint">{t('settings.speech_ai.low_power_hint')}</div>
          </div>
          <div className="settings-item-action">
            <Switch
              checked={Boolean(transcription.liveSentenceOnly)}
              onChange={(checked) => updateConfig({ liveSentenceOnly: checked })}
              aria-label={t('settings.speech_ai.low_power')}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function ModeCard({
  icon,
  title,
  hint,
  modelId,
  modelLabel,
  options,
  disabled,
  speakers,
  onModel,
  onChangeModel,
  onSpeakers,
  readOnly,
}: {
  icon: React.ReactNode;
  title: string;
  hint: string;
  modelId: string;
  modelLabel: string;
  options: Array<{ id: string; label: string }>;
  disabled: boolean;
  speakers: string;
  onModel: (modelId: string) => void;
  onChangeModel: () => void;
  onSpeakers: () => void;
  readOnly: boolean;
}) {
  const { t } = useTranslation();
  const current = options.some((option) => option.id === modelId) ? modelId : '';
  return (
    <section className="speech-ai-mode-card">
      <div className="speech-ai-mode-head">
        {icon}
        <div>
          <h3>{title}</h3>
          <p>{hint}</p>
        </div>
      </div>
      <div className="settings-item-container layout-horizontal">
        <div className="settings-item-info">
          <div className="settings-item-title">{t('settings.speech_ai.speech_model')}</div>
          <div className="settings-item-hint">
            {readOnly ? modelLabel : t('settings.speech_ai.used_for_activity')}
          </div>
        </div>
        <div
          className="settings-item-action"
          style={{ display: 'flex', gap: 8, alignItems: 'center' }}
        >
          {readOnly ? (
            <span className="speech-ai-detail">{modelLabel}</span>
          ) : (
            <>
              <div style={{ width: 190 }}>
                <Dropdown
                  aria-label={t('settings.speech_ai.speech_model')}
                  value={current}
                  disabled={disabled || options.length === 0}
                  placeholder={modelLabel || t('settings.speech_ai.not_set_up')}
                  onChange={onModel}
                  options={options.map((option) => ({ value: option.id, label: option.label }))}
                />
              </div>
              <button type="button" className="btn btn-text" onClick={onChangeModel}>
                {t('settings.speech_ai.change')}
              </button>
            </>
          )}
        </div>
      </div>
      <div className="settings-item-container layout-horizontal">
        <div className="settings-item-info">
          <div className="settings-item-title">{t('settings.speech_ai.speakers')}</div>
          <div className="settings-item-hint">{speakers}</div>
        </div>
        <div className="settings-item-action">
          <button type="button" className="btn btn-text" onClick={onSpeakers} disabled={readOnly}>
            {readOnly ? speakers : t('settings.speech_ai.configure')}
            {!readOnly ? ' ›' : ''}
          </button>
        </div>
      </div>
    </section>
  );
}
