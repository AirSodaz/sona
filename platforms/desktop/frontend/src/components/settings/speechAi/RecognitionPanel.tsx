import { FileText, Mic } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { buildModelSelectionPatch } from '../../../services/asrConfigService';
import { useConfigStore, useSetConfig, useTranscriptionConfig } from '../../../stores/configStore';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import { SettingDrill } from './CapabilityRow';
import {
  modesAreLinked,
  resolveModelTitle,
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
  const { modelCatalog } = useModelManagerContext();
  const linked = modesAreLinked(config);
  const count = speakerCountLabel(config.speakerExpectedCount, t);

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
        modelLabel={resolveModelTitle(config, 'live', modelCatalog.models, t)}
        speakers={
          speakersEnabled(config, 'live')
            ? t('settings.speech_ai.speakers_on', {
                number: count,
              })
            : t('settings.speech_ai.off')
        }
        onChangeModel={() => onChangeModel('live')}
        onSpeakers={() => onConfigureSpeakers(linked ? 'both' : 'live')}
      />

      {linked ? (
        <p className="speech-ai-note">{t('settings.speech_ai.files_same_line')}</p>
      ) : (
        <ModeCard
          icon={<FileText size={16} />}
          title={t('settings.speech_ai.when_file')}
          hint={t('settings.speech_ai.when_file_hint')}
          modelLabel={resolveModelTitle(config, 'batch', modelCatalog.models, t)}
          speakers={
            speakersEnabled(config, 'batch')
              ? t('settings.speech_ai.speakers_on', {
                  number: count,
                })
              : t('settings.speech_ai.off')
          }
          onChangeModel={() => onChangeModel('batch')}
          onSpeakers={() => onConfigureSpeakers('batch')}
        />
      )}

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
  modelLabel,
  speakers,
  onChangeModel,
  onSpeakers,
}: {
  icon: React.ReactNode;
  title: string;
  hint: string;
  modelLabel: string;
  speakers: string;
  onChangeModel: () => void;
  onSpeakers: () => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="speech-ai-mode-card">
      <div className="speech-ai-mode-head">
        {icon}
        <div>
          <h3>{title}</h3>
          <p>{hint}</p>
        </div>
      </div>
      <SettingDrill
        title={t('settings.speech_ai.speech_model')}
        value={modelLabel || t('settings.speech_ai.not_set_up')}
        onClick={onChangeModel}
      />
      <SettingDrill
        title={t('settings.speech_ai.speakers')}
        value={speakers}
        onClick={onSpeakers}
      />
    </section>
  );
}
