import { Keyboard } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../../hooks/useVoiceTypingReadiness';
import { useSetConfig, useVoiceTypingConfig } from '../../../stores/configStore';
import { SettingsTabContainer } from '../../settings/SettingsLayout';
import {
  VoiceTypingHistorySection,
  VoiceTypingSettingsSection,
} from '../../settings/SettingsSubtitleTab';
import { ToolHeroHeader } from '../ToolHeroHeader';

export function ToolVoiceTypingPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const vtConfig = useVoiceTypingConfig();
  const updateConfig = useSetConfig();
  const readiness = useVoiceTypingReadiness();

  const isEnabled = Boolean(vtConfig.voiceTypingEnabled);
  const isReady = readiness.state === 'ready';
  const statusTone = !isEnabled ? 'inactive' : isReady ? 'active' : 'warning';
  const statusText = !isEnabled
    ? t('tools.status_disabled', { defaultValue: 'Disabled' })
    : isReady
      ? t('tools.status_ready', { defaultValue: 'Ready' })
      : t('tools.status_needs_config', { defaultValue: 'Needs Config' });

  return (
    <SettingsTabContainer id="tools-panel-voice_typing" ariaLabelledby="tools-tab-voice_typing">
      <ToolHeroHeader
        icon={<Keyboard size={24} />}
        title={t('tools.voice_typing', { defaultValue: 'Voice Typing' })}
        description={t('settings.voice_typing_description', {
          defaultValue:
            'Configure dictation into other applications and see whether Voice Typing is ready to run.',
        })}
        statusText={statusText}
        statusTone={statusTone}
        enabled={isEnabled}
        onToggle={(checked) => updateConfig({ voiceTypingEnabled: checked })}
        switchAriaLabel={t('settings.voice_typing', { defaultValue: 'Voice Typing' })}
      />

      <div className="tools-panel-body">
        <VoiceTypingSettingsSection hideEnableSwitch={true} />
        <VoiceTypingHistorySection />
      </div>
    </SettingsTabContainer>
  );
}
