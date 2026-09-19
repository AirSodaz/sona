import type { AppConfig, AsrScenario } from '../types/config';
import type {
  SpeakerProcessingConfig,
  SpeakerProfile,
  SpeakerProfileSample,
} from '../types/speaker';
import { normalizeSpeakerProfiles } from '../types/speakerNormalization';
import type { TranscriptSegment } from '../types/transcript';
import {
  getScenarioSpeakerEmbeddingModelPath,
  getScenarioSpeakerSegmentationModelPath,
  type ScenarioModelPathConfig,
} from '../utils/scenarioModels';
import {
  annotateSpeakerSegmentsFromFile,
  enrollSpeakerProfileSampleFromAudio,
  importSpeakerProfileSample,
} from './tauri/speaker';

type SpeakerConfigInput = Pick<AppConfig, 'speakerProfiles' | 'speakerDiarizationSensitivity'> &
  Partial<ScenarioModelPathConfig>;

export interface SpeakerServicePorts {
  annotateSpeakerSegmentsFromFile: typeof annotateSpeakerSegmentsFromFile;
  importSpeakerProfileSample: typeof importSpeakerProfileSample;
  enrollSpeakerProfileSampleFromAudio: typeof enrollSpeakerProfileSampleFromAudio;
}

export class SpeakerService {
  constructor(private readonly ports: SpeakerServicePorts) {}

  resolveEffectiveProfiles(
    profiles: SpeakerProfile[] | undefined,
    projectId?: string | null
  ): SpeakerProfile[] {
    const normalized = normalizeSpeakerProfiles(profiles);
    return normalized.filter((profile) => {
      if (!profile.enabled) return false;
      const scope = profile.scope ?? 'global';
      if (scope === 'global') return true;
      if (scope === 'project' && projectId) {
        return (profile.projectIds ?? []).includes(projectId);
      }
      return false;
    });
  }

  isConfigured(config: SpeakerConfigInput, scenario: AsrScenario): boolean {
    const embeddingPath = getScenarioSpeakerEmbeddingModelPath(config, scenario);
    if (!embeddingPath) {
      return false;
    }
    if (scenario === 'batch') {
      return Boolean(getScenarioSpeakerSegmentationModelPath(config, scenario));
    }
    return true;
  }

  buildProcessingConfig(
    config: SpeakerConfigInput,
    scenario: AsrScenario,
    projectId?: string | null
  ): SpeakerProcessingConfig | null {
    const embeddingModelPath = getScenarioSpeakerEmbeddingModelPath(config, scenario);
    if (!embeddingModelPath) {
      return null;
    }
    const segmentationModelPath = getScenarioSpeakerSegmentationModelPath(config, scenario);
    if (scenario === 'batch' && !segmentationModelPath) {
      return null;
    }

    return {
      speakerSegmentationModelPath: segmentationModelPath || undefined,
      speakerEmbeddingModelPath: embeddingModelPath,
      speakerProfiles: this.resolveEffectiveProfiles(config.speakerProfiles, projectId),
      sensitivity: config.speakerDiarizationSensitivity ?? 'balanced',
    };
  }

  async annotateSegmentsForFile(
    filePath: string,
    segments: TranscriptSegment[],
    config: SpeakerConfigInput,
    scenario: AsrScenario = 'live',
    projectId?: string | null
  ): Promise<TranscriptSegment[]> {
    if (!filePath || segments.length === 0) {
      return segments;
    }

    const speakerProcessing = this.buildProcessingConfig(config, scenario, projectId);
    if (
      !speakerProcessing?.speakerSegmentationModelPath ||
      !speakerProcessing?.speakerEmbeddingModelPath
    ) {
      return segments;
    }
    return this.ports.annotateSpeakerSegmentsFromFile(filePath, segments, speakerProcessing);
  }

  async importProfileSample(
    profileId: string,
    sourcePath: string,
    sourceName?: string
  ): Promise<SpeakerProfileSample> {
    return this.ports.importSpeakerProfileSample(profileId, sourcePath, sourceName);
  }

  async enrollProfileSampleFromAudio(
    profileId: string,
    sourceAudioPath: string,
    startSeconds: number,
    endSeconds: number,
    sampleName?: string
  ): Promise<SpeakerProfileSample> {
    return this.ports.enrollSpeakerProfileSampleFromAudio(
      profileId,
      sourceAudioPath,
      startSeconds,
      endSeconds,
      sampleName
    );
  }
}

export function createSpeakerService(ports: SpeakerServicePorts): SpeakerService {
  return new SpeakerService(ports);
}

export const speakerService = createSpeakerService({
  annotateSpeakerSegmentsFromFile,
  importSpeakerProfileSample,
  enrollSpeakerProfileSampleFromAudio,
});
