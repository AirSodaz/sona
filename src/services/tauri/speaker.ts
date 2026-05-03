import type { SpeakerProfileSample, SpeakerProcessingConfig } from '../../types/speaker';
import type { TranscriptSegment } from '../../types/transcript';
import type {
  ApplySpeakerProfileToGroupRequest,
  SpeakerCorrectionResponse,
  SpeakerGroupRequest,
} from '../speakerCorrectionService';
import type {
  SpeakerReviewFilter,
  SpeakerReviewSnapshot,
} from '../speakerReviewService';
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

export async function buildSpeakerReviewSnapshot(
  segments: TranscriptSegment[],
  activeFilter: SpeakerReviewFilter,
): Promise<SpeakerReviewSnapshot> {
  return invokeTauri(TauriCommand.speaker.buildReviewSnapshot, {
    segments,
    activeFilter,
  });
}

export async function applySpeakerProfileToGroup(
  request: ApplySpeakerProfileToGroupRequest,
): Promise<SpeakerCorrectionResponse> {
  return invokeTauri(TauriCommand.speaker.applyProfileToGroup, { request });
}

export async function resetSpeakerGroupToAnonymous(
  request: SpeakerGroupRequest,
): Promise<SpeakerCorrectionResponse> {
  return invokeTauri(TauriCommand.speaker.resetGroupToAnonymous, { request });
}

export async function confirmSpeakerGroupReview(
  request: SpeakerGroupRequest,
): Promise<SpeakerCorrectionResponse> {
  return invokeTauri(TauriCommand.speaker.confirmGroupReview, { request });
}
