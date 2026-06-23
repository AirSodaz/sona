import { create } from 'zustand';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { openUrl } from '@tauri-apps/plugin-opener';
import i18n from '../i18n';
import { runGuardedQuit } from '../services/quitGuard';
import { fetchUrl } from '../services/tauri/app';
import { buildErrorDialogViewModel, extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { useConfigStore } from './configStore';
import { useErrorDialogStore } from './errorDialogStore';
import packageJson from '../../package.json';

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'uptodate'
  | 'downloading'
  | 'installing'
  | 'downloaded'
  | 'error';

export type Channel = 'stable' | 'nightly';

export interface CheckUpdateOptions {
  manual?: boolean;
  channelSwitch?: boolean;
}

interface AppUpdaterState {
  status: UpdateStatus;
  error: string | null;
  updateInfo: Update | null;
  progress: number;
  dismissedVersion: string | null;
  notificationVisible: boolean;
  hasAutoCheckedThisSession: boolean;
  channel: Channel;
  crossChannelDownloadUrl: string | null;
  checkUpdate: (opts?: CheckUpdateOptions) => Promise<void>;
  installUpdate: () => Promise<void>;
  dismissNotification: () => void;
  relaunchToUpdate: () => Promise<void>;
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

function getPlatformKey(): string {
  const platform = navigator.platform?.toLowerCase() || '';
  const userAgent = navigator.userAgent?.toLowerCase() || '';

  if (platform.includes('win')) {
    return userAgent.includes('aarch64') || userAgent.includes('arm64')
      ? 'windows-aarch64'
      : 'windows-x86_64';
  }
  if (platform.includes('mac')) {
    return userAgent.includes('aarch64') || userAgent.includes('arm64')
      ? 'darwin-aarch64'
      : 'darwin-x86_64';
  }
  return 'linux-x86_64';
}

export const useAppUpdaterStore = create<AppUpdaterState>((set, get) => ({
  status: 'idle',
  error: null,
  updateInfo: null,
  progress: 0,
  dismissedVersion: null,
  notificationVisible: false,
  hasAutoCheckedThisSession: false,
  channel: 'stable',
  crossChannelDownloadUrl: null,

  checkUpdate: async (opts?: CheckUpdateOptions) => {
    const state = get();
    const channel = opts?.channelSwitch
      ? (useConfigStore.getState().config.channel ?? 'stable')
      : state.channel;

    if (state.status === 'checking') {
      return;
    }

    if (!opts?.manual && state.hasAutoCheckedThisSession) {
      return;
    }

    const previousStatus = state.status;
    const previousUpdateInfo = state.updateInfo;
    const previousNotificationVisible = state.notificationVisible;

    set({
      status: 'checking',
      error: null,
      progress: 0,
      channel,
      crossChannelDownloadUrl: null,
      hasAutoCheckedThisSession: opts?.manual ? state.hasAutoCheckedThisSession : true,
    });

    // Cross-channel switch from nightly back to stable:
    // semver cannot downgrade, so we fetch updater.json manually and offer browser download.
    if (opts?.channelSwitch && channel === 'stable') {
      try {
        const responseText = await fetchUrl('https://github.com/AirSodaz/sona/releases/latest/download/updater.json');
        const data = JSON.parse(responseText);
        const stableVersion: string = data.version;

        if (stableVersion && stableVersion !== packageJson.version) {
          const platformKey = getPlatformKey();
          const downloadUrl = data.platforms?.[platformKey]?.url;
          if (!downloadUrl) {
            throw new Error('No matching platform URL found in updater.json');
          }
          set({
            status: 'available',
            updateInfo: null,
            error: null,
            crossChannelDownloadUrl: downloadUrl,
          });
          return;
        }

        // Same version as current — up to date
        set({
          status: 'uptodate',
          error: null,
          notificationVisible: false,
        });
      } catch (error) {
        const errorMessage = extractErrorMessage(error);
        logger.error('Cross-channel fetch failed:', error);
        set({
          status: 'error',
          error: errorMessage,
        });
      }
      return;
    }

    // Normal check (within-channel or stable→nightly switch)
    try {
      const endpoints = channel === 'nightly'
        ? ['https://github.com/AirSodaz/sona/releases/latest/download/updater-nightly.json']
        : undefined;

      const update = await check({ endpoints });

      if (update) {
        const dismissedVersion = get().dismissedVersion;
        set({
          updateInfo: update,
          status: 'available',
          error: null,
          progress: 0,
          notificationVisible: opts?.manual
            ? previousNotificationVisible
            : dismissedVersion !== update.version,
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
      const errorMessage = extractErrorMessage(error);

      if (opts?.manual) {
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
        error: extractErrorMessage(error),
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
        error: extractErrorMessage(error),
      });
      await showUpdateError(error);
    }
  },
}));
