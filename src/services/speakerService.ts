import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import {
  normalizeSpeakerProfiles,
  type SpeakerProcessingConfig,
  type SpeakerProfileSample,
} from '../types/speaker';

type SpeakerConfigInput = Pick<
  AppConfig,
  'speakerSegmentationModelPath' | 'speakerEmbeddingModelPath' | 'speakerProfiles'
>;

class SpeakerService {
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

    return invoke<TranscriptSegment[]>('annotate_speaker_segments_from_file', {
      filePath,
      segments,
      speakerProcessing,
    });
  }

  async importProfileSample(
    profileId: string,
    sourcePath: string,
    sourceName?: string,
  ): Promise<SpeakerProfileSample> {
    return invoke<SpeakerProfileSample>('import_speaker_profile_sample', {
      profileId,
      sourcePath,
      sourceName: sourceName || null,
    });
  }
}

export const speakerService = new SpeakerService();
