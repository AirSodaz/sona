import { useShallow } from 'zustand/shallow';
import { Update } from '@tauri-apps/plugin-updater';
import { useAppUpdaterStore, UpdateStatus } from '../stores/appUpdaterStore';

interface UseAppUpdaterReturn {
  status: UpdateStatus;
  error: string | null;
  updateInfo: Update | null;
  checkUpdate: (manual?: boolean) => Promise<void>;
  installUpdate: () => Promise<void>;
  progress: number;
  notificationVisible: boolean;
  dismissNotification: () => void;
  relaunchToUpdate: () => Promise<void>;
}

export function useAppUpdater(): UseAppUpdaterReturn {
  return useAppUpdaterStore(useShallow((state) => ({
    status: state.status,
    error: state.error,
    updateInfo: state.updateInfo,
    checkUpdate: state.checkUpdate,
    installUpdate: state.installUpdate,
    progress: state.progress,
    notificationVisible: state.notificationVisible,
    dismissNotification: state.dismissNotification,
    relaunchToUpdate: state.relaunchToUpdate,
  })));
}
