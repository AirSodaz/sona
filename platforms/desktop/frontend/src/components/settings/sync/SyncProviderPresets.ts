export type WellKnownSyncProviderId =
  | 'nutstore'
  | 'nextcloud'
  | 'infinicloud'
  | 'synology'
  | 'alist'
  | 'custom';

export interface SyncProviderPreset {
  id: WellKnownSyncProviderId;
  nameKey: string;
  defaultName: string;
  badgeKey: string;
  defaultBadge: string;
  defaultServerUrl: string;
  defaultRemoteRoot: string;
  usernamePlaceholder: string;
  helpKey: string;
  helpDefault: string;
  authDocUrl?: string;
}

export const SYNC_PROVIDER_PRESETS: readonly SyncProviderPreset[] = [
  {
    id: 'nutstore',
    nameKey: 'settings.sync.preset_nutstore',
    defaultName: 'Nutstore',
    badgeKey: 'settings.sync.badge_popular',
    defaultBadge: 'Popular',
    defaultServerUrl: 'https://dav.jianguoyun.com/dav/',
    defaultRemoteRoot: 'Sona',
    usernamePlaceholder: 'account@example.com',
    helpKey: 'settings.sync.preset_help_nutstore',
    helpDefault: 'Generate an app password in Nutstore: Account Info -> Security -> Third-party apps.',
    authDocUrl: 'https://help.jianguoyun.com/?p=2064',
  },
  {
    id: 'nextcloud',
    nameKey: 'settings.sync.preset_nextcloud',
    defaultName: 'Nextcloud / ownCloud',
    badgeKey: 'settings.sync.badge_selfhosted',
    defaultBadge: 'Self-hosted',
    defaultServerUrl: 'https://cloud.example.com/remote.php/dav/files/USERNAME/',
    defaultRemoteRoot: 'Sona',
    usernamePlaceholder: 'username',
    helpKey: 'settings.sync.preset_help_nextcloud',
    helpDefault: 'Generate an app password in Nextcloud: Personal Settings -> Security -> Devices & Sessions.',
  },
  {
    id: 'infinicloud',
    nameKey: 'settings.sync.preset_infinicloud',
    defaultName: 'InfiniCLOUD',
    badgeKey: 'settings.sync.badge_cloud',
    defaultBadge: 'Cloud',
    defaultServerUrl: 'https://<account>.teracloud.jp/dav/',
    defaultRemoteRoot: 'Sona',
    usernamePlaceholder: 'account-name',
    helpKey: 'settings.sync.preset_help_infinicloud',
    helpDefault: 'Enable Apps Connection in InfiniCLOUD My Page to get an app password.',
  },
  {
    id: 'synology',
    nameKey: 'settings.sync.preset_synology',
    defaultName: 'Synology NAS',
    badgeKey: 'settings.sync.badge_nas',
    defaultBadge: 'NAS',
    defaultServerUrl: 'https://your-nas.synology.me:5006/home/',
    defaultRemoteRoot: 'Sona',
    usernamePlaceholder: 'dsm_username',
    helpKey: 'settings.sync.preset_help_synology',
    helpDefault: 'Ensure WebDAV Server is installed on DSM with HTTPS enabled (default port 5006).',
  },
  {
    id: 'alist',
    nameKey: 'settings.sync.preset_alist',
    defaultName: 'Alist / OpenList',
    badgeKey: 'settings.sync.badge_multicloud',
    defaultBadge: 'Multi-cloud',
    defaultServerUrl: 'https://your-alist.example.com/dav/',
    defaultRemoteRoot: 'Sona',
    usernamePlaceholder: 'admin',
    helpKey: 'settings.sync.preset_help_alist',
    helpDefault: 'Verify WebDAV policy is enabled in Alist and connect with an authorized account.',
  },
  {
    id: 'custom',
    nameKey: 'settings.sync.preset_custom',
    defaultName: 'Custom WebDAV',
    badgeKey: 'settings.sync.badge_custom',
    defaultBadge: 'Custom',
    defaultServerUrl: '',
    defaultRemoteRoot: 'Sona',
    usernamePlaceholder: 'username',
    helpKey: 'settings.sync.preset_help_custom',
    helpDefault: 'Supports any RFC 4918 compliant WebDAV server over HTTPS.',
  },
] as const;

export function detectProviderPresetId(serverUrl: string): WellKnownSyncProviderId {
  const normalized = serverUrl.trim().toLowerCase();
  if (normalized.includes('jianguoyun.com')) return 'nutstore';
  if (normalized.includes('teracloud.jp') || normalized.includes('infinicloud')) return 'infinicloud';
  if (normalized.includes('remote.php/dav') || normalized.includes('nextcloud') || normalized.includes('owncloud')) return 'nextcloud';
  if (normalized.includes(':5006') || normalized.includes('synology')) return 'synology';
  if (normalized.includes('/dav') && normalized.includes('alist')) return 'alist';
  return 'custom';
}
