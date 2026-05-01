import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import type { ProjectRecord } from '../types/project';
import type { SpeakerAttribution, SpeakerProfile, SpeakerTag } from '../types/speaker';
import { normalizeSpeakerProfiles } from '../types/speaker';
import type { TranscriptSegment } from '../types/transcript';

export interface SpeakerCorrectionProfileSections {
  primaryProfiles: SpeakerProfile[];
  secondaryProfiles: SpeakerProfile[];
}

function resolveSegmentGroupId(segment: TranscriptSegment): string {
  return segment.speakerAttribution?.groupId || segment.speaker?.id || '';
}

function buildAnonymousAttribution(
  groupId: string,
  anonymousLabel: string,
  previous: SpeakerAttribution | undefined,
): SpeakerAttribution {
  return {
    groupId,
    anonymousLabel,
    state: 'anonymous',
    source: 'manual',
    confidence: 'low',
    candidates: previous?.candidates || [],
  };
}

function buildOrderedEnabledSpeakerProfileIds(
  profiles: SpeakerProfile[],
  existingIds: string[],
  nextId: string,
): string[] {
  const enabledIdSet = new Set([...existingIds, nextId]);
  return profiles
    .filter((profile) => enabledIdSet.has(profile.id))
    .map((profile) => profile.id);
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

export function applySpeakerProfileToSegments(
  segments: TranscriptSegment[],
  groupId: string,
  targetProfile: SpeakerProfile,
): TranscriptSegment[] {
  const nextSpeaker: SpeakerTag = {
    id: targetProfile.id,
    label: targetProfile.name,
    kind: 'identified',
  };

  return segments.map((segment) => (
    resolveSegmentGroupId(segment) === groupId
      ? {
          ...segment,
          speaker: nextSpeaker,
          speakerAttribution: {
            groupId,
            anonymousLabel: segment.speakerAttribution?.anonymousLabel || segment.speaker?.label || 'Speaker',
            state: 'identified',
            source: 'manual',
            confidence: 'high',
            candidates: segment.speakerAttribution?.candidates || [],
          },
        }
      : segment
  ));
}

export function resetSpeakerGroupToAnonymous(
  segments: TranscriptSegment[],
  groupId: string,
): TranscriptSegment[] {
  return segments.map((segment) => {
    if (resolveSegmentGroupId(segment) !== groupId) {
      return segment;
    }

    const anonymousLabel = segment.speakerAttribution?.anonymousLabel || segment.speaker?.label || 'Speaker';
    return {
      ...segment,
      speaker: {
        id: groupId,
        label: anonymousLabel,
        kind: 'anonymous',
      },
      speakerAttribution: buildAnonymousAttribution(groupId, anonymousLabel, segment.speakerAttribution),
    };
  });
}

class SpeakerCorrectionService {
  async assignProfileToSpeakerGroup(
    sourceGroupId: string,
    targetProfileId: string,
  ): Promise<TranscriptSegment[]> {
    const profiles = normalizeSpeakerProfiles(useConfigStore.getState().config.speakerProfiles);
    const targetProfile = profiles.find((profile) => profile.id === targetProfileId);

    if (!sourceGroupId.trim()) {
      throw new Error('Speaker correction requires a source speaker id.');
    }

    if (!targetProfile) {
      throw new Error(`Speaker profile not found: ${targetProfileId}`);
    }

    const sessionStore = useTranscriptSessionStore.getState();
    const nextSegments = applySpeakerProfileToSegments(
      sessionStore.segments,
      sourceGroupId,
      targetProfile,
    );
    sessionStore.setSegments(nextSegments);

    const projectStore = useProjectStore.getState();
    const activeProject = projectStore.getActiveProject();
    if (
      activeProject
      && !activeProject.defaults.enabledSpeakerProfileIds.includes(targetProfile.id)
    ) {
      const enabledSpeakerProfileIds = buildOrderedEnabledSpeakerProfileIds(
        profiles,
        activeProject.defaults.enabledSpeakerProfileIds,
        targetProfile.id,
      );
      await projectStore.updateProjectDefaults(activeProject.id, {
        enabledSpeakerProfileIds,
      });
    }

    useEffectiveConfigStore.getState().syncConfig();
    return nextSegments;
  }

  async resetGroupToAnonymous(groupId: string): Promise<TranscriptSegment[]> {
    if (!groupId.trim()) {
      throw new Error('Speaker correction requires a source speaker id.');
    }

    const sessionStore = useTranscriptSessionStore.getState();
    const nextSegments = resetSpeakerGroupToAnonymous(sessionStore.segments, groupId);
    sessionStore.setSegments(nextSegments);
    return nextSegments;
  }
}

export const speakerCorrectionService = new SpeakerCorrectionService();
