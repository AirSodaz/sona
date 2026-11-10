import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { setSegmentsForSession } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { SpeakerCorrectionProfileSections, SpeakerProfile } from '../types/speaker';
import type { SpeakerCorrectionResponse } from '../types/speakerCommands';
import { normalizeSpeakerProfiles } from '../types/speakerNormalization';
import type { TranscriptSegment } from '../types/transcript';
import {
  captureTaskTarget,
  resolveLatestSegmentsForTarget,
  resolveTargetHistoryId,
  runWithDocumentLock,
} from './task/taskTarget';
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

export interface SpeakerTargetOptions {
  sessionId?: string | null;
  historyId?: string | null;
}

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

  private async runLockedSpeakerMutation(
    targetOptions: SpeakerTargetOptions | undefined,
    mutate: (segments: TranscriptSegment[]) => Promise<SpeakerCorrectionResponse>
  ): Promise<SpeakerCorrectionResponse> {
    const fallbackSession = this.ports.getTranscriptSessionStore();
    const sessionId = targetOptions?.sessionId ?? null;
    const sessionSnapshot = sessionId
      ? useTranscriptStore.getState().sessions[sessionId]
      : undefined;
    const target = captureTaskTarget({
      sessionId,
      historyId:
        targetOptions?.historyId ??
        (sessionSnapshot ? sessionSnapshot.sourceHistoryId : fallbackSession.sourceHistoryId),
    });

    return runWithDocumentLock(target, { taskType: 'speakerCorrection' }, async () => {
      const latestSegments = await resolveLatestSegmentsForTarget(
        target,
        sessionSnapshot?.segments ?? fallbackSession.segments
      );
      const response = await mutate(latestSegments);
      const resolvedHistoryId = resolveTargetHistoryId(target);

      if (resolvedHistoryId) {
        await useHistoryStore
          .getState()
          .commitSystemTranscriptUpdate(resolvedHistoryId, response.segments);
      } else if (target.sessionId) {
        setSegmentsForSession(target.sessionId, response.segments);
      } else {
        this.ports.getTranscriptSessionStore().setSegments(response.segments);
      }
      return response;
    });
  }

  async assignProfileToSpeakerGroup(
    sourceGroupId: string,
    targetProfileId: string,
    currentProjectId?: string | null,
    targetOptions?: SpeakerTargetOptions
  ): Promise<TranscriptSegment[]> {
    const configStore = this.ports.getConfigStore();
    const profiles = normalizeSpeakerProfiles(configStore.config.speakerProfiles);

    const response = await this.runLockedSpeakerMutation(targetOptions, (segments) =>
      this.ports.applySpeakerProfileToGroup({
        segments,
        groupId: sourceGroupId,
        targetProfileId,
        speakerProfiles: profiles,
        enabledSpeakerProfileIds: profiles
          .filter((profile) => profile.enabled)
          .map((profile) => profile.id),
      })
    );

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

  async resetGroupToAnonymous(
    groupId: string,
    targetOptions?: SpeakerTargetOptions
  ): Promise<TranscriptSegment[]> {
    const response = await this.runLockedSpeakerMutation(targetOptions, (segments) =>
      this.ports.resetSpeakerGroupToAnonymous({
        segments,
        groupId,
      })
    );
    return response.segments;
  }

  async confirmSpeakerGroupReview(
    groupId: string,
    targetOptions?: SpeakerTargetOptions
  ): Promise<TranscriptSegment[]> {
    const response = await this.runLockedSpeakerMutation(targetOptions, (segments) =>
      this.ports.confirmSpeakerGroupReview({
        segments,
        groupId,
      })
    );
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
