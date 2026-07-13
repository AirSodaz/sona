import type { TranscriptSegment } from './transcript';

export interface ApiServerHealth {
  status: string;
  uptime: number;
  activeJobs: number;
  pendingJobs: number;
  cacheSpaceBytes: number;
}

export interface ApiServerOnlineAsrProviderInfo {
  id: string;
  configured: boolean;
  supportsBatch: boolean;
  supportsStreaming: boolean;
}

export interface ApiServerInfo {
  platform: string;
  gpuAvailable: boolean;
  models: string[];
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
