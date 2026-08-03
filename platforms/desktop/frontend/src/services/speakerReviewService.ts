import type {
  SpeakerReviewFilter,
  SpeakerReviewSnapshot,
} from '../types/speaker';
import type { TranscriptSegment } from '../types/transcript';
import { buildSpeakerReviewSnapshot as buildSpeakerReviewSnapshotFromRust } from './tauri/speaker';

export type {
  SpeakerReviewCandidate,
  SpeakerReviewCounts,
  SpeakerReviewFilter,
  SpeakerReviewFilterOption,
  SpeakerReviewGroup,
  SpeakerReviewRiskReason,
  SpeakerReviewSegmentPreview,
  SpeakerReviewSnapshot,
  SpeakerReviewStatus,
} from '../types/speaker';

export interface SpeakerReviewServicePorts {
  buildSpeakerReviewSnapshotFromRust: typeof buildSpeakerReviewSnapshotFromRust;
}

export class SpeakerReviewService {
  constructor(private readonly ports: SpeakerReviewServicePorts) {}

  buildSpeakerReviewSnapshot = async (
    segments: TranscriptSegment[],
    activeFilter: SpeakerReviewFilter,
  ): Promise<SpeakerReviewSnapshot> => {
    return this.ports.buildSpeakerReviewSnapshotFromRust(segments, activeFilter);
  }
}

export function createSpeakerReviewService(ports: SpeakerReviewServicePorts): SpeakerReviewService {
  return new SpeakerReviewService(ports);
}

export const speakerReviewService = createSpeakerReviewService({
  buildSpeakerReviewSnapshotFromRust,
});

export const {
  buildSpeakerReviewSnapshot,
} = speakerReviewService;
