import type { SpeakerProfileSample, SpeakerProcessingConfig } from '../../types/speaker';
import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function annotateSpeakerSegmentsFromFile(
  filePath: string,
  segments: TranscriptSegment[],
  speakerProcessing: SpeakerProcessingConfig,
): Promise<TranscriptSegment[]> {
  return invokeTauri(TauriCommand.speaker.annotateSegmentsFromFile, {
    filePath,
    segments,
    speakerProcessing,
  });
}

export async function importSpeakerProfileSample(
  profileId: string,
  sourcePath: string,
  sourceName?: string,
): Promise<SpeakerProfileSample> {
  return invokeTauri(TauriCommand.speaker.importProfileSample, {
    profileId,
    sourcePath,
    sourceName: sourceName || null,
  });
}
