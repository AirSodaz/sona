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

export interface ApiServerInfo {
  gpuAvailable: boolean;
  models: ApiServerModelInfo[];
  onlineAsrProviders?: string[];
}

export type JobStatusType =
  | 'Pending'
  | 'Processing'
  | { Completed: TranscriptSegment[] }
  | { Failed: string };

class ApiServerClient {
  private baseUrl: string;

  constructor() {
    // If running in browser hosted by api_server (e.g. port 14200), default to origin.
    // If running in vite dev (port 1420/5173), default to http://127.0.0.1:14200.
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
    } else {
      this.baseUrl = 'http://127.0.0.1:14200';
    }
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }

  setBaseUrl(url: string): void {
    this.baseUrl = url.replace(/\/+$/, '');
  }

  async checkHealth(): Promise<ApiServerHealth> {
    const res = await fetch(`${this.baseUrl}/health`);
    if (!res.ok) {
      throw new Error(`Health check failed: HTTP ${res.status}`);
    }
    return res.json();
  }

  async getInfo(): Promise<ApiServerInfo> {
    const res = await fetch(`${this.baseUrl}/info`);
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
    const res = await fetch(`${this.baseUrl}/v1/transcriptions/${jobId}`);
    if (!res.ok) {
      throw new Error(`Failed to fetch job status (${res.status})`);
    }
    return res.json();
  }

  getAudioUrl(jobId: string): string {
    return `${this.baseUrl}/v1/transcriptions/${jobId}/audio`;
  }

  async polish(segments: TranscriptSegment[]): Promise<TranscriptSegment[]> {
    const res = await fetch(`${this.baseUrl}/v1/llm/polish`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ segments }),
    });

    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Polish failed (${res.status}): ${text}`);
    }

    return res.json();
  }

  async translate(
    segments: TranscriptSegment[],
    targetLanguage: string
  ): Promise<TranscriptSegment[]> {
    const res = await fetch(`${this.baseUrl}/v1/llm/translate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ segments, target_language: targetLanguage }),
    });

    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Translate failed (${res.status}): ${text}`);
    }

    return res.json();
  }
}

export const apiServerClient = new ApiServerClient();
