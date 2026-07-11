export type RuntimePathStatusKind = 'file' | 'directory' | 'missing' | 'unknown';

export interface RuntimePathStatus {
  path: string;
  kind: RuntimePathStatusKind;
  error?: string | null;
}

export interface RuntimeEnvironmentStatus {
  ffmpegPath: string;
  ffmpegExists: boolean;
  logDirPath: string;
}

export interface AsrModelLoadMetric {
  occurredAtMs: number;
  instanceId: string;
  modelPath: string;
  modelType: string;
  recognizerKind: string;
  numThreads: number;
  reusedFromPool: boolean;
  loadMs: number;
  rssBeforeMb: number | null;
  rssAfterMb: number | null;
  rssDeltaMb: number | null;
  processRssMb: number | null;
}

export interface AsrInferenceMetric {
  occurredAtMs: number;
  source: 'live' | 'batch' | string;
  instanceId: string | null;
  stage: string;
  isFinal: boolean;
  audioDurationMs: number;
  bufferedSamples: number;
  audioExtractMs: number | null;
  decodeMs: number;
  emitLatencyMs: number | null;
  totalMs: number | null;
  rtf: number | null;
  segmentCount: number | null;
  processRssMb: number | null;
}

export interface AsrRuntimeMetricsSnapshot {
  modelLoad: AsrModelLoadMetric | null;
  liveInference: AsrInferenceMetric | null;
  batchInference: AsrInferenceMetric | null;
}
