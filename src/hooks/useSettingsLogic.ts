import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/configStore';

export type SettingsTab =
    | 'general'
    | 'dashboard'
    | 'microphone'
    | 'subtitle'
    | 'voice_typing'
    | 'models'
    | 'shortcuts'
    | 'about'
    | 'llm_service'
    | 'vocabulary'
    | 'automation';
export type SettingsTabInput = SettingsTab | 'context';

function normalizeInitialSettingsTab(initialTab?: SettingsTabInput): SettingsTab {
    if (!initialTab) {
        return 'general';
    }

    return initialTab === 'context' ? 'vocabulary' : initialTab;
}

/**
 * Hook managing local UI state for the Settings dialog:
 * active tab and language synchronisation.
 */
export function useSettingsLogic(_isOpen: boolean, _onClose: () => void, initialTab?: SettingsTabInput) {
    const config = useConfigStore((state) => state.config);
    const setConfig = useConfigStore((state) => state.setConfig);
    const { i18n } = useTranslation();

    const [tabState, setTabState] = useState(() => ({
        isOpen: _isOpen,
        initialTab,
        activeTab: _isOpen ? normalizeInitialSettingsTab(initialTab) : 'general',
    }));

    let activeTab = tabState.activeTab;
    if (tabState.isOpen !== _isOpen || tabState.initialTab !== initialTab) {
        activeTab = _isOpen ? normalizeInitialSettingsTab(initialTab) : 'general';
        setTabState({
            isOpen: _isOpen,
            initialTab,
            activeTab,
        });
    }

    const setActiveTab = useCallback((nextTab: SettingsTab) => {
        setTabState((current) => ({
            ...current,
            activeTab: nextTab,
        }));
    }, []);

    // Sync language change
    useEffect(() => {
        if (config.appLanguage === 'auto') {
            i18n.changeLanguage(navigator.language);
        } else {
            i18n.changeLanguage(config.appLanguage);
        }
    }, [config.appLanguage, i18n]);

    const updateConfig = (updates: Partial<typeof config>) => {
        setConfig(updates);
    };

    return {
        activeTab,
        setActiveTab,
        config,
        updateConfig,
    };
}
