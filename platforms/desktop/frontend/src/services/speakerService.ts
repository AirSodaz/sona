import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import type { AppConfig, AsrScenario } from '../types/config';
import type { SpeakerProcessingConfig, SpeakerProfileSample } from '../types/speaker';
import { normalizeSpeakerProfiles } from '../types/speakerNormalization';
import type { TranscriptSegment } from '../types/transcript';
import {
  getScenarioSpeakerEmbeddingModelPath,
  getScenarioSpeakerSegmentationModelPath,
  type ScenarioModelPathConfig,
} from '../utils/scenarioModels';
import { historyService } from './historyService';
import {
  annotateSpeakerSegmentsFromFile,
  enrollSpeakerProfileSampleFromAudio,
  importSpeakerProfileSample,
} from './tauri/speaker';

type SpeakerConfigInput = Pick<AppConfig, 'speakerProfiles'> & Partial<ScenarioModelPathConfig>;

export interface SpeakerServicePorts {
  annotateSpeakerSegmentsFromFile: typeof annotateSpeakerSegmentsFromFile;
  importSpeakerProfileSample: typeof importSpeakerProfileSample;
  enrollSpeakerProfileSampleFromAudio: typeof enrollSpeakerProfileSampleFromAudio;
}

export class SpeakerService {
  constructor(private readonly ports: SpeakerServicePorts) {}

  isConfigured(config: SpeakerConfigInput, scenario: AsrScenario): boolean {
    return Boolean(
      getScenarioSpeakerSegmentationModelPath(config, scenario) &&
        getScenarioSpeakerEmbeddingModelPath(config, scenario)
    );
  }

  buildProcessingConfig(
    config: SpeakerConfigInput,
    scenario: AsrScenario
  ): SpeakerProcessingConfig | null {
    const segmentationModelPath = getScenarioSpeakerSegmentationModelPath(config, scenario);
    const embeddingModelPath = getScenarioSpeakerEmbeddingModelPath(config, scenario);
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
    scenario: AsrScenario = 'live'
  ): Promise<TranscriptSegment[]> {
    if (!filePath || segments.length === 0) {
      return segments;
    }

    const speakerProcessing = this.buildProcessingConfig(config, scenario);
    if (!speakerProcessing) {
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

export async function resolveCurrentSessionAudioPath(): Promise<string | null> {
  const session = useTranscriptSessionStore.getState();
  if (session.sourceHistoryId) {
    const path = await historyService.getAudioAbsolutePath(session.sourceHistoryId);
    if (path) {
      return path;
    }
  }

  const activeBatchItem = useBatchQueueStore
    .getState()
    .items.find((item) => item.id === useBatchQueueStore.getState().activeItemId);
  if (activeBatchItem?.filePath) {
    return activeBatchItem.filePath;
  }

  return null;
}
