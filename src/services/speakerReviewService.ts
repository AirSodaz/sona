import type { SpeakerAttribution, SpeakerCandidate, SpeakerTag } from '../types/speaker';
import type { TranscriptSegment } from '../types/transcript';

export interface SpeakerReviewGroup {
  groupId: string;
  displayLabel: string;
  anonymousLabel: string;
  state: SpeakerAttribution['state'];
  source: SpeakerAttribution['source'];
  confidence: SpeakerAttribution['confidence'];
  candidates: SpeakerCandidate[];
  speaker: SpeakerTag | undefined;
  segmentCount: number;
  durationSeconds: number;
  firstSegmentId: string;
  firstStart: number;
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
        existing.source = attribution.source;
      }
      if (attribution.candidates.length > existing.candidates.length) {
        existing.candidates = attribution.candidates;
      }
      if (!existing.speaker && segment.speaker) {
        existing.speaker = segment.speaker;
        existing.displayLabel = segment.speaker.label;
      }
      continue;
    }

    groups.set(attribution.groupId, {
      groupId: attribution.groupId,
      displayLabel: segment.speaker?.label || attribution.anonymousLabel,
      anonymousLabel: attribution.anonymousLabel,
      state: attribution.state,
      source: attribution.source,
      confidence: attribution.confidence,
      candidates: attribution.candidates,
      speaker: segment.speaker,
      segmentCount: 1,
      durationSeconds: Math.max(0, segment.end - segment.start),
      firstSegmentId: segment.id,
      firstStart: segment.start,
    });
  }

  return [...groups.values()].sort((left, right) => (
    reviewStateWeight(left.state) - reviewStateWeight(right.state)
    || left.firstStart - right.firstStart
  ));
}
