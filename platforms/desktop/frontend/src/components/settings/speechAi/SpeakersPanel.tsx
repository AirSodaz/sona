import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../../hooks/useModelManager';
import { useConfigStore, useSetConfig } from '../../../stores/configStore';
import type { ModelInfo } from '../../../types/modelCatalog';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import { SegmentedSubTabs } from '../SettingsLayout';
import {
  helperModel,
  modesAreLinked,
  RECOMMENDED_EMBEDDING_ID,
  RECOMMENDED_SEGMENTATION_ID,
  type SpeechScope,
  speakersEnabled,
} from './speechAiLogic';

export function SpeakersPanel({
  scope,
  onChangeHelper,
}: {
  scope: SpeechScope;
  onChangeHelper: (kind: 'segmentation' | 'embedding') => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useSetConfig();
  const { installedModels, handleDownload, modelCatalog, downloads } = useModelManagerContext();
  const linked = modesAreLinked(config) || scope === 'both';
  const activeScope: 'live' | 'batch' = scope === 'batch' ? 'batch' : 'live';
  const enabled = linked
    ? speakersEnabled(config, 'live') || speakersEnabled(config, 'batch')
    : speakersEnabled(config, activeScope);
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

  const enable = async (next: boolean) => {
    const target: SpeechScope = linked ? 'both' : activeScope;
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
    const previous = {
      liveSpeakerSegmentationModelPath: config.liveSpeakerSegmentationModelPath ?? '',
      liveSpeakerEmbeddingModelPath: config.liveSpeakerEmbeddingModelPath ?? '',
      batchSpeakerSegmentationModelPath: config.batchSpeakerSegmentationModelPath ?? '',
      batchSpeakerEmbeddingModelPath: config.batchSpeakerEmbeddingModelPath ?? '',
    };
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
      if (target === 'live') {
        updateConfig({
          batchSpeakerSegmentationModelPath: previous.batchSpeakerSegmentationModelPath,
          batchSpeakerEmbeddingModelPath: previous.batchSpeakerEmbeddingModelPath,
        });
      } else if (target === 'batch') {
        updateConfig({
          liveSpeakerSegmentationModelPath: previous.liveSpeakerSegmentationModelPath,
          liveSpeakerEmbeddingModelPath: previous.liveSpeakerEmbeddingModelPath,
        });
      }
    } finally {
      setBusy(false);
    }
  };

  const countOptions = [
    { value: 'detect', label: t('settings.speech_ai.detect', { defaultValue: 'Detect' }) },
    ...Array.from({ length: 9 }, (_, index) => {
      const value = String(index + 2);
      return { value, label: value };
    }),
  ];

  return (
    <div className="speech-ai" id="settings-speech-speakers-panel">
      <p className="speech-ai-detail">
        {t('settings.speech_ai.speakers_intro', {
          defaultValue:
            'Label who said what. Helper models download automatically the first time you turn this on.',
        })}
      </p>
      <div className="speech-ai-card">
        <div className="settings-item-container layout-horizontal" id="settings-speech-speakers">
          <div className="settings-item-info">
            <div className="settings-item-title">
              {t('settings.speech_ai.tell_speakers', { defaultValue: 'Tell speakers apart' })}
            </div>
            <div className="settings-item-hint">
              {busy || segDownloading || embDownloading
                ? t('settings.speech_ai.helpers_downloading', {
                    defaultValue: 'Downloading two small helpers (~34 MB)…',
                  })
                : t('settings.speech_ai.tell_speakers_hint', {
                    defaultValue: 'Downloads two small helpers the first time (~34 MB).',
                  })}
            </div>
          </div>
          <div className="settings-item-action">
            <Switch
              checked={enabled || busy}
              disabled={busy}
              onChange={(checked) => void enable(checked)}
              aria-label={t('settings.speech_ai.tell_speakers', {
                defaultValue: 'Tell speakers apart',
              })}
            />
          </div>
        </div>
        <div className="settings-item-container layout-horizontal">
          <div className="settings-item-info">
            <div className="settings-item-title">
              {t('settings.speech_ai.number_of_speakers', { defaultValue: 'Number of speakers' })}
            </div>
            <div className="settings-item-hint">
              {t('settings.speech_ai.number_hint', {
                defaultValue: "Leave on Detect if you're not sure.",
              })}
            </div>
          </div>
          <div className="settings-item-action" style={{ width: 160 }}>
            <Dropdown
              aria-label={t('settings.speech_ai.number_of_speakers', {
                defaultValue: 'Number of speakers',
              })}
              value={
                config.speakerExpectedCount && config.speakerExpectedCount >= 2
                  ? String(config.speakerExpectedCount)
                  : 'detect'
              }
              disabled={!enabled}
              onChange={(value) =>
                updateConfig({ speakerExpectedCount: value === 'detect' ? null : Number(value) })
              }
              options={countOptions}
            />
          </div>
        </div>
        <div className="settings-item-container layout-horizontal">
          <div className="settings-item-info">
            <div className="settings-item-title">
              {t('settings.speech_ai.how_strict', { defaultValue: 'How strict' })}
            </div>
            <div className="settings-item-hint">
              {t('settings.speech_ai.how_strict_hint', {
                defaultValue: 'Permissive merges similar voices; Strict keeps them apart.',
              })}
            </div>
          </div>
          <div className="settings-item-action">
            <SegmentedSubTabs<'permissive' | 'balanced' | 'strict'>
              role="radiogroup"
              size="sm"
              ariaLabel={t('settings.speech_ai.how_strict', { defaultValue: 'How strict' })}
              activeTab={sensitivity}
              disabled={!enabled}
              onChange={(value) => updateConfig({ speakerDiarizationSensitivity: value })}
              items={[
                {
                  value: 'permissive',
                  label: t('settings.speech_ai.permissive', { defaultValue: 'Permissive' }),
                },
                {
                  value: 'balanced',
                  label: t('settings.speech_ai.balanced', { defaultValue: 'Balanced' }),
                },
                {
                  value: 'strict',
                  label: t('settings.speech_ai.strict', { defaultValue: 'Strict' }),
                },
              ]}
            />
          </div>
        </div>
      </div>

      <div className="speech-ai-section-label">
        {t('settings.speech_ai.helper_models', { defaultValue: 'Helper models' })}
      </div>
      <div className="speech-ai-card">
        <HelperRow
          title={t('settings.speech_ai.find_changes', { defaultValue: 'Find speaker changes' })}
          detail={helperDetail(
            segModel ?? helperModel(RECOMMENDED_SEGMENTATION_ID),
            'Pyannote 3.0 · 6 MB · used mainly for files'
          )}
          installed={installedModels.has(RECOMMENDED_SEGMENTATION_ID)}
          onChange={() => onChangeHelper('segmentation')}
        />
        <HelperRow
          title={t('settings.speech_ai.match_voices', { defaultValue: 'Match voices' })}
          detail={helperDetail(
            embModel ?? helperModel(RECOMMENDED_EMBEDDING_ID),
            '3D-Speaker CAMPPlus · 28 MB · matches speakers to your voice library'
          )}
          installed={installedModels.has(RECOMMENDED_EMBEDDING_ID)}
          onChange={() => onChangeHelper('embedding')}
        />
      </div>
      <p className="speech-ai-note">
        {t('settings.speech_ai.speakers_note', {
          defaultValue:
            'Recording and files currently share these speaker settings. To set them separately, open Recognition and turn off “Keep recording and files the same.”',
        })}
      </p>
    </div>
  );
}

function helperDetail(model: ModelInfo | undefined, fallback: string): string {
  if (!model) return fallback;
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
    <div className="settings-item-container layout-horizontal">
      <div className="settings-item-info">
        <div className="settings-item-title">{title}</div>
        <div className="settings-item-hint">{detail}</div>
      </div>
      <div
        className="settings-item-action"
        style={{ display: 'flex', gap: 8, alignItems: 'center' }}
      >
        <span className={`speech-ai-status ${installed ? 'is-ok' : 'is-idle'}`}>
          {installed
            ? t('settings.speech_ai.installed', { defaultValue: 'Installed' })
            : t('settings.speech_ai.not_installed', { defaultValue: 'Not installed' })}
        </span>
        <button type="button" className="btn btn-text" onClick={onChange}>
          {t('settings.speech_ai.change', { defaultValue: 'Change' })}
        </button>
      </div>
    </div>
  );
}
