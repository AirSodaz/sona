import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import type { ProjectRecord } from '../types/project';
import type { SpeakerProfile } from '../types/speaker';
import { normalizeSpeakerProfiles } from '../types/speaker';
import type { TranscriptSegment } from '../types/transcript';
import {
  applySpeakerProfileToGroup,
  confirmSpeakerGroupReview as confirmSpeakerGroupReviewInRust,
  resetSpeakerGroupToAnonymous as resetSpeakerGroupToAnonymousInRust,
} from './tauri/speaker';

export interface SpeakerCorrectionProfileSections {
  primaryProfiles: SpeakerProfile[];
  secondaryProfiles: SpeakerProfile[];
}

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

export function buildSpeakerCorrectionProfileSections(
  inputProfiles: SpeakerProfile[] | undefined,
  activeProject: ProjectRecord | null,
): SpeakerCorrectionProfileSections {
  const profiles = normalizeSpeakerProfiles(inputProfiles);
  if (!activeProject) {
    return {
      primaryProfiles: profiles,
      secondaryProfiles: [],
    };
  }

  const enabledIdSet = new Set(activeProject.defaults.enabledSpeakerProfileIds);
  return {
    primaryProfiles: profiles.filter((profile) => enabledIdSet.has(profile.id)),
    secondaryProfiles: profiles.filter((profile) => !enabledIdSet.has(profile.id)),
  };
}

class SpeakerCorrectionService {
  async assignProfileToSpeakerGroup(
    sourceGroupId: string,
    targetProfileId: string,
  ): Promise<TranscriptSegment[]> {
    const profiles = normalizeSpeakerProfiles(useConfigStore.getState().config.speakerProfiles);
    const sessionStore = useTranscriptSessionStore.getState();
    const projectStore = useProjectStore.getState();
    const activeProject = projectStore.getActiveProject();

    const response = await applySpeakerProfileToGroup({
      segments: sessionStore.segments,
      groupId: sourceGroupId,
      targetProfileId,
      speakerProfiles: profiles,
      enabledSpeakerProfileIds: activeProject?.defaults.enabledSpeakerProfileIds || [],
    });

    sessionStore.setSegments(response.segments);

    if (activeProject && response.enabledSpeakerProfileIds) {
      await projectStore.updateProjectDefaults(activeProject.id, {
        enabledSpeakerProfileIds: response.enabledSpeakerProfileIds,
      });
    }

    await useEffectiveConfigStore.getState().syncConfig();
    return response.segments;
  }

  async resetGroupToAnonymous(groupId: string): Promise<TranscriptSegment[]> {
    const sessionStore = useTranscriptSessionStore.getState();
    const response = await resetSpeakerGroupToAnonymousInRust({
      segments: sessionStore.segments,
      groupId,
    });
    sessionStore.setSegments(response.segments);
    return response.segments;
  }

  async confirmSpeakerGroupReview(groupId: string): Promise<TranscriptSegment[]> {
    const sessionStore = useTranscriptSessionStore.getState();
    const response = await confirmSpeakerGroupReviewInRust({
      segments: sessionStore.segments,
      groupId,
    });
    sessionStore.setSegments(response.segments);
    return response.segments;
  }
}

export const speakerCorrectionService = new SpeakerCorrectionService();
