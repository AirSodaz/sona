import type { SettingsTab } from '../../hooks/useSettingsLogic';
import { preloadSettingsGeneralDeferredSections } from './settingsGeneralDeferredLoaders';

export const SETTINGS_TABS = [
    'general',
    'dashboard',
    'microphone',
    'subtitle',
    'models',
    'vocabulary',
    'automation',
    'api_server',
    'llm_service',
    'shortcuts',
    'about',
] as const satisfies readonly SettingsTab[];

function memoizeLoader<T>(loader: () => Promise<T>): () => Promise<T> {
    let promise: Promise<T> | null = null;

    return () => {
        if (!promise) {
            promise = loader();
        }

        return promise;
    };
}

const loadSettingsGeneralModule = memoizeLoader(async () => import('./SettingsGeneralTab'));

export const loadSettingsGeneralTab = memoizeLoader(async () => {
    const module = await loadSettingsGeneralModule();
    return { default: module.SettingsGeneralTab };
});

export const loadSettingsDashboardTab = memoizeLoader(async () => {
    const module = await import('./SettingsDashboardTab');
    return { default: module.SettingsDashboardTab };
});

export const loadSettingsMicrophoneTab = memoizeLoader(async () => {
    const module = await import('./SettingsMicrophoneTab');
    return { default: module.SettingsMicrophoneTab };
});

export const loadSettingsSubtitleTab = memoizeLoader(async () => {
    const module = await import('./SettingsSubtitleTab');
    return { default: module.SettingsSubtitleTab };
});

export const loadSettingsModelsPane = memoizeLoader(async () => {
    const module = await import('./SettingsModelsPane');
    return { default: module.SettingsModelsPane };
});

export const loadSettingsVocabularyTab = memoizeLoader(async () => {
    const module = await import('./SettingsVocabularyTab');
    return { default: module.SettingsVocabularyTab };
});

export const loadSettingsAutomationTab = memoizeLoader(async () => {
    const module = await import('./SettingsAutomationTab');
    return { default: module.SettingsAutomationTab };
});

export const loadSettingsApiServerTab = memoizeLoader(async () => {
    const module = await import('./SettingsApiServerTab');
    return { default: module.SettingsApiServerTab };
});

export const loadSettingsLLMServiceTab = memoizeLoader(async () => {
    const module = await import('./SettingsLLMServiceTab');
    return { default: module.SettingsLLMServiceTab };
});

export const loadSettingsShortcutsTab = memoizeLoader(async () => {
    const module = await import('./SettingsShortcutsTab');
    return { default: module.SettingsShortcutsTab };
});

export const loadSettingsAboutTab = memoizeLoader(async () => {
    const module = await import('./SettingsAboutTab');
    return { default: module.SettingsAboutTab };
});

async function preloadSettingsGeneralTab(): Promise<void> {
    await Promise.all([
        loadSettingsGeneralTab(),
        preloadSettingsGeneralDeferredSections(),
    ]);
}

const settingsPanePreloaders: Record<SettingsTab, () => Promise<void>> = {
    general: preloadSettingsGeneralTab,
    dashboard: () => loadSettingsDashboardTab().then(() => undefined),
    microphone: () => loadSettingsMicrophoneTab().then(() => undefined),
    subtitle: () => loadSettingsSubtitleTab().then(() => undefined),
    models: () => loadSettingsModelsPane().then(() => undefined),
    vocabulary: () => loadSettingsVocabularyTab().then(() => undefined),
    automation: () => loadSettingsAutomationTab().then(() => undefined),
    api_server: () => loadSettingsApiServerTab().then(() => undefined),
    llm_service: () => loadSettingsLLMServiceTab().then(() => undefined),
    shortcuts: () => loadSettingsShortcutsTab().then(() => undefined),
    about: () => loadSettingsAboutTab().then(() => undefined),
};

export function preloadSettingsTab(tab: SettingsTab): Promise<void> {
    return settingsPanePreloaders[tab]();
}

export function preloadAllSettingsTabs(): Promise<void> {
    return Promise.all(SETTINGS_TABS.map((tab) => preloadSettingsTab(tab))).then(() => undefined);
}
