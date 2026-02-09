/**
 * Core data structure for transcript segments.
 * This is the "source of truth" for all transcription data.
 */
export interface TranscriptSegment {
  /** Unique identifier (UUID). */
  id: string;
  /** Start time in seconds. */
  start: number;
  /** End time in seconds. */
  end: number;
  /** The transcribed text content. */
  text: string;
  /** True if the segment is finalized (not a partial/in-progress result). */
  isFinal: boolean;
  /** List of tokens in the segment. */
  tokens?: string[];
  /** Start time of each token. */
  timestamps?: number[];
  /** Duration of each token. */
  durations?: number[];
}

/**
 * Application operation mode.
 */
export type AppMode = 'live' | 'batch' | 'history';

/**
 * Processing status for batch imports.
 */
export type ProcessingStatus = 'idle' | 'loading' | 'processing' | 'complete' | 'error';

/**
 * Configuration for the application.
 */
export interface AppConfig {
  /** Path to streaming model (e.g. sherpa-onnx streaming zipformer). */
  streamingModelPath: string;
  /** Path to offline model (e.g. sherpa-onnx offline paraformer/sensevoice). */
  offlineModelPath: string;
  /** Selected language for transcription. */
  language: string;
  /** Application UI language preference. */
  appLanguage: 'auto' | 'en' | 'zh';
  /** IDs of enabled ITN models. */
  enabledITNModels?: string[];
  /** Order of ITN models (IDs) for sorting. */
  itnRulesOrder?: string[];
  /** DEPRECATED: Enable Inverse Text Normalization. */
  enableITN?: boolean;
  /** Path to punctuation model. */
  punctuationModelPath?: string;
  /** Application theme preference. */
  theme?: 'auto' | 'light' | 'dark';
  /** Font preference. */
  font?: 'system' | 'serif' | 'sans' | 'mono' | 'arial' | 'georgia';
  /** Path to VAD model. */
  vadModelPath?: string;
  /** VAD buffer size in seconds. Default: 5. */
  vadBufferSize?: number;
}
