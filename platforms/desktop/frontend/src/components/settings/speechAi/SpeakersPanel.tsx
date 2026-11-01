import { useContext, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { useConfigStore, useSetConfig } from '../../../stores/configStore';
import type { ModelInfo } from '../../../types/modelCatalog';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import { SegmentedSubTabs } from '../SettingsLayout';
import { SettingsNavigationContext } from '../SettingsNavigationContext';
import { SettingDrill } from './CapabilityRow';
import {
  helperModel,
  modesAreLinked,
  RECOMMENDED_EMBEDDING_ID,
  RECOMMENDED_SEGMENTATION_ID,
  type SpeechScope,
  speakersEnabled,
} from './speechAiLogic';

export function SpeakersPanel({
  scope: _scope,
  onChangeHelper,
}: {
  scope: SpeechScope;
  onChangeHelper: (kind: 'segmentation' | 'embedding') => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const navContext = useContext(SettingsNavigationContext);
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const { installedModels, handleDownload, modelCatalog, downloads } = useModelManagerContext();
  const linked = modesAreLinked(config);
  const enabledFor = (target: SpeechScope) =>
    target === 'batch'
      ? speakersEnabled(config, 'batch')
      : target === 'live'
        ? speakersEnabled(config, 'live')
        : speakersEnabled(config, 'live') || speakersEnabled(config, 'batch');
  const [busy, setBusy] = useState(false);
  const sensitivity = config.speakerDiarizationSensitivity ?? 'balanced';
  const segDownloading = Boolean(downloads[RECOMMENDED_SEGMENTATION_ID]);
  const embDownloading = Boolean(downloads[RECOMMENDED_EMBEDDING_ID]);
  const segModel = modelCatalog.models.find((model) => model.id === RECOMMENDED_SEGMENTATION_ID);
  const embModel = modelCatalog.models.find((model) => model.id === RECOMMENDED_EMBEDDING_ID);

  const writePaths = (segmentation: string, embedding: string, target: SpeechScope) => {
    const patch: Record<string, string> = {};
    if (target !== 'batch') {
      patch.liveSpeakerSegmentationModelPath = segmentation;
      patch.liveSpeakerEmbeddingModelPath = embedding;
    }
    if (target !== 'live') {
      patch.batchSpeakerSegmentationModelPath = segmentation;
      patch.batchSpeakerEmbeddingModelPath = embedding;
    }
    updateConfig(patch);
  };

  const enable = async (next: boolean, target: SpeechScope) => {
    if (!next) {
      writePaths('', '', target);
      return;
    }
    const segPath =
      modelCatalog.modelPathById[RECOMMENDED_SEGMENTATION_ID] ||
      (installedModels.has(RECOMMENDED_SEGMENTATION_ID) ? segModel?.installPath : '') ||
      '';
    const embPath =
      modelCatalog.modelPathById[RECOMMENDED_EMBEDDING_ID] ||
      (installedModels.has(RECOMMENDED_EMBEDDING_ID) ? embModel?.installPath : '') ||
      '';
    if (segPath && embPath) {
      writePaths(segPath, embPath, target);
      return;
    }
    setBusy(true);
    try {
      const segPreset = helperModel(RECOMMENDED_SEGMENTATION_ID);
      const embPreset = helperModel(RECOMMENDED_EMBEDDING_ID);
      if (!installedModels.has(RECOMMENDED_SEGMENTATION_ID) && segPreset) {
        await handleDownload(segPreset);
      }
      if (!installedModels.has(RECOMMENDED_EMBEDDING_ID) && embPreset) {
        await handleDownload(embPreset);
      }
      const seg =
        modelCatalog.modelPathById[RECOMMENDED_SEGMENTATION_ID] || segModel?.installPath || '';
      const emb =
        modelCatalog.modelPathById[RECOMMENDED_EMBEDDING_ID] || embModel?.installPath || '';
      writePaths(seg, emb, target);
    } finally {
      setBusy(false);
    }
  };

  const countOptions = [
    { value: 'detect', label: t('settings.speech_ai.detect') },
    ...Array.from({ length: 9 }, (_, index) => {
      const value = String(index + 2);
      return { value, label: value };
    }),
  ];

  return (
    <div className="speech-ai" id="settings-speech-speakers-panel">
      <p className="speech-ai-detail">{t('settings.speech_ai.speakers_intro')}</p>
      {linked ? (
        <SpeakerControls
          target="both"
          busy={busy || segDownloading || embDownloading}
          enabled={enabledFor('both')}
          onToggle={(checked) => void enable(checked, 'both')}
          countValue={
            config.speakerExpectedCount && config.speakerExpectedCount >= 2
              ? String(config.speakerExpectedCount)
              : 'detect'
          }
          countOptions={countOptions}
          onCount={(value) =>
            updateConfig({ speakerExpectedCount: value === 'detect' ? null : Number(value) })
          }
          sensitivity={sensitivity}
          onSensitivity={(value) => updateConfig({ speakerDiarizationSensitivity: value })}
        />
      ) : (
        <>
          <p className="speech-ai-section-label">{t('settings.speech_ai.while_recording')}</p>
          <SpeakerControls
            target="live"
            busy={busy || segDownloading || embDownloading}
            enabled={enabledFor('live')}
            onToggle={(checked) => void enable(checked, 'live')}
            countValue={
              config.speakerExpectedCount && config.speakerExpectedCount >= 2
                ? String(config.speakerExpectedCount)
                : 'detect'
            }
            countOptions={countOptions}
            onCount={(value) =>
              updateConfig({ speakerExpectedCount: value === 'detect' ? null : Number(value) })
            }
            sensitivity={sensitivity}
            onSensitivity={(value) => updateConfig({ speakerDiarizationSensitivity: value })}
          />
          <p className="speech-ai-section-label">{t('settings.speech_ai.when_file')}</p>
          <SpeakerControls
            target="batch"
            busy={busy || segDownloading || embDownloading}
            enabled={enabledFor('batch')}
            onToggle={(checked) => void enable(checked, 'batch')}
            countValue={
              config.speakerExpectedCount && config.speakerExpectedCount >= 2
                ? String(config.speakerExpectedCount)
                : 'detect'
            }
            countOptions={countOptions}
            onCount={(value) =>
              updateConfig({ speakerExpectedCount: value === 'detect' ? null : Number(value) })
            }
            sensitivity={sensitivity}
            onSensitivity={(value) => updateConfig({ speakerDiarizationSensitivity: value })}
          />
        </>
      )}

      <div className="speech-ai-section-label">{t('settings.speech_ai.helper_models')}</div>
      <div className="speech-ai-card">
        <HelperRow
          title={t('settings.speech_ai.find_changes')}
          detail={
            (segModel ?? helperModel(RECOMMENDED_SEGMENTATION_ID))
              ? helperDetail((segModel ?? helperModel(RECOMMENDED_SEGMENTATION_ID)) as ModelInfo)
              : t('settings.speech_ai.helper_seg_fallback')
          }
          installed={installedModels.has(RECOMMENDED_SEGMENTATION_ID)}
          onChange={() => onChangeHelper('segmentation')}
        />
        <HelperRow
          title={t('settings.speech_ai.match_voices')}
          detail={
            (embModel ?? helperModel(RECOMMENDED_EMBEDDING_ID))
              ? helperDetail((embModel ?? helperModel(RECOMMENDED_EMBEDDING_ID)) as ModelInfo)
              : t('settings.speech_ai.helper_emb_fallback')
          }
          installed={installedModels.has(RECOMMENDED_EMBEDDING_ID)}
          onChange={() => onChangeHelper('embedding')}
        />
      </div>
      <p className="speech-ai-note">
        {linked
          ? t('settings.speech_ai.speakers_note')
          : t('settings.speech_ai.speakers_note_split')}
      </p>
      {navContext ? (
        <div style={{ marginTop: 8 }}>
          <button
            type="button"
            className="btn btn-text"
            onClick={() => navContext.navigateToTab('vocabulary')}
          >
            {t('settings.speaker_manage_profiles')} →
          </button>
        </div>
      ) : null}
    </div>
  );
}

function SpeakerControls({
  target,
  busy,
  enabled,
  onToggle,
  countValue,
  countOptions,
  onCount,
  sensitivity,
  onSensitivity,
}: {
  target: SpeechScope;
  busy: boolean;
  enabled: boolean;
  onToggle: (checked: boolean) => void;
  countValue: string;
  countOptions: Array<{ value: string; label: string }>;
  onCount: (value: string) => void;
  sensitivity: 'permissive' | 'balanced' | 'strict';
  onSensitivity: (value: 'permissive' | 'balanced' | 'strict') => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className="speech-ai-card"
      id={target === 'batch' ? undefined : 'settings-speech-speakers'}
    >
      <div className="settings-item-container layout-horizontal">
        <div className="settings-item-info">
          <div className="settings-item-title">{t('settings.speech_ai.tell_speakers')}</div>
          <div className="settings-item-hint">
            {busy
              ? t('settings.speech_ai.helpers_downloading')
              : t('settings.speech_ai.tell_speakers_hint')}
          </div>
        </div>
        <div className="settings-item-action">
          <Switch
            checked={enabled || busy}
            disabled={busy}
            onChange={onToggle}
            aria-label={t('settings.speech_ai.tell_speakers')}
          />
        </div>
      </div>
      <div className="settings-item-container layout-horizontal">
        <div className="settings-item-info">
          <div className="settings-item-title">{t('settings.speech_ai.number_of_speakers')}</div>
          <div className="settings-item-hint">{t('settings.speech_ai.number_hint')}</div>
        </div>
        <div className="settings-item-action" style={{ width: 160 }}>
          <Dropdown
            aria-label={t('settings.speech_ai.number_of_speakers')}
            value={countValue}
            disabled={!enabled}
            onChange={onCount}
            options={countOptions}
          />
        </div>
      </div>
      <div className="settings-item-container layout-horizontal">
        <div className="settings-item-info">
          <div className="settings-item-title">{t('settings.speech_ai.how_strict')}</div>
          <div className="settings-item-hint">{t('settings.speech_ai.how_strict_hint')}</div>
        </div>
        <div className="settings-item-action">
          <SegmentedSubTabs<'permissive' | 'balanced' | 'strict'>
            role="radiogroup"
            size="sm"
            ariaLabel={t('settings.speech_ai.how_strict')}
            activeTab={sensitivity}
            disabled={!enabled}
            onChange={onSensitivity}
            items={[
              {
                value: 'permissive',
                label: t('settings.speech_ai.permissive'),
              },
              {
                value: 'balanced',
                label: t('settings.speech_ai.balanced'),
              },
              {
                value: 'strict',
                label: t('settings.speech_ai.strict'),
              },
            ]}
          />
        </div>
      </div>
    </div>
  );
}

function helperDetail(model: ModelInfo): string {
  const size = (model.size ?? '').replace(/^~\s*/, '');
  return `${model.name}${size ? ` · ${size}` : ''}`;
}

function HelperRow({
  title,
  detail,
  installed,
  onChange,
}: {
  title: string;
  detail: string;
  installed: boolean;
  onChange: () => void;
}) {
  const { t } = useTranslation();
  return (
    <SettingDrill
      title={title}
      hint={detail}
      value={installed ? t('settings.speech_ai.installed') : t('settings.speech_ai.not_installed')}
      onClick={onChange}
    />
  );
}
