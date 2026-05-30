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
    | 'automation'
    | 'api_server';
export type SettingsTabInput = SettingsTab | 'context';

interface SettingsTabState {
    isOpen: boolean;
    initialTab?: SettingsTabInput;
    activeTab: SettingsTab;
}

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

    const [tabState, setTabState] = useState<SettingsTabState>({
        isOpen: _isOpen,
        initialTab,
        activeTab: _isOpen ? normalizeInitialSettingsTab(initialTab) : 'general',
    });

    const derivedActiveTab = _isOpen ? normalizeInitialSettingsTab(initialTab) : 'general';
    const isTabStateSynced = tabState.isOpen === _isOpen && tabState.initialTab === initialTab;
    const activeTab = isTabStateSynced ? tabState.activeTab : derivedActiveTab;

    useEffect(() => {
        let cancelled = false;
        queueMicrotask(() => {
            if (cancelled) {
                return;
            }

            setTabState((current) => {
                if (
                    current.isOpen === _isOpen
                    && current.initialTab === initialTab
                ) {
                    return current;
                }

                return {
                    isOpen: _isOpen,
                    initialTab,
                    activeTab: derivedActiveTab,
                };
            });
        });

        return () => {
            cancelled = true;
        };
    }, [_isOpen, derivedActiveTab, initialTab]);

    const setActiveTab = useCallback((nextTab: SettingsTab) => {
        setTabState({
            isOpen: _isOpen,
            initialTab,
            activeTab: nextTab,
        });
    }, [_isOpen, initialTab]);

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
