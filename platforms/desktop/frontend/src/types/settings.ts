export type SettingsTab =
  | 'general'
  | 'dashboard'
  | 'microphone'
  | 'subtitle'
  | 'capabilities'
  /** @deprecated Redirects to capabilities. Kept for deep-links and search aliases. */
  | 'models'
  | 'shortcuts'
  | 'about'
  /** @deprecated Redirects to capabilities. Kept for deep-links and search aliases. */
  | 'llm_service'
  | 'vocabulary'
  | 'automation'
  | 'storage'
  | 'sync'
  | 'api_server'
  | 'mcp';

export type SettingsTabInput = SettingsTab | 'context' | 'voice_typing';

/** Legacy Model Settings / LLM Service tabs collapse into Capabilities. */
export function normalizeSettingsTab(tab: SettingsTabInput | undefined): SettingsTab {
  if (!tab) {
    return 'general';
  }
  if (tab === 'context') {
    return 'vocabulary';
  }
  if (tab === 'voice_typing') {
    return 'subtitle';
  }
  if (tab === 'models' || tab === 'llm_service') {
    return 'capabilities';
  }
  return tab;
}
