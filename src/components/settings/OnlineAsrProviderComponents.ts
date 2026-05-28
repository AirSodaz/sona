import React from 'react';
import { ProviderSettingsProps, VolcengineSettingsCard, GroqWhisperSettingsCard } from './OnlineAsrSettingsCards';
import { VOLCENGINE_DOUBAO_PROVIDER_ID, GROQ_WHISPER_PROVIDER_ID } from '../../services/onlineAsrProviders';

export const CUSTOM_PROVIDER_COMPONENTS: Record<string, React.ComponentType<ProviderSettingsProps>> = {
    [VOLCENGINE_DOUBAO_PROVIDER_ID]: VolcengineSettingsCard,
    [GROQ_WHISPER_PROVIDER_ID]: GroqWhisperSettingsCard,
};
