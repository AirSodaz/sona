import { create } from 'zustand';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { openUrl } from '@tauri-apps/plugin-opener';
import i18n from '../i18n';
import { runGuardedQuit } from '../services/quitGuard';
import { buildErrorDialogViewModel } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { useErrorDialogStore } from './errorDialogStore';

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'uptodate'
  | 'downloading'
  | 'installing'
  | 'downloaded'
  | 'error';

interface AppUpdaterState {
  status: UpdateStatus;
  error: string | null;
  updateInfo: Update | null;
  progress: number;
  dismissedVersion: string | null;
  notificationVisible: boolean;
  hasAutoCheckedThisSession: boolean;
  checkUpdate: (manual?: boolean) => Promise<void>;
  installUpdate: () => Promise<void>;
  dismissNotification: () => void;
  relaunchToUpdate: () => Promise<void>;
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

async function openLatestReleasePage() {
  try {
    await openUrl('https://github.com/AirSodaz/sona/releases/latest');
  } catch (openErr) {
    logger.error('Failed to open URL:', openErr);
  }
}

async function showUpdateError(error: unknown) {
  const showError = useErrorDialogStore.getState().showError;
  const result = await showError(buildErrorDialogViewModel(i18n.t.bind(i18n), {
    code: 'update.failed',
    messageKey: 'errors.update.failed',
    cause: error,
    primaryActionLabelKey: 'settings.update_download_manually',
  }));

  if (result === 'primary') {
    await openLatestReleasePage();
  }
}

export const useAppUpdaterStore = create<AppUpdaterState>((set, get) => ({
  status: 'idle',
  error: null,
  updateInfo: null,
  progress: 0,
  dismissedVersion: null,
  notificationVisible: false,
  hasAutoCheckedThisSession: false,

  checkUpdate: async (manual = false) => {
    const state = get();

    if (state.status === 'checking') {
      return;
    }

    if (!manual && state.hasAutoCheckedThisSession) {
      return;
    }

    const previousStatus = state.status;
    const previousUpdateInfo = state.updateInfo;
    const previousNotificationVisible = state.notificationVisible;

    set({
      status: 'checking',
      error: null,
      progress: 0,
      hasAutoCheckedThisSession: manual ? state.hasAutoCheckedThisSession : true,
    });

    try {
      const update = await check();

      if (update) {
        const dismissedVersion = get().dismissedVersion;
        set({
          updateInfo: update,
          status: 'available',
          error: null,
          progress: 0,
          notificationVisible: manual ? previousNotificationVisible : dismissedVersion !== update.version,
        });
        return;
      }

      set({
        updateInfo: null,
        status: 'uptodate',
        error: null,
        progress: 0,
        notificationVisible: false,
      });
    } catch (error) {
      logger.error('Update check failed:', error);
      const errorMessage = getErrorMessage(error);

      if (manual) {
        set({
          updateInfo: previousUpdateInfo,
          status: previousUpdateInfo ? 'available' : 'idle',
          error: errorMessage,
          progress: 0,
          notificationVisible: previousNotificationVisible,
        });
        await showUpdateError(error);
        return;
      }

      set({
        updateInfo: previousUpdateInfo,
        status: previousUpdateInfo ? 'available' : previousStatus,
        error: errorMessage,
        progress: previousStatus === 'downloaded' ? 100 : 0,
        notificationVisible: previousNotificationVisible,
      });
    }
  },

  installUpdate: async () => {
    const { updateInfo, status } = get();

    if (!updateInfo || status === 'downloading' || status === 'installing') {
      return;
    }

    set({
      status: 'downloading',
      error: null,
      progress: 0,
    });

    try {
      let downloaded = 0;
      let contentLength = 0;

      await updateInfo.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          contentLength = event.data.contentLength || 0;
          return;
        }

        if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          if (contentLength > 0) {
            set({
              progress: Math.round((downloaded / contentLength) * 100),
            });
          }
          return;
        }

        set({
          status: 'installing',
        });
      });

      set({
        status: 'downloaded',
        error: null,
        progress: 100,
      });
    } catch (error) {
      logger.error('Update installation failed:', error);
      set((state) => ({
        status: state.updateInfo ? 'available' : 'idle',
        error: getErrorMessage(error),
        progress: 0,
      }));
      await showUpdateError(error);
    }
  },

  dismissNotification: () => {
    const { dismissedVersion, updateInfo } = get();
    set({
      dismissedVersion: updateInfo?.version ?? dismissedVersion,
      notificationVisible: false,
    });
  },

  relaunchToUpdate: async () => {
    try {
      await runGuardedQuit(async () => {
        await relaunch();
      });
    } catch (error) {
      logger.error('Update relaunch failed:', error);
      set({
        error: getErrorMessage(error),
      });
      await showUpdateError(error);
    }
  },
}));
