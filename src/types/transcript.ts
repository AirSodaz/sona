/**
 * Core data structure for transcript segments.
 * This is the "source of truth" for all transcription data.
 */
export interface TranscriptSegment {
  /** Unique identifier (UUID) */
  id: string;
  /** Start time in seconds */
  start: number;
  /** End time in seconds */
  end: number;
  /** The transcribed text content */
  text: string;
  /** True if the segment is finalized (not a partial/in-progress result) */
  isFinal: boolean;
  /** List of tokens in the segment */
  tokens?: string[];
  /** Start time of each token */
  timestamps?: number[];
}

/**
 * Application operation mode
 */
export type AppMode = 'live' | 'batch';

/**
 * Processing status for batch imports
 */
export type ProcessingStatus = 'idle' | 'loading' | 'processing' | 'complete' | 'error';

/**
 * Configuration for the application
 */
export interface AppConfig {
  /** Path to sherpa-onnx model directory */
  modelPath: string;
  /** Selected language for transcription */
  /** Selected language for transcription */
  language: string;
  /** Application UI language preference */
  appLanguage: 'auto' | 'en' | 'zh';
  /** Enable Inverse Text Normalization (e.g. number conversion) */
  enableITN?: boolean;
}
