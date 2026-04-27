import React, { createContext, useContext } from 'react';
import type { SettingsTab } from '../../hooks/useSettingsLogic';

interface SettingsNavigationContextValue {
    activeTab: SettingsTab;
    navigateToTab: (tab: SettingsTab) => void;
}

const SettingsNavigationContext = createContext<SettingsNavigationContextValue | null>(null);

export function SettingsNavigationProvider({
    value,
    children,
}: {
    value: SettingsNavigationContextValue;
    children: React.ReactNode;
}): React.JSX.Element {
    return (
        <SettingsNavigationContext.Provider value={value}>
            {children}
        </SettingsNavigationContext.Provider>
    );
}

export function useSettingsNavigation(): SettingsNavigationContextValue {
    const context = useContext(SettingsNavigationContext);
    if (!context) {
        throw new Error('useSettingsNavigation must be used within SettingsNavigationProvider');
    }

    return context;
}
