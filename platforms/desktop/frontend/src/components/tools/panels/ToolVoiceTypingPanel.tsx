import { Keyboard } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../../hooks/useVoiceTypingReadiness';
import { useSetConfig, useVoiceTypingConfig } from '../../../stores/configStore';
import { Switch } from '../../Switch';
import { SettingsPageHeader, SettingsTabContainer } from '../../settings/SettingsLayout';
import {
  VoiceTypingHistorySection,
  VoiceTypingSettingsSection,
} from '../../settings/SettingsSubtitleTab';

export function ToolVoiceTypingPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const vtConfig = useVoiceTypingConfig();
  const updateConfig = useSetConfig();
  const readiness = useVoiceTypingReadiness();

  const isEnabled = Boolean(vtConfig.voiceTypingEnabled);
  const isReady = readiness.state === 'ready';

  return (
    <SettingsTabContainer id="tools-panel-voice-typing" ariaLabelledby="tools-tab-voice_typing">
      <div className="tool-hero-header">
        <SettingsPageHeader
          icon={<Keyboard width={28} height={28} />}
          title={t('tools.voice_typing', { defaultValue: 'Voice Typing' })}
          description={t('settings.voice_typing_description', {
            defaultValue:
              'Configure dictation into other applications and see whether Voice Typing is ready to run.',
          })}
        />
        <div className="tool-hero-control">
          <div className="tool-hero-status">
            <span
              className={`tools-status-badge tools-status-${
                !isEnabled ? 'inactive' : isReady ? 'active' : 'warning'
              }`}
            >
              <span className="tools-status-dot" aria-hidden="true" />
              <span>
                {!isEnabled
                  ? t('tools.status_disabled', { defaultValue: 'Disabled' })
                  : isReady
                    ? t('tools.status_ready', { defaultValue: 'Ready' })
                    : t('tools.status_needs_config', { defaultValue: 'Needs Config' })}
              </span>
            </span>
          </div>
          <Switch
            checked={isEnabled}
            onChange={(checked) => updateConfig({ voiceTypingEnabled: checked })}
            aria-label={t('settings.voice_typing', { defaultValue: 'Voice Typing' })}
          />
        </div>
      </div>

      <div
        className="tools-panel-body"
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--spacing-xl, 32px)',
        }}
      >
        <VoiceTypingSettingsSection />
        <VoiceTypingHistorySection />
      </div>
    </SettingsTabContainer>
  );
}
