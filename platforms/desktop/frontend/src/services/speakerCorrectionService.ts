import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import type { SpeakerCorrectionProfileSections, SpeakerProfile } from '../types/speaker';
import { normalizeSpeakerProfiles } from '../types/speakerNormalization';
import type { TranscriptSegment } from '../types/transcript';
import {
  applySpeakerProfileToGroup,
  confirmSpeakerGroupReview as confirmSpeakerGroupReviewInRust,
  resetSpeakerGroupToAnonymous as resetSpeakerGroupToAnonymousInRust,
} from './tauri/speaker';

export type { SpeakerCorrectionProfileSections } from '../types/speaker';
export type {
  ApplySpeakerProfileToGroupRequest,
  SpeakerCorrectionResponse,
  SpeakerGroupRequest,
} from '../types/speakerCommands';

export function buildSpeakerCorrectionProfileSections(
  inputProfiles: SpeakerProfile[] | undefined,
  currentProjectId?: string | null
): SpeakerCorrectionProfileSections {
  const profiles = normalizeSpeakerProfiles(inputProfiles);

  const isProfileInCurrentScope = (profile: SpeakerProfile) => {
    if (!profile.enabled) return false;
    const scope = profile.scope ?? 'global';
    if (scope === 'global') return true;
    if (scope === 'project' && currentProjectId) {
      return (profile.projectIds ?? []).includes(currentProjectId);
    }
    return false;
  };

  return {
    primaryProfiles: profiles.filter(isProfileInCurrentScope),
    secondaryProfiles: profiles.filter((profile) => !isProfileInCurrentScope(profile)),
  };
}

export interface SpeakerCorrectionServicePorts {
  getConfigStore: typeof useConfigStore.getState;
  getEffectiveConfigStore: typeof useEffectiveConfigStore.getState;
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
    currentProjectId?: string | null
  ): Promise<TranscriptSegment[]> {
    const configStore = this.ports.getConfigStore();
    const profiles = normalizeSpeakerProfiles(configStore.config.speakerProfiles);
    const sessionStore = this.ports.getTranscriptSessionStore();

    const response = await this.ports.applySpeakerProfileToGroup({
      segments: sessionStore.segments,
      groupId: sourceGroupId,
      targetProfileId,
      speakerProfiles: profiles,
      enabledSpeakerProfileIds: profiles
        .filter((profile) => profile.enabled)
        .map((profile) => profile.id),
    });

    sessionStore.setSegments(response.segments);

    if (response.enabledSpeakerProfileIds) {
      const enabledIds = new Set(response.enabledSpeakerProfileIds);
      configStore.setConfig({
        speakerProfiles: profiles.map((profile) => {
          const isTarget = profile.id === targetProfileId;
          const shouldAddProject =
            isTarget &&
            currentProjectId &&
            profile.scope === 'project' &&
            !profile.projectIds?.includes(currentProjectId);

          const projectIds = shouldAddProject
            ? [...(profile.projectIds ?? []), currentProjectId]
            : profile.projectIds;

          return {
            ...profile,
            enabled: enabledIds.has(profile.id),
            ...(projectIds ? { projectIds } : {}),
          };
        }),
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

export function createSpeakerCorrectionService(
  ports: SpeakerCorrectionServicePorts
): SpeakerCorrectionService {
  return new SpeakerCorrectionService(ports);
}

export const speakerCorrectionService = createSpeakerCorrectionService({
  getConfigStore: useConfigStore.getState,
  getEffectiveConfigStore: useEffectiveConfigStore.getState,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  applySpeakerProfileToGroup,
  confirmSpeakerGroupReview: confirmSpeakerGroupReviewInRust,
  resetSpeakerGroupToAnonymous: resetSpeakerGroupToAnonymousInRust,
});
