import type { SpeakerProfile } from './speaker';
import type { TranscriptSegment } from './transcript';

export interface ApplySpeakerProfileToGroupRequest {
  segments: TranscriptSegment[];
  groupId: string;
  targetProfileId: string;
  speakerProfiles: SpeakerProfile[];
  enabledSpeakerProfileIds: string[];
}

export interface SpeakerGroupRequest {
  segments: TranscriptSegment[];
  groupId: string;
}

export interface SpeakerCorrectionResponse {
  segments: TranscriptSegment[];
  enabledSpeakerProfileIds?: string[];
}
