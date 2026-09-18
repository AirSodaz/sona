export type SyncProtocolType = 'webdav' | 's3';

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
    helpDefault:
      'Generate an app password in Nutstore: Account Info -> Security -> Third-party apps.',
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
    helpDefault:
      'Generate an app password in Nextcloud: Personal Settings -> Security -> Devices & Sessions.',
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
    helpDefault: 'Supports any RFC 4918 compliant WebDAV server (HTTPS or local/LAN HTTP).',
  },
] as const;

export function detectProviderPresetId(serverUrl: string): WellKnownSyncProviderId {
  const normalized = serverUrl.trim().toLowerCase();
  if (normalized.includes('jianguoyun.com')) return 'nutstore';
  if (normalized.includes('teracloud.jp') || normalized.includes('infinicloud'))
    return 'infinicloud';
  if (
    normalized.includes('remote.php/dav') ||
    normalized.includes('nextcloud') ||
    normalized.includes('owncloud')
  )
    return 'nextcloud';
  if (normalized.includes(':5006') || normalized.includes('synology')) return 'synology';
  if (normalized.includes('/dav') && normalized.includes('alist')) return 'alist';
  return 'custom';
}

export type WellKnownS3ProviderId =
  | 'cloudflare-r2'
  | 'aws-s3'
  | 'aliyun-oss'
  | 'tencent-cos'
  | 'minio'
  | 's3-custom';

export interface S3ProviderPreset {
  id: WellKnownS3ProviderId;
  nameKey: string;
  defaultName: string;
  badgeKey: string;
  defaultBadge: string;
  defaultEndpoint: string;
  defaultRegion: string;
  defaultBucket: string;
  defaultRemoteRoot: string;
  defaultForcePathStyle: boolean;
  accessKeyPlaceholder: string;
  helpKey: string;
  helpDefault: string;
  authDocUrl?: string;
}

export const S3_PROVIDER_PRESETS: readonly S3ProviderPreset[] = [
  {
    id: 'cloudflare-r2',
    nameKey: 'settings.sync.s3_preset_r2',
    defaultName: 'Cloudflare R2',
    badgeKey: 'settings.sync.s3_badge_r2',
    defaultBadge: 'Zero Egress',
    defaultEndpoint: 'https://<account-id>.r2.cloudflarestorage.com',
    defaultRegion: 'auto',
    defaultBucket: 'sona-sync',
    defaultRemoteRoot: 'sona',
    defaultForcePathStyle: false,
    accessKeyPlaceholder: 'Cloudflare R2 Access Key ID',
    helpKey: 'settings.sync.s3_preset_help_r2',
    helpDefault:
      'In Cloudflare Dashboard -> R2 -> Manage R2 API Tokens, create a token with Object Read & Write permissions.',
    authDocUrl: 'https://developers.cloudflare.com/r2/api/s3/tokens/',
  },
  {
    id: 'aws-s3',
    nameKey: 'settings.sync.s3_preset_aws',
    defaultName: 'AWS S3',
    badgeKey: 'settings.sync.s3_badge_standard',
    defaultBadge: 'AWS',
    defaultEndpoint: 'https://s3.us-east-1.amazonaws.com',
    defaultRegion: 'us-east-1',
    defaultBucket: 'sona-sync',
    defaultRemoteRoot: 'sona',
    defaultForcePathStyle: false,
    accessKeyPlaceholder: 'AKIA...',
    helpKey: 'settings.sync.s3_preset_help_aws',
    helpDefault:
      'Provide IAM Access Key with s3:GetObject, s3:PutObject, s3:DeleteObject, and s3:ListBucket permissions.',
    authDocUrl: 'https://docs.aws.amazon.com/AmazonS3/latest/userguide/security-iam.html',
  },
  {
    id: 'aliyun-oss',
    nameKey: 'settings.sync.s3_preset_oss',
    defaultName: 'Aliyun OSS / 阿里云',
    badgeKey: 'settings.sync.s3_badge_oss',
    defaultBadge: 'OSS',
    defaultEndpoint: 'https://oss-cn-hangzhou.aliyuncs.com',
    defaultRegion: 'oss-cn-hangzhou',
    defaultBucket: 'sona-sync',
    defaultRemoteRoot: 'sona',
    defaultForcePathStyle: false,
    accessKeyPlaceholder: 'LTAI...',
    helpKey: 'settings.sync.s3_preset_help_oss',
    helpDefault:
      'Create AccessKey in Aliyun RAM console and ensure the user has AliyunOSSFullAccess or custom bucket permissions.',
  },
  {
    id: 'tencent-cos',
    nameKey: 'settings.sync.s3_preset_cos',
    defaultName: 'Tencent COS / 腾讯云',
    badgeKey: 'settings.sync.s3_badge_cos',
    defaultBadge: 'COS',
    defaultEndpoint: 'https://cos.ap-shanghai.myqcloud.com',
    defaultRegion: 'ap-shanghai',
    defaultBucket: 'sona-sync',
    defaultRemoteRoot: 'sona',
    defaultForcePathStyle: false,
    accessKeyPlaceholder: 'AKID...',
    helpKey: 'settings.sync.s3_preset_help_cos',
    helpDefault:
      'Obtain SecretId and SecretKey from Tencent Cloud CAM console with COS bucket read/write permissions.',
  },
  {
    id: 'minio',
    nameKey: 'settings.sync.s3_preset_minio',
    defaultName: 'MinIO',
    badgeKey: 'settings.sync.badge_selfhosted',
    defaultBadge: 'Self-hosted',
    defaultEndpoint: 'http://localhost:9000',
    defaultRegion: 'us-east-1',
    defaultBucket: 'sona-sync',
    defaultRemoteRoot: 'sona',
    defaultForcePathStyle: true,
    accessKeyPlaceholder: 'minioadmin',
    helpKey: 'settings.sync.s3_preset_help_minio',
    helpDefault:
      'Self-hosted MinIO instance. Path-style addressing is enabled by default for localhost and private IP addresses.',
  },
  {
    id: 's3-custom',
    nameKey: 'settings.sync.s3_preset_custom',
    defaultName: 'Custom S3 / 自定义',
    badgeKey: 'settings.sync.badge_custom',
    defaultBadge: 'Custom',
    defaultEndpoint: 'https://s3.example.com',
    defaultRegion: 'us-east-1',
    defaultBucket: 'sona-sync',
    defaultRemoteRoot: 'sona',
    defaultForcePathStyle: false,
    accessKeyPlaceholder: 'Access Key ID',
    helpKey: 'settings.sync.s3_preset_help_custom',
    helpDefault:
      'Compatible with any S3 API provider (Backblaze B2, Wasabi, Supabase, Garage, etc.).',
  },
] as const;

export function detectS3ProviderPresetId(endpoint: string): WellKnownS3ProviderId {
  const normalized = endpoint.trim().toLowerCase();
  if (normalized.includes('r2.cloudflarestorage.com')) return 'cloudflare-r2';
  if (normalized.includes('amazonaws.com')) return 'aws-s3';
  if (normalized.includes('aliyuncs.com')) return 'aliyun-oss';
  if (normalized.includes('myqcloud.com')) return 'tencent-cos';
  if (normalized.includes(':9000') || normalized.includes('minio')) return 'minio';
  return 's3-custom';
}
