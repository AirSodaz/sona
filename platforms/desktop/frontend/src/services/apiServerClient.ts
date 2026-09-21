import type { TranscriptSegment } from '../types/transcript';

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
      const savedUrl = localStorage.getItem('sona_api_server_url');
      if (savedUrl?.trim()) {
        this.baseUrl = savedUrl.trim().replace(/\/+$/, '');
      } else if (
        (window.location.protocol === 'http:' || window.location.protocol === 'https:') &&
        window.location.port !== '1420' &&
        window.location.port !== '5173' &&
        window.location.port !== '5174' &&
        window.location.port !== '4173'
      ) {
        this.baseUrl = window.location.origin;
      } else {
        this.baseUrl = 'http://127.0.0.1:14200';
      }
      const savedKey = localStorage.getItem('sona_api_server_key');
      this.apiKey = savedKey?.trim() || '';
    } else {
      this.baseUrl = 'http://127.0.0.1:14200';
      this.apiKey = '';
    }
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }

  setBaseUrl(url: string): void {
    this.baseUrl = url.trim().replace(/\/+$/, '');
    if (typeof window !== 'undefined') {
      if (this.baseUrl) {
        localStorage.setItem('sona_api_server_url', this.baseUrl);
      } else {
        localStorage.removeItem('sona_api_server_url');
      }
    }
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
  async listJobs(options?: {
    status?: string;
    limit?: number;
    offset?: number;
    full?: boolean;
  }): Promise<Record<string, JobStatusType>> {
    const params = new URLSearchParams();
    if (options?.status) params.set('status', options.status);
    if (options?.limit !== undefined) params.set('limit', String(options.limit));
    if (options?.offset !== undefined) params.set('offset', String(options.offset));
    if (options?.full !== undefined) params.set('full', String(options.full));

    const query = params.toString();
    const url = `${this.baseUrl}/v1/transcriptions/jobs${query ? `?${query}` : ''}`;
    const res = await fetch(url, {
      headers: this.getHeaders(),
    });
    if (!res.ok) {
      throw new Error(`Failed to list jobs (${res.status})`);
    }
    return res.json();
  }

  async deleteJob(jobId: string): Promise<void> {
    const res = await fetch(`${this.baseUrl}/v1/transcriptions/${jobId}`, {
      method: 'DELETE',
      headers: this.getHeaders(),
    });
    if (!res.ok && res.status !== 404) {
      throw new Error(`Failed to delete job (${res.status})`);
    }
  }

  async exportJob(
    jobId: string,
    format: 'srt' | 'vtt' | 'txt' | 'md' | 'json' = 'srt',
    mode: 'original' | 'translation' | 'bilingual' = 'original'
  ): Promise<string> {
    const params = new URLSearchParams({ format, mode });
    const res = await fetch(
      `${this.baseUrl}/v1/transcriptions/${jobId}/export?${params.toString()}`,
      {
        headers: this.getHeaders(),
      }
    );
    if (!res.ok) {
      throw new Error(`Failed to export job (${res.status})`);
    }
    return res.text();
  }

  async polishSegments(
    segments: TranscriptSegment[],
    config?: unknown
  ): Promise<TranscriptSegment[]> {
    const res = await fetch(`${this.baseUrl}/v1/llm/polish`, {
      method: 'POST',
      headers: this.getHeaders({ 'Content-Type': 'application/json' }),
      body: JSON.stringify({ segments, config }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Failed to polish segments (${res.status}): ${text}`);
    }
    const data = await res.json();
    return data.segments;
  }

  async translateSegments(
    segments: TranscriptSegment[],
    targetLanguage: string,
    config?: unknown
  ): Promise<TranscriptSegment[]> {
    const res = await fetch(`${this.baseUrl}/v1/llm/translate`, {
      method: 'POST',
      headers: this.getHeaders({ 'Content-Type': 'application/json' }),
      body: JSON.stringify({ segments, target_language: targetLanguage, config }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Failed to translate segments (${res.status}): ${text}`);
    }
    const data = await res.json();
    return data.segments;
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
