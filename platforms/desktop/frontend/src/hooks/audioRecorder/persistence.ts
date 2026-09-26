import type { LiveRecordingDraftHandle } from '../../services/historyService';
import type { AppConfig } from '../../types/config';
import type { HistoryItem } from '../../types/history';
import type { TranscriptSegment } from '../../types/transcript';
import type {
  AudioRecorderLogger,
  RecordingHistorySaver,
  RecordingMetaState,
  RecordingPersistenceTranscriptState,
} from './types';

export function syncSavedRecordingMeta(
  transcriptState: RecordingMetaState,
  title: string,
  historyId: string,
  icon: string | undefined | null
): void {
  transcriptState.setSourceHistoryId(historyId);
  transcriptState.setTitle(title);
  transcriptState.setIcon(icon || null);
}

export function getRecordedAudioExtension(mimeType: string): string {
  if (mimeType.includes('mp4')) return 'm4a';
  if (mimeType.includes('aac')) return 'aac';
  if (mimeType.includes('ogg')) return 'ogg';
  if (mimeType.includes('wav')) return 'wav';
  return 'webm';
}

interface CreateRecordingPersistenceArgs {
  logger: AudioRecorderLogger;
  history: RecordingHistorySaver;
  getTranscriptState: () => RecordingPersistenceTranscriptState;
  getActiveProjectId: () => string | null;
  setActiveProjectId: (projectId: string | null) => Promise<void> | void;
  addHistoryItem: (item: HistoryItem) => void;
  upsertHistoryItem: (item: HistoryItem) => void;
  removeHistoryItem: (id: string) => void;
  persistSummary: (historyId: string) => Promise<void>;
  postProcessSavedItem?: (
    historyId: string,
    segments: TranscriptSegment[]
  ) => Promise<TranscriptSegment[]>;
  annotateSegmentsForFile: (
    filePath: string,
    segments: TranscriptSegment[],
    config: AppConfig
  ) => Promise<TranscriptSegment[]>;
  syncSavedRecordingMeta: (
    title: string,
    historyId: string,
    icon: string | undefined | null
  ) => void;
  writeFile: (filePath: string, contents: Uint8Array) => Promise<void>;
  removeFile: (filePath: string) => Promise<void>;
  fileSrcFromPath: (filePath: string) => string;
}

export function createRecordingPersistence({
  logger,
  history,
  getTranscriptState,
  getActiveProjectId,
  setActiveProjectId,
  addHistoryItem,
  upsertHistoryItem,
  removeHistoryItem,
  persistSummary,
  postProcessSavedItem,
  annotateSegmentsForFile,
  syncSavedRecordingMeta: syncMeta,
  writeFile,
  removeFile,
  fileSrcFromPath,
}: CreateRecordingPersistenceArgs) {
  async function persistSavedItem(
    newItem: HistoryItem,
    strategy: 'add' | 'upsert' = 'upsert',
    completedSegments?: TranscriptSegment[]
  ): Promise<void> {
    if (strategy === 'add') {
      addHistoryItem(newItem);
    } else {
      upsertHistoryItem(newItem);
    }
    syncMeta(newItem.title, newItem.id, newItem.icon);
    void setActiveProjectId(newItem.tagIds?.[0] ?? newItem.projectId ?? null);
    if (completedSegments?.length && postProcessSavedItem) {
      const processedSegments = await postProcessSavedItem(newItem.id, completedSegments);
      getTranscriptState().setSegments(processedSegments);
    }
    await persistSummary(newItem.id);
  }

  function mergeSpeakerAnnotations(
    targetSegments: TranscriptSegment[],
    annotated: TranscriptSegment[]
  ): TranscriptSegment[] {
    const speakerMap = new Map(
      annotated.map((seg) => [
        seg.id,
        { speaker: seg.speaker, speakerAttribution: seg.speakerAttribution },
      ])
    );
    return targetSegments.map((seg) => {
      const spk = speakerMap.get(seg.id);
      if (!spk) {
        return seg;
      }
      return {
        ...seg,
        speaker: spk.speaker,
        speakerAttribution: spk.speakerAttribution,
      };
    });
  }

  async function applyBackgroundSpeakerAnnotation(
    sessionId: string,
    filePath: string,
    initialSegments: TranscriptSegment[]
  ): Promise<void> {
    try {
      const transcriptState = getTranscriptState();
      const annotated = await annotateSegmentsForFile(
        filePath,
        initialSegments,
        transcriptState.config
      );
      if (!annotated || annotated === initialSegments) {
        return;
      }

      const currentTranscriptState = getTranscriptState();
      if (currentTranscriptState.setSegmentsForSession) {
        currentTranscriptState.setSegmentsForSession(
          sessionId,
          mergeSpeakerAnnotations(initialSegments, annotated)
        );
      } else if (currentTranscriptState.activeSessionId === sessionId) {
        currentTranscriptState.setSegments(
          mergeSpeakerAnnotations(currentTranscriptState.segments, annotated)
        );
      }

      if (history.updateTranscript) {
        const mergedForHistory = mergeSpeakerAnnotations(initialSegments, annotated);
        const updatedItem = await history.updateTranscript(sessionId, mergedForHistory);
        upsertHistoryItem(updatedItem);
      }
    } catch (error) {
      logger.warn('[useAudioRecorder] Failed to annotate speaker labels in background:', error);
    }
  }

  async function writeRecordedBlobToPath(blob: Blob, filePath: string): Promise<void> {
    const contents = new Uint8Array(await blob.arrayBuffer());
    await writeFile(filePath, contents);
  }

  async function createLiveRecordingDraft(
    audioExtension: string,
    id?: string
  ): Promise<LiveRecordingDraftHandle> {
    const draft = await history.createLiveRecordingDraft(
      audioExtension,
      getActiveProjectId(),
      'system:mic',
      id
    );
    await persistSavedItem(draft.item, 'add');
    return draft;
  }

  async function discardLiveRecordingDraft(draft: LiveRecordingDraftHandle): Promise<void> {
    await history.discardLiveRecordingDraft(draft.item.id);
    removeHistoryItem(draft.item.id);
  }

  async function persistBrowserRecording(
    draft: LiveRecordingDraftHandle,
    blob: Blob,
    duration: number
  ): Promise<void> {
    const transcriptState = getTranscriptState();
    const segments = transcriptState.segments;

    if (segments.length === 0) {
      await discardLiveRecordingDraft(draft);
      return;
    }

    try {
      await writeRecordedBlobToPath(blob, draft.audioAbsolutePath);
      transcriptState.setAudioUrl(fileSrcFromPath(draft.audioAbsolutePath));

      const newItem = await history.completeLiveRecordingDraft(draft.item.id, segments, duration);
      await persistSavedItem(newItem, 'upsert', segments);

      void applyBackgroundSpeakerAnnotation(draft.item.id, draft.audioAbsolutePath, segments);
    } catch (error) {
      logger.error('[useAudioRecorder] Failed to persist browser recording draft:', error);
      throw error;
    }
  }

  async function persistNativeRecording(
    draft: LiveRecordingDraftHandle,
    savedWavPath: string,
    duration: number
  ): Promise<void> {
    const transcriptState = getTranscriptState();
    const segments = transcriptState.segments;

    if (segments.length > 0) {
      transcriptState.setAudioUrl(fileSrcFromPath(savedWavPath));

      const newItem = await history.completeLiveRecordingDraft(draft.item.id, segments, duration);
      await persistSavedItem(newItem, 'upsert', segments);

      void applyBackgroundSpeakerAnnotation(draft.item.id, savedWavPath, segments);
      return;
    }

    logger.info('[useAudioRecorder] Empty transcript, deleting unsaved WAV file:', savedWavPath);
    try {
      await removeFile(savedWavPath);
    } catch (error) {
      logger.error('[useAudioRecorder] Failed to delete empty WAV file:', error);
    }
    await discardLiveRecordingDraft(draft);
  }

  return {
    createLiveRecordingDraft,
    discardLiveRecordingDraft,
    persistBrowserRecording,
    persistNativeRecording,
  };
}
