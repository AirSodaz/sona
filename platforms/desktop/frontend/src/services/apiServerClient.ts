import type { TranscriptSegment } from '../types/transcript';

export interface ApiServerHealth {
  status: string;
  uptimeSeconds: number;
  availableCacheSpaceBytes: number;
}

export interface ApiServerModelInfo {
  id: string;
  name: string;
  description?: string;
  languages?: string[];
  installed?: boolean;
}

export interface OnlineAsrProviderInfo {
  id: string;
  languages: string[];
  configured: boolean;
  supportsBatch: boolean;
  supportsStreaming: boolean;
}

export interface ApiServerInfo {
  platform?: string;
  gpuAvailable: boolean;
  models: (string | ApiServerModelInfo)[];
  vadInstalled?: boolean;
  punctuationInstalled?: boolean;
  onlineAsrProviders?: OnlineAsrProviderInfo[];
}

export type JobStatusType =
  | 'Pending'
  | 'Processing'
  | { Completed: TranscriptSegment[] }
  | { Failed: string };

class ApiServerClient {
  private baseUrl: string;
  private apiKey: string;

  constructor() {
    if (typeof window !== 'undefined') {
      const port = window.location.port;
      if (
        port === '14200' ||
        (window.location.hostname !== 'localhost' && window.location.hostname !== '127.0.0.1')
      ) {
        this.baseUrl = window.location.origin;
      } else {
        this.baseUrl = 'http://127.0.0.1:14200';
      }
      this.apiKey = localStorage.getItem('sona_api_server_key') || '';
    } else {
      this.baseUrl = 'http://127.0.0.1:14200';
      this.apiKey = '';
    }
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }

  setBaseUrl(url: string): void {
    this.baseUrl = url.replace(/\/+$/, '');
  }

  getApiKey(): string {
    return this.apiKey;
  }

  setApiKey(key: string): void {
    this.apiKey = key.trim();
    if (typeof window !== 'undefined') {
      if (this.apiKey) {
        localStorage.setItem('sona_api_server_key', this.apiKey);
      } else {
        localStorage.removeItem('sona_api_server_key');
      }
    }
  }

  private getHeaders(extra?: Record<string, string>): HeadersInit {
    const headers: Record<string, string> = { ...extra };
    if (this.apiKey) {
      headers.Authorization = `Bearer ${this.apiKey}`;
    }
    return headers;
  }

  async checkHealth(): Promise<ApiServerHealth> {
    const res = await fetch(`${this.baseUrl}/health`);
    if (!res.ok) {
      throw new Error(`Health check failed: HTTP ${res.status}`);
    }
    return res.json();
  }

  async getInfo(): Promise<ApiServerInfo> {
    const res = await fetch(`${this.baseUrl}/info`, {
      headers: this.getHeaders(),
    });
    if (!res.ok) {
      throw new Error(`Failed to fetch info: HTTP ${res.status}`);
    }
    return res.json();
  }

  async transcribe(
    file: File,
    options: {
      modelId: string;
      language?: string;
      hotwords?: string;
    }
  ): Promise<string> {
    const formData = new FormData();
    formData.append('file', file);
    formData.append('model_id', options.modelId);
    if (options.language) {
      formData.append('language', options.language);
    }
    if (options.hotwords) {
      formData.append('hotwords', options.hotwords);
    }

    const res = await fetch(`${this.baseUrl}/v1/transcriptions`, {
      method: 'POST',
      headers: this.getHeaders(),
      body: formData,
    });

    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Transcription request failed (${res.status}): ${text}`);
    }

    const data = await res.json();
    return data.job_id;
  }

  async getJobStatus(jobId: string): Promise<JobStatusType> {
    const res = await fetch(`${this.baseUrl}/v1/transcriptions/${jobId}`, {
      headers: this.getHeaders(),
    });
    if (!res.ok) {
      throw new Error(`Failed to fetch job status (${res.status})`);
    }
    return res.json();
  }

  getAudioUrl(jobId: string): string {
    const url = `${this.baseUrl}/v1/transcriptions/${jobId}/audio`;
    if (this.apiKey) {
      return `${url}?token=${encodeURIComponent(this.apiKey)}`;
    }
    return url;
  }
}

export const apiServerClient = new ApiServerClient();
