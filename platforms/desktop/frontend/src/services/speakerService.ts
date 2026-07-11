import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import {
  normalizeSpeakerProfiles,
  type SpeakerProcessingConfig,
  type SpeakerProfileSample,
} from '../types/speaker';
import {
  annotateSpeakerSegmentsFromFile,
  importSpeakerProfileSample,
} from './tauri/speaker';

type SpeakerConfigInput = Pick<
  AppConfig,
  'speakerSegmentationModelPath' | 'speakerEmbeddingModelPath' | 'speakerProfiles'
>;

export interface SpeakerServicePorts {
  annotateSpeakerSegmentsFromFile: typeof annotateSpeakerSegmentsFromFile;
  importSpeakerProfileSample: typeof importSpeakerProfileSample;
}

export class SpeakerService {
  constructor(private readonly ports: SpeakerServicePorts) {}

  isConfigured(config: SpeakerConfigInput): boolean {
    return Boolean(
      config.speakerSegmentationModelPath?.trim()
      && config.speakerEmbeddingModelPath?.trim(),
    );
  }

  buildProcessingConfig(config: SpeakerConfigInput): SpeakerProcessingConfig | null {
    const segmentationModelPath = config.speakerSegmentationModelPath?.trim();
    const embeddingModelPath = config.speakerEmbeddingModelPath?.trim();
    if (!segmentationModelPath || !embeddingModelPath) {
      return null;
    }

    return {
      speakerSegmentationModelPath: segmentationModelPath,
      speakerEmbeddingModelPath: embeddingModelPath,
      speakerProfiles: normalizeSpeakerProfiles(config.speakerProfiles),
    };
  }

  async annotateSegmentsForFile(
    filePath: string,
    segments: TranscriptSegment[],
    config: SpeakerConfigInput,
  ): Promise<TranscriptSegment[]> {
    if (!filePath || segments.length === 0) {
      return segments;
    }

    const speakerProcessing = this.buildProcessingConfig(config);
    if (!speakerProcessing) {
      return segments;
    }

    return this.ports.annotateSpeakerSegmentsFromFile(filePath, segments, speakerProcessing);
  }

  async importProfileSample(
    profileId: string,
    sourcePath: string,
    sourceName?: string,
  ): Promise<SpeakerProfileSample> {
    return this.ports.importSpeakerProfileSample(profileId, sourcePath, sourceName);
  }
}

export function createSpeakerService(ports: SpeakerServicePorts): SpeakerService {
  return new SpeakerService(ports);
}

export const speakerService = createSpeakerService({
  annotateSpeakerSegmentsFromFile,
  importSpeakerProfileSample,
});
