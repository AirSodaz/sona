import packageJson from '../../package.json';
import type { AppBuildInfo, AppReleaseChannel } from '../types/build';

export type DesktopReleaseChannel = AppReleaseChannel;

/**
 * Returns the full structured AppBuildInfo.
 * Reads synchronously from Vite compile-time `__APP_BUILD_INFO__`,
 * falling back gracefully in unit test or node environments.
 */
export function getAppBuildInfo(): AppBuildInfo {
  if (typeof __APP_BUILD_INFO__ !== 'undefined' && __APP_BUILD_INFO__) {
    return __APP_BUILD_INFO__;
  }

  // Fallback if __APP_BUILD_INFO__ is not defined
  const envChannel = (import.meta.env?.VITE_APP_CHANNEL as string | undefined)
    ?.toLowerCase()
    .trim();
  const channel: AppReleaseChannel =
    envChannel === 'nightly' ||
    envChannel === 'ci' ||
    envChannel === 'dev' ||
    envChannel === 'stable'
      ? envChannel
      : packageJson.version.includes('-')
        ? 'nightly'
        : 'stable';

  const isUpdateSupported = channel === 'stable' || channel === 'nightly';

  return {
    semver: packageJson.version,
    channel,
    commitSha: 'unknown',
    isDirty: false,
    buildNumber: null,
    buildTimestamp: new Date().toISOString(),
    displayVersion: packageJson.version,
    isUpdateSupported,
  };
}

/**
 * Resolves the desktop application release channel.
 *
 * Precedence:
 * 1. Explicit VITE_APP_CHANNEL environment variable (e.g. 'nightly' | 'stable' | 'ci' | 'dev')
 * 2. Explicit non-default version string override (e.g. '0.8.2-45' -> 'nightly')
 * 3. Channel from getAppBuildInfo()
 */
export function getAppReleaseChannel(version?: string, envChannel?: string): AppReleaseChannel {
  const normalizedEnv = (envChannel ?? (import.meta.env?.VITE_APP_CHANNEL as string | undefined))
    ?.toLowerCase()
    .trim();

  if (
    normalizedEnv === 'nightly' ||
    normalizedEnv === 'stable' ||
    normalizedEnv === 'ci' ||
    normalizedEnv === 'dev'
  ) {
    return normalizedEnv;
  }

  if (version !== undefined && version !== packageJson.version) {
    return version.includes('-') ? 'nightly' : 'stable';
  }

  return getAppBuildInfo().channel;
}

export function formatChannelBadgeLabel(channel: AppReleaseChannel): string {
  switch (channel) {
    case 'nightly':
      return 'Nightly';
    case 'ci':
      return 'CI Build';
    case 'dev':
      return 'Dev';
    case 'stable':
    default:
      return 'Stable';
  }
}

export function isUpdateSupported(channel: AppReleaseChannel): boolean {
  return channel === 'stable' || channel === 'nightly';
}
