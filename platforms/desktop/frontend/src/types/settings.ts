export type SettingsTab =
  | 'general'
  | 'dashboard'
  | 'microphone'
  | 'subtitle'
  | 'models'
  | 'shortcuts'
  | 'about'
  | 'llm_service'
  | 'vocabulary'
  | 'automation'
  | 'storage'
  | 'api_server';

export type SettingsTabInput = SettingsTab | 'context' | 'voice_typing';
