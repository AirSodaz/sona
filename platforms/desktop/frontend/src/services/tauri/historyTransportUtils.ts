import type {
  HistorySummaryPayload_Deserialize,
  TranscriptDiffRow_Deserialize,
  TranscriptDiffRow_Serialize,
  TranscriptSegment_Deserialize,
  TranscriptSegment_Serialize,
} from '../../bindings';
import { normalizeSpeakerAttribution, normalizeSpeakerTag } from '../../types/speakerNormalization';
import type { HistorySummaryPayload, TranscriptSegment } from '../../types/transcript';
import type { TranscriptDiffRow } from '../../types/transcriptSnapshot';

export function normalizeTranscriptSegment(
  segment: TranscriptSegment_Serialize
): TranscriptSegment {
  return {
    id: segment.id,
    text: segment.text,
    start: segment.start,
    end: segment.end,
    isFinal: segment.isFinal,
    timing: segment.timing ?? undefined,
    tokens: segment.tokens ?? undefined,
    timestamps: segment.timestamps ?? undefined,
    durations: segment.durations ?? undefined,
    translation: segment.translation ?? undefined,
    speaker: normalizeSpeakerTag(segment.speaker) ?? undefined,
    speakerAttribution: normalizeSpeakerAttribution(segment.speakerAttribution) ?? undefined,
  };
}

export function toTranscriptSegmentTransport(
  segment: TranscriptSegment
): TranscriptSegment_Deserialize {
  return {
    id: segment.id,
    text: segment.text,
    start: segment.start,
    end: segment.end,
    isFinal: segment.isFinal,
    timing: segment.timing ?? null,
    tokens: segment.tokens ?? null,
    timestamps: segment.timestamps ?? null,
    durations: segment.durations ?? null,
    translation: segment.translation ?? null,
    speaker: segment.speaker ?? null,
    speakerAttribution: segment.speakerAttribution ?? null,
  };
}

export function toTranscriptDiffRowTransport(
  row: TranscriptDiffRow
): TranscriptDiffRow_Deserialize {
  return {
    id: row.id,
    status: row.status,
    snapshotSegment: row.snapshotSegment ? toTranscriptSegmentTransport(row.snapshotSegment) : null,
    currentSegment: row.currentSegment ? toTranscriptSegmentTransport(row.currentSegment) : null,
    snapshotIndex: row.snapshotIndex,
    currentIndex: row.currentIndex,
  };
}

export function normalizeTranscriptDiffRow(row: TranscriptDiffRow_Serialize): TranscriptDiffRow {
  return {
    ...row,
    snapshotSegment: row.snapshotSegment
      ? normalizeTranscriptSegment(row.snapshotSegment)
      : undefined,
    currentSegment: row.currentSegment ? normalizeTranscriptSegment(row.currentSegment) : undefined,
  };
}

export function toHistorySummaryPayloadTransport(
  payload: HistorySummaryPayload
): HistorySummaryPayload_Deserialize {
  return {
    activeTemplateId: payload.activeTemplateId,
    record: payload.record ?? null,
  };
}

export function inferAudioExtensionFromMime(mimeType: string): string | null {
  const normalized = mimeType.toLowerCase().trim();
  if (normalized.includes('wav') || normalized.includes('wave')) return 'wav';
  if (normalized.includes('webm')) return 'webm';
  if (normalized.includes('mp4') || normalized.includes('m4a')) return 'm4a';
  if (normalized.includes('aac')) return 'aac';
  if (normalized.includes('ogg')) return 'ogg';
  if (normalized.includes('flac')) return 'flac';
  if (normalized.includes('mp3') || normalized.includes('mpeg')) return 'mp3';
  if (normalized.includes('opus')) return 'opus';
  return null;
}

export function inferAudioExtensionFromBlob(blob: Blob, fallback = 'webm'): string {
  return inferAudioExtensionFromMime(blob.type) ?? fallback;
}

export function inferAudioExtensionFromPath(filePath: string, fallback = 'wav'): string {
  const fileName = filePath.split(/[/\\]/).pop() || '';
  const extensionIndex = fileName.lastIndexOf('.');
  const extension =
    extensionIndex >= 0
      ? fileName
          .slice(extensionIndex + 1)
          .trim()
          .toLowerCase()
      : '';
  return extension || fallback;
}
