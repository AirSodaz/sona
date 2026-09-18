/* eslint-disable react-refresh/only-export-components */

import type React from 'react';
import { createContext, useContext } from 'react';
import type { SettingsTab } from '../../types/settings';

interface SettingsNavigationContextValue {
  activeTab: SettingsTab;
  navigateToTab: (tab: SettingsTab) => void;
}

export const SettingsNavigationContext = createContext<SettingsNavigationContextValue | null>(null);

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

export function useOptionalSettingsNavigation(): SettingsNavigationContextValue | null {
  return useContext(SettingsNavigationContext);
}
