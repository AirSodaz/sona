import packageJson from '../../package.json';

export type DesktopReleaseChannel = 'stable' | 'nightly';

/**
 * Resolves the desktop application release channel.
 *
 * Precedence:
 * 1. Explicit VITE_APP_CHANNEL environment variable (e.g. 'nightly' | 'stable')
 * 2. Version-based inference (nightly versions contain a hyphen, e.g. 0.8.2-45)
 * 3. Default fallback to 'stable'
 */
export function getAppReleaseChannel(
  version: string = packageJson.version,
  envChannel?: string,
): DesktopReleaseChannel {
  const normalizedEnv = (
    envChannel ?? (import.meta.env?.VITE_APP_CHANNEL as string | undefined)
  )
    ?.toLowerCase()
    .trim();

  if (normalizedEnv === 'nightly') {
    return 'nightly';
  }
  if (normalizedEnv === 'stable') {
    return 'stable';
  }

  return version.includes('-') ? 'nightly' : 'stable';
}

export function formatChannelBadgeLabel(channel: DesktopReleaseChannel): 'Stable' | 'Nightly' {
  return channel === 'nightly' ? 'Nightly' : 'Stable';
}
