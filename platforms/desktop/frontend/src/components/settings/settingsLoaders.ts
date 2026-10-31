import type { SettingsTab } from '../../types/settings';
import { preloadSettingsGeneralDeferredSections } from './settingsGeneralDeferredLoaders';

export const SETTINGS_TAB_GROUPS: ReadonlyArray<{
  labelKey: string;
  tabs: readonly SettingsTab[];
}> = [
  {
    labelKey: 'settings.group.general',
    tabs: ['general', 'dashboard', 'shortcuts', 'about'],
  },
  {
    labelKey: 'settings.group.audio',
    tabs: ['microphone', 'capabilities', 'vocabulary'],
  },
  {
    labelKey: 'settings.group.intelligence',
    tabs: ['automation'],
  },
  {
    labelKey: 'settings.group.infrastructure',
    tabs: ['storage', 'sync'],
  },
] as const;

export const SETTINGS_TABS = SETTINGS_TAB_GROUPS.flatMap((g) => g.tabs) as unknown as readonly [
  SettingsTab,
  ...SettingsTab[],
];

function memoizeLoader<T>(loader: () => Promise<T>): () => Promise<T> {
  let promise: Promise<T> | null = null;

  return () => {
    if (!promise) {
      promise = loader();
    }

    return promise;
  };
}

function resolveModuleComponent<M extends object, K extends keyof M>(
  module: M,
  exportName: K
): { default: M[K] } {
  if (exportName in module && module[exportName]) {
    return { default: module[exportName] };
  }
  const candidate = module as { default?: M[K] };
  if ('default' in candidate && candidate.default) {
    return { default: candidate.default };
  }
  return { default: (module[exportName] ?? candidate.default) as M[K] };
}

const loadSettingsGeneralModule = memoizeLoader(async () => import('./SettingsGeneralTab'));

export const loadSettingsGeneralTab = memoizeLoader(async () => {
  const module = await loadSettingsGeneralModule();
  return resolveModuleComponent(module, 'SettingsGeneralTab');
});

export const loadSettingsDashboardTab = memoizeLoader(async () => {
  const module = await import('./SettingsDashboardTab');
  return resolveModuleComponent(module, 'SettingsDashboardTab');
});

export const loadSettingsMicrophoneTab = memoizeLoader(async () => {
  const module = await import('./SettingsMicrophoneTab');
  return resolveModuleComponent(module, 'SettingsMicrophoneTab');
});

export const loadSettingsModelsPane = memoizeLoader(async () => {
  const module = await import('./SettingsModelsPane');
  return resolveModuleComponent(module, 'SettingsModelsPane');
});

export const loadSettingsCapabilitiesPane = memoizeLoader(async () => {
  const module = await import('./SettingsCapabilitiesPane');
  return resolveModuleComponent(module, 'SettingsCapabilitiesPane');
});

export const loadSettingsVocabularyTab = memoizeLoader(async () => {
  const module = await import('./SettingsVocabularyTab');
  return resolveModuleComponent(module, 'SettingsVocabularyTab');
});

export const loadSettingsAutomationTab = memoizeLoader(async () => {
  const module = await import('./SettingsAutomationTab');
  return resolveModuleComponent(module, 'SettingsAutomationTab');
});

export const loadSettingsStorageTab = memoizeLoader(async () => {
  const module = await import('./SettingsStorageTab');
  return resolveModuleComponent(module, 'SettingsStorageTab');
});
export const loadSettingsSyncTab = memoizeLoader(async () => {
  const module = await import('./SettingsSyncTab');
  return resolveModuleComponent(module, 'SettingsSyncTab');
});

export const loadSettingsLLMServiceTab = memoizeLoader(async () => {
  const module = await import('./SettingsLLMServiceTab');
  return resolveModuleComponent(module, 'SettingsLLMServiceTab');
});

export const loadSettingsShortcutsTab = memoizeLoader(async () => {
  const module = await import('./SettingsShortcutsTab');
  return resolveModuleComponent(module, 'SettingsShortcutsTab');
});

export const loadSettingsAboutTab = memoizeLoader(async () => {
  const module = await import('./SettingsAboutTab');
  return resolveModuleComponent(module, 'SettingsAboutTab');
});

async function preloadSettingsGeneralTab(): Promise<void> {
  await Promise.all([loadSettingsGeneralTab(), preloadSettingsGeneralDeferredSections()]);
}

const settingsPanePreloaders: Record<SettingsTab, () => Promise<void>> = {
  general: preloadSettingsGeneralTab,
  dashboard: () => loadSettingsDashboardTab().then(() => undefined),
  microphone: () => loadSettingsMicrophoneTab().then(() => undefined),
  subtitle: () => Promise.resolve(),
  capabilities: () => loadSettingsCapabilitiesPane().then(() => undefined),
  models: () => loadSettingsCapabilitiesPane().then(() => undefined),
  vocabulary: () => loadSettingsVocabularyTab().then(() => undefined),
  automation: () => loadSettingsAutomationTab().then(() => undefined),
  storage: () => loadSettingsStorageTab().then(() => undefined),
  sync: () => loadSettingsSyncTab().then(() => undefined),
  api_server: () => Promise.resolve(),
  mcp: () => Promise.resolve(),
  llm_service: () => loadSettingsCapabilitiesPane().then(() => undefined),
  shortcuts: () => loadSettingsShortcutsTab().then(() => undefined),
  about: () => loadSettingsAboutTab().then(() => undefined),
};

export function preloadSettingsTab(tab: SettingsTab): Promise<void> {
  return settingsPanePreloaders[tab]();
}

export function preloadAllSettingsTabs(): Promise<void> {
  return Promise.all(SETTINGS_TABS.map((tab) => preloadSettingsTab(tab))).then(() => undefined);
}
