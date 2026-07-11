import { useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';
import { useAppUpdaterStore } from '../stores/appUpdaterStore';

export function useAutoUpdateCheck(isLoaded: boolean) {
  const autoCheckUpdates = useConfigStore((state) => state.config.autoCheckUpdates ?? true);
  const hasAutoCheckedThisSession = useAppUpdaterStore((state) => state.hasAutoCheckedThisSession);
  const checkUpdate = useAppUpdaterStore((state) => state.checkUpdate);

  useEffect(() => {
    if (!isLoaded || !autoCheckUpdates || hasAutoCheckedThisSession) {
      return;
    }

    const timer = window.setTimeout(() => {
      void checkUpdate(false);
    }, 5000);

    return () => window.clearTimeout(timer);
  }, [autoCheckUpdates, checkUpdate, hasAutoCheckedThisSession, isLoaded]);
}
