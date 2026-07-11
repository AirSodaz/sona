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

export interface SpeakerCorrectionServicePorts {
  getConfigStore: typeof useConfigStore.getState;
  getEffectiveConfigStore: typeof useEffectiveConfigStore.getState;
  getProjectStore: typeof useProjectStore.getState;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  applySpeakerProfileToGroup: typeof applySpeakerProfileToGroup;
  confirmSpeakerGroupReview: typeof confirmSpeakerGroupReviewInRust;
  resetSpeakerGroupToAnonymous: typeof resetSpeakerGroupToAnonymousInRust;
}

export class SpeakerCorrectionService {
  constructor(private readonly ports: SpeakerCorrectionServicePorts) {}

  async assignProfileToSpeakerGroup(
    sourceGroupId: string,
    targetProfileId: string,
  ): Promise<TranscriptSegment[]> {
    const profiles = normalizeSpeakerProfiles(this.ports.getConfigStore().config.speakerProfiles);
    const sessionStore = this.ports.getTranscriptSessionStore();
    const projectStore = this.ports.getProjectStore();
    const activeProject = projectStore.getActiveProject();

    const response = await this.ports.applySpeakerProfileToGroup({
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

    await this.ports.getEffectiveConfigStore().syncConfig();
    return response.segments;
  }

  async resetGroupToAnonymous(groupId: string): Promise<TranscriptSegment[]> {
    const sessionStore = this.ports.getTranscriptSessionStore();
    const response = await this.ports.resetSpeakerGroupToAnonymous({
      segments: sessionStore.segments,
      groupId,
    });
    sessionStore.setSegments(response.segments);
    return response.segments;
  }

  async confirmSpeakerGroupReview(groupId: string): Promise<TranscriptSegment[]> {
    const sessionStore = this.ports.getTranscriptSessionStore();
    const response = await this.ports.confirmSpeakerGroupReview({
      segments: sessionStore.segments,
      groupId,
    });
    sessionStore.setSegments(response.segments);
    return response.segments;
  }
}

export function createSpeakerCorrectionService(ports: SpeakerCorrectionServicePorts): SpeakerCorrectionService {
  return new SpeakerCorrectionService(ports);
}

export const speakerCorrectionService = createSpeakerCorrectionService({
  getConfigStore: useConfigStore.getState,
  getEffectiveConfigStore: useEffectiveConfigStore.getState,
  getProjectStore: useProjectStore.getState,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  applySpeakerProfileToGroup,
  confirmSpeakerGroupReview: confirmSpeakerGroupReviewInRust,
  resetSpeakerGroupToAnonymous: resetSpeakerGroupToAnonymousInRust,
});
