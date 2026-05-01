import type { SpeakerAttribution, SpeakerCandidate, SpeakerTag } from '../types/speaker';
import type { TranscriptSegment } from '../types/transcript';

export type SpeakerReviewStatus = 'pending' | 'auto' | 'reviewed';
export type SpeakerReviewRiskReason =
  | 'suggested'
  | 'anonymous'
  | 'low_confidence'
  | 'medium_confidence'
  | 'auto_identified'
  | 'reviewed';
export type SpeakerReviewFilter =
  | 'pending'
  | 'suggested'
  | 'anonymous'
  | 'identified'
  | 'reviewed'
  | 'all';

export interface SpeakerReviewSegmentPreview {
  id: string;
  start: number;
  end: number;
  text: string;
}

export interface SpeakerReviewGroup {
  groupId: string;
  displayLabel: string;
  anonymousLabel: string;
  state: SpeakerAttribution['state'];
  source: SpeakerAttribution['source'];
  confidence: SpeakerAttribution['confidence'];
  reviewStatus: SpeakerReviewStatus;
  riskReason: SpeakerReviewRiskReason;
  priority: number;
  candidates: SpeakerCandidate[];
  speaker: SpeakerTag | undefined;
  segmentCount: number;
  durationSeconds: number;
  firstSegmentId: string;
  firstStart: number;
  previewSegments: SpeakerReviewSegmentPreview[];
}

export interface SpeakerReviewCounts {
  total: number;
  pending: number;
  suggested: number;
  anonymous: number;
  identified: number;
  reviewed: number;
}

function fallbackAttribution(segment: TranscriptSegment): SpeakerAttribution | null {
  if (!segment.speaker) {
    return null;
  }

  const groupId = segment.speakerAttribution?.groupId || segment.speaker.id || segment.id;
  const anonymousLabel = segment.speakerAttribution?.anonymousLabel
    || (segment.speaker.kind === 'anonymous' ? segment.speaker.label : 'Speaker');

  return {
    groupId,
    anonymousLabel,
    state: segment.speakerAttribution?.state || (segment.speaker.kind === 'identified' ? 'identified' : 'anonymous'),
    source: segment.speakerAttribution?.source || 'auto',
    confidence: segment.speakerAttribution?.confidence || (segment.speaker.kind === 'identified' ? 'high' : 'low'),
    candidates: segment.speakerAttribution?.candidates || [],
  };
}

function reviewStateWeight(state: SpeakerAttribution['state']): number {
  switch (state) {
    case 'suggested':
      return 0;
    case 'anonymous':
      return 1;
    case 'identified':
    default:
      return 2;
  }
}

function resolveReviewStatus(
  source: SpeakerAttribution['source'],
  state: SpeakerAttribution['state'],
  confidence: SpeakerAttribution['confidence'],
): SpeakerReviewStatus {
  if (source === 'manual') {
    return 'reviewed';
  }

  if (state === 'suggested' || state === 'anonymous' || confidence !== 'high') {
    return 'pending';
  }

  return 'auto';
}

function resolveRiskReason(
  source: SpeakerAttribution['source'],
  state: SpeakerAttribution['state'],
  confidence: SpeakerAttribution['confidence'],
): SpeakerReviewRiskReason {
  if (source === 'manual') {
    return 'reviewed';
  }

  if (state === 'suggested') {
    return 'suggested';
  }

  if (state === 'anonymous') {
    return 'anonymous';
  }

  if (confidence === 'low') {
    return 'low_confidence';
  }

  if (confidence === 'medium') {
    return 'medium_confidence';
  }

  return 'auto_identified';
}

function getPriority(reason: SpeakerReviewRiskReason): number {
  switch (reason) {
    case 'suggested':
      return 0;
    case 'anonymous':
      return 1;
    case 'low_confidence':
      return 2;
    case 'medium_confidence':
      return 3;
    case 'auto_identified':
      return 4;
    case 'reviewed':
    default:
      return 5;
  }
}

function segmentPreview(segment: TranscriptSegment): SpeakerReviewSegmentPreview {
  return {
    id: segment.id,
    start: segment.start,
    end: segment.end,
    text: segment.text,
  };
}

export function buildSpeakerReviewGroups(segments: TranscriptSegment[]): SpeakerReviewGroup[] {
  const groups = new Map<string, SpeakerReviewGroup>();

  for (const segment of segments) {
    const attribution = segment.speakerAttribution || fallbackAttribution(segment);
    if (!attribution) {
      continue;
    }

    const existing = groups.get(attribution.groupId);
    if (existing) {
      existing.segmentCount += 1;
      existing.durationSeconds += Math.max(0, segment.end - segment.start);
      if (segment.start < existing.firstStart) {
        existing.firstStart = segment.start;
        existing.firstSegmentId = segment.id;
      }
      if (reviewStateWeight(attribution.state) < reviewStateWeight(existing.state)) {
        existing.state = attribution.state;
        existing.confidence = attribution.confidence;
      }
      if (attribution.source === 'manual') {
        existing.source = 'manual';
      }
      if (attribution.candidates.length > existing.candidates.length) {
        existing.candidates = attribution.candidates;
      }
      if (!existing.speaker && segment.speaker) {
        existing.speaker = segment.speaker;
        existing.displayLabel = segment.speaker.label;
      }
      existing.previewSegments.push(segmentPreview(segment));
      continue;
    }

    const reviewStatus = resolveReviewStatus(attribution.source, attribution.state, attribution.confidence);
    const riskReason = resolveRiskReason(attribution.source, attribution.state, attribution.confidence);

    groups.set(attribution.groupId, {
      groupId: attribution.groupId,
      displayLabel: segment.speaker?.label || attribution.anonymousLabel,
      anonymousLabel: attribution.anonymousLabel,
      state: attribution.state,
      source: attribution.source,
      confidence: attribution.confidence,
      reviewStatus,
      riskReason,
      priority: getPriority(riskReason),
      candidates: attribution.candidates,
      speaker: segment.speaker,
      segmentCount: 1,
      durationSeconds: Math.max(0, segment.end - segment.start),
      firstSegmentId: segment.id,
      firstStart: segment.start,
      previewSegments: [segmentPreview(segment)],
    });
  }

  return [...groups.values()]
    .map((group) => {
      const reviewStatus = resolveReviewStatus(group.source, group.state, group.confidence);
      const riskReason = resolveRiskReason(group.source, group.state, group.confidence);
      return {
        ...group,
        reviewStatus,
        riskReason,
        priority: getPriority(riskReason),
        previewSegments: [...group.previewSegments]
          .sort((left, right) => left.start - right.start)
          .slice(0, 3),
      };
    })
    .sort((left, right) => (
      left.priority - right.priority
      || left.firstStart - right.firstStart
    ));
}

export function buildSpeakerReviewCounts(groups: SpeakerReviewGroup[]): SpeakerReviewCounts {
  return groups.reduce<SpeakerReviewCounts>((counts, group) => ({
    total: counts.total + 1,
    pending: counts.pending + (group.reviewStatus === 'pending' ? 1 : 0),
    suggested: counts.suggested + (group.state === 'suggested' ? 1 : 0),
    anonymous: counts.anonymous + (group.state === 'anonymous' ? 1 : 0),
    identified: counts.identified + (group.state === 'identified' ? 1 : 0),
    reviewed: counts.reviewed + (group.reviewStatus === 'reviewed' ? 1 : 0),
  }), {
    total: 0,
    pending: 0,
    suggested: 0,
    anonymous: 0,
    identified: 0,
    reviewed: 0,
  });
}

export function filterSpeakerReviewGroups(
  groups: SpeakerReviewGroup[],
  filter: SpeakerReviewFilter,
): SpeakerReviewGroup[] {
  switch (filter) {
    case 'pending':
      return groups.filter((group) => group.reviewStatus === 'pending');
    case 'suggested':
      return groups.filter((group) => group.state === 'suggested');
    case 'anonymous':
      return groups.filter((group) => group.state === 'anonymous');
    case 'identified':
      return groups.filter((group) => group.state === 'identified');
    case 'reviewed':
      return groups.filter((group) => group.reviewStatus === 'reviewed');
    case 'all':
    default:
      return groups;
  }
}
