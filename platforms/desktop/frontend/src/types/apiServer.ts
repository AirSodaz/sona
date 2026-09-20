import type { TranscriptSegment } from './transcript';

export interface ApiServerHealth {
  status: string;
  uptime: number;
  activeJobs: number;
  pendingJobs: number;
  cacheSpaceBytes: number;
}
export interface ApiServerModelInfo {
  id: string;
  name: string;
  description?: string;
  languages?: string[];
  languageMode?: string;
}

export interface ApiServerInfo {
  platform: string;
  gpuAvailable: boolean;
  models: (string | ApiServerModelInfo)[];
  vadInstalled: boolean;
  punctuationInstalled: boolean;
  onlineAsrProviders: ApiServerOnlineAsrProviderInfo[];
}

export type ApiServerJobStatus =
  | 'Pending'
  | 'Processing'
  | { Completed: TranscriptSegment[] }
  | { Failed: string };

export interface ApiServerDashboardSnapshot {
  health: ApiServerHealth;
  info: ApiServerInfo;
  jobs: Record<string, ApiServerJobStatus>;
}
