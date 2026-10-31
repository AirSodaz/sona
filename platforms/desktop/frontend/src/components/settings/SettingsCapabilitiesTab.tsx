import {
  ChevronRight,
  Globe,
  Loader2,
  Mic,
  Settings2,
  Sparkles,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelManagerContext } from '../../hooks/useModelManager';
import {
  downloadRecommendedOnboardingModels,
  getRecommendedOnboardingConfig,
  RECOMMENDED_RECOGNITION_MODEL_ID,
} from '../../services/onboardingService';
import { getFeatureModelEntry } from '../../services/llm/state';
import { getProviderDefinition } from '../../services/llm/providers';
import {
  useConfigStore,
  useLlmAssistantConfig,
  useModelConfig,
  useSetConfig,
} from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import type { LlmProvider } from '../../types/transcript';
import { ModelIcon, RobotIcon } from '../Icons';
import { AiWritingConnectModal } from './capabilities/AiWritingConnectModal';
import {
  fingerprintApiKey,
  getProviderProbeCacheKey,
  isAiWritingConfigured,
  isSpeechRecognitionReady,
  isTranslationConfigured,
  readProviderProbeCache,
  type CapabilityStatusKind,
} from './capabilities/capabilityStatus';
import { ModelPickerModal } from './capabilities/ModelPickerModal';
import { SettingsLLMServiceTab } from './SettingsLLMServiceTab';
import {
  SettingsAccordion,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';
import { SettingsModelsTab } from './SettingsModelsTab';
import { getCurrentLlmSettings } from './llm/helpers';
import { useOptionalSettingsNavigation } from './SettingsNavigationContext';
import './SettingsCapabilitiesTab.css';

export type CapabilitiesFocus =
  | 'speech'
  | 'ai_writing'
  | 'translation'
  | 'advanced_speech'
  | 'advanced_ai'
  | 'live'
  | 'batch';

export interface SettingsCapabilitiesTabProps {
  isActive?: boolean;
  initialFocus?: CapabilitiesFocus;
  focusRequestId?: number;
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
}

function StatusBadge({
  kind,
  label,
}: {
  kind: CapabilityStatusKind;
  label: string;
}): React.JSX.Element {
  return <span className={`capability-status capability-status--${kind}`}>{label}</span>;
}

export function SettingsCapabilitiesTab({
  isActive = true,
  initialFocus,
  focusRequestId,
  onOpenLlmProviderDetails,
}: SettingsCapabilitiesTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const llmConfig = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const fullConfig = useConfigStore((state) => state.config);
  const showError = useDialogStore((state) => state.showError);
  const nav = useOptionalSettingsNavigation();
  const { selectedModelIds, downloads } = useModelManagerContext();

  const [installingRecommended, setInstallingRecommended] = useState(false);
  const [installProgress, setInstallProgress] = useState<string | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [connectOpen, setConnectOpen] = useState(false);
  const [advancedSpeechOpen, setAdvancedSpeechOpen] = useState(false);
  const [advancedAiOpen, setAdvancedAiOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  useEffect(() => {
    if (!initialFocus || focusRequestId === undefined) {
      return;
    }
    if (initialFocus === 'advanced_speech' || initialFocus === 'live' || initialFocus === 'batch') {
      setAdvancedOpen(true);
      setAdvancedSpeechOpen(true);
      setAdvancedAiOpen(false);
      return;
    }
    if (initialFocus === 'advanced_ai') {
      setAdvancedOpen(true);
      setAdvancedAiOpen(true);
      setAdvancedSpeechOpen(false);
      return;
    }
    if (initialFocus === 'ai_writing') {
      document.getElementById('settings-capability-ai-writing')?.scrollIntoView({
        block: 'center',
      });
      return;
    }
    if (initialFocus === 'translation') {
      document.getElementById('settings-capability-translation')?.scrollIntoView({
        block: 'center',
      });
      return;
    }
    document.getElementById('settings-capability-speech')?.scrollIntoView({ block: 'center' });
  }, [focusRequestId, initialFocus]);

  const speechReady = isSpeechRecognitionReady(modelConfig);
  const speechModelLabel = useMemo(() => {
    const streamingId = selectedModelIds.streaming;
    if (streamingId) {
      return streamingId;
    }
    const livePath = modelConfig.streamingModelPath || modelConfig.asr?.selections?.live?.modelPath;
    if (!livePath) {
      return null;
    }
    const parts = livePath.split(/[/\\]/);
    return parts[parts.length - 1] || livePath;
  }, [modelConfig.asr?.selections?.live?.modelPath, modelConfig.streamingModelPath, selectedModelIds.streaming]);

  const recommendedDownloading = Boolean(
    downloads[RECOMMENDED_RECOGNITION_MODEL_ID] || installingRecommended
  );

  const speechStatus: { kind: CapabilityStatusKind; label: string } = recommendedDownloading
    ? {
        kind: 'downloading',
        label: t('settings.capabilities.status_downloading', { defaultValue: 'Downloading…' }),
      }
    : speechReady
      ? {
          kind: 'ready',
          label: speechModelLabel
            ? t('settings.capabilities.status_ready_named', {
                defaultValue: 'Ready · {{name}}',
                name: speechModelLabel,
              })
            : t('settings.capabilities.status_ready_plain', { defaultValue: 'Ready' }),
        }
      : {
          kind: 'needs_setup',
          label: t('settings.capabilities.status_needs_setup', {
            defaultValue: 'Needs setup',
          }),
        };

  const polishEntry = getFeatureModelEntry(llmConfig, 'polish');
  const llmSettings = getCurrentLlmSettings(llmConfig);
  const polishProvider = polishEntry?.provider;
  const polishProviderSetting = polishProvider
    ? llmSettings.providers[polishProvider]
    : undefined;
  const probeKey =
    polishProvider && polishProviderSetting
      ? getProviderProbeCacheKey(
          polishProvider,
          polishProviderSetting.apiHost ||
            getProviderDefinition(polishProvider, llmSettings.customProviders).defaultApiHost ||
            '',
          fingerprintApiKey(polishProviderSetting.apiKey || '')
        )
      : null;
  const probe = probeKey ? readProviderProbeCache(probeKey) : undefined;
  const aiConfigured = isAiWritingConfigured(llmConfig);

  const aiStatus: { kind: CapabilityStatusKind; label: string } = (() => {
    if (!aiConfigured) {
      return {
        kind: 'needs_setup' as const,
        label: t('settings.capabilities.status_needs_setup', { defaultValue: 'Needs setup' }),
      };
    }
    if (probe?.status === 'connected') {
      return {
        kind: 'connected' as const,
        label: t('settings.capabilities.status_connected', {
          defaultValue: 'Connected · {{model}}',
          model: polishEntry?.model || polishProvider || '',
        }),
      };
    }
    if (probe?.status === 'not_running') {
      return {
        kind: 'not_running' as const,
        label: t('settings.capabilities.status_not_running', {
          defaultValue: 'Not running',
        }),
      };
    }
    if (probe?.status === 'needs_key') {
      return {
        kind: 'needs_key' as const,
        label: t('settings.capabilities.status_needs_key', { defaultValue: 'Needs key' }),
      };
    }
    if (probe?.status === 'error') {
      return {
        kind: 'error' as const,
        label: t('settings.capabilities.status_error', { defaultValue: 'Error' }),
      };
    }
    // Configured but not yet probed — do not claim Ready/Connected.
    return {
      kind: 'needs_setup' as const,
      label: t('settings.capabilities.status_unverified', {
        defaultValue: 'Saved · Verify connection',
      }),
    };
  })();

  const translationEntry = getFeatureModelEntry(llmConfig, 'translation');
  const translationReady = isTranslationConfigured(llmConfig);
  const translationLabel = translationEntry
    ? translationEntry.provider === 'google_translate_free'
      ? t('settings.capabilities.google_translate_free', {
          defaultValue: 'Google Translate (Free)',
        })
      : `${translationEntry.provider} / ${translationEntry.model}`
    : t('settings.capabilities.google_translate_free', {
        defaultValue: 'Google Translate (Free)',
      });

  const handleInstallRecommended = useCallback(async () => {
    if (installingRecommended) {
      return;
    }
    setInstallingRecommended(true);
    setInstallProgress(
      t('settings.capabilities.installing_recommended', {
        defaultValue: 'Installing recommended speech pack…',
      })
    );
    try {
      const paths = await downloadRecommendedOnboardingModels((update) => {
        setInstallProgress(
          t('settings.capabilities.install_progress', {
            defaultValue: '{{model}}: {{percentage}}%',
            model: update.modelId,
            percentage: Math.round(update.percentage),
          })
        );
      });
      updateConfig(getRecommendedOnboardingConfig(paths));
      setInstallProgress(null);
    } catch (error) {
      await showError({
        code: 'model.download_failed',
        messageKey: 'errors.model.download_failed',
        cause: error,
      });
      setInstallProgress(null);
    } finally {
      setInstallingRecommended(false);
    }
  }, [installingRecommended, showError, t, updateConfig]);

  void fullConfig;

  return (
    <SettingsTabContainer
      id="settings-panel-capabilities"
      ariaLabelledby="settings-tab-capabilities"
    >
      <SettingsPageHeader
        icon={<ModelIcon width={28} height={28} />}
        title={t('settings.capabilities.title', { defaultValue: 'Capabilities' })}
        description={t('settings.capabilities.description', {
          defaultValue:
            'Get speech recognition and AI writing ready. Advanced plumbing stays collapsed until you need it.',
        })}
      />

      <SettingsSection
        id="settings-capability-speech"
        title={t('settings.capabilities.speech_title', {
          defaultValue: 'Speech Recognition',
        })}
        description={t('settings.capabilities.speech_description', {
          defaultValue:
            'One model powers Live and Batch. Companion VAD and punctuation install automatically when needed.',
        })}
        icon={<Mic size={20} />}
        actions={<StatusBadge kind={speechStatus.kind} label={speechStatus.label} />}
      >
        <div className="capability-card-body">
          {installProgress && (
            <div className="capability-progress" role="status">
              <Loader2 className="spin" size={14} />
              {installProgress}
            </div>
          )}
          <div className="capability-actions">
            {!speechReady && (
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => void handleInstallRecommended()}
                disabled={recommendedDownloading}
              >
                {recommendedDownloading && <Loader2 className="spin" size={14} />}
                {t('settings.capabilities.install_recommended', {
                  defaultValue: 'Install recommended',
                })}
              </button>
            )}
            <button
              type="button"
              className={speechReady ? 'btn btn-primary' : 'btn btn-secondary'}
              onClick={() => setPickerOpen(true)}
            >
              {speechReady
                ? t('settings.capabilities.change_model', { defaultValue: 'Change model' })
                : t('settings.capabilities.choose_another_model', {
                    defaultValue: 'Choose another model',
                  })}
            </button>
          </div>
        </div>
      </SettingsSection>

      <SettingsSection
        id="settings-capability-ai-writing"
        title={t('settings.capabilities.ai_writing_title', {
          defaultValue: 'AI Writing',
        })}
        description={t('settings.capabilities.ai_writing_description', {
          defaultValue: 'One connection powers Polish and Summary.',
        })}
        icon={<Sparkles size={20} />}
        actions={<StatusBadge kind={aiStatus.kind} label={aiStatus.label} />}
      >
        <div className="capability-card-body">
          <div className="capability-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => setConnectOpen(true)}
            >
              {aiConfigured
                ? t('settings.capabilities.manage_connection', {
                    defaultValue: 'Manage connection',
                  })
                : t('settings.capabilities.connect_ai_writing', {
                    defaultValue: 'Connect AI Writing',
                  })}
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => nav?.navigateToTab('vocabulary')}
            >
              {t('settings.capabilities.edit_polish_prompts', {
                defaultValue: 'Edit polish prompts',
              })}
            </button>
          </div>
        </div>
      </SettingsSection>

      <SettingsSection
        id="settings-capability-translation"
        title={t('settings.capabilities.translation_title', {
          defaultValue: 'Translation',
        })}
        description={t('settings.capabilities.translation_description', {
          defaultValue: 'Ready out of the box with Google Translate (Free).',
        })}
        icon={<Globe size={20} />}
        actions={
          <StatusBadge
            kind={translationReady ? 'ready' : 'needs_setup'}
            label={
              translationReady
                ? t('settings.capabilities.status_ready_named', {
                    defaultValue: 'Ready · {{name}}',
                    name: translationLabel,
                  })
                : t('settings.capabilities.status_needs_setup', {
                    defaultValue: 'Needs setup',
                  })
            }
          />
        }
      >
        <div className="capability-card-body">
          <div className="capability-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => {
                setAdvancedOpen(true);
                setAdvancedAiOpen(true);
              }}
            >
              {t('settings.capabilities.change_translation', {
                defaultValue: 'Change engine',
              })}
              <ChevronRight size={14} />
            </button>
          </div>
        </div>
      </SettingsSection>

      <div id="settings-capabilities-advanced">
      <SettingsAccordion
        isOpen={advancedOpen}
        onToggle={() => setAdvancedOpen((prev) => !prev)}
        title={t('settings.capabilities.advanced_title', {
          defaultValue: 'Advanced',
        })}
        status={
          <span className="settings-hint" style={{ margin: 0 }}>
            {t('settings.capabilities.advanced_hint', {
              defaultValue: 'Live≠Batch, speakers, companions, providers, temperatures',
            })}
          </span>
        }
      >
        <div className="capability-advanced">
          <button
            type="button"
            className={`capability-advanced-toggle ${advancedSpeechOpen ? 'open' : ''}`}
            onClick={() => setAdvancedSpeechOpen((prev) => !prev)}
          >
            <Settings2 size={16} />
            {t('settings.capabilities.advanced_speech', {
              defaultValue: 'Speech advanced',
            })}
            <ChevronRight size={16} className="capability-advanced-chevron" />
          </button>
          {advancedSpeechOpen && (
            <div className="capability-advanced-panel" id="settings-model-hub">
              <SettingsModelsTab
                isActive={isActive && advancedSpeechOpen}
                embedded
                initialScenario={
                  initialFocus === 'batch' ? 'batch' : initialFocus === 'live' ? 'live' : undefined
                }
                subTabRequestId={
                  initialFocus === 'live' || initialFocus === 'batch' ? focusRequestId : undefined
                }
              />
            </div>
          )}

          <button
            type="button"
            className={`capability-advanced-toggle ${advancedAiOpen ? 'open' : ''}`}
            onClick={() => setAdvancedAiOpen((prev) => !prev)}
          >
            <RobotIcon width={16} height={16} />
            {t('settings.capabilities.advanced_ai', {
              defaultValue: 'AI advanced',
            })}
            <ChevronRight size={16} className="capability-advanced-chevron" />
          </button>
          {advancedAiOpen && (
            <div className="capability-advanced-panel" id="settings-llm-models-config">
              <SettingsLLMServiceTab
                isActive={isActive && advancedAiOpen}
                embedded
                onOpenProviderDetails={onOpenLlmProviderDetails}
              />
            </div>
          )}
        </div>
      </SettingsAccordion>
      </div>

      <ModelPickerModal isOpen={pickerOpen} onClose={() => setPickerOpen(false)} />
      <AiWritingConnectModal isOpen={connectOpen} onClose={() => setConnectOpen(false)} />
    </SettingsTabContainer>
  );
}

export default SettingsCapabilitiesTab;
