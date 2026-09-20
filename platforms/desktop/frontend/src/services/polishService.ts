import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import type { AppConfig } from '../types/config';
import type { PolishMode } from '../types/llmTask';
import type { TranscriptSegment } from '../types/transcript';
import { resolvePolishPreset } from '../utils/polishPresets';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/configUtils';
import { runConfiguredSegmentTask, runTranscriptSegmentTaskJob } from './llm/segmentTask';
import { listenToTranscriptLlmJobUpdates } from './llmTaskEvents';
import type {
  PolishedSegment,
  PolishSegmentsRequest,
  TranscriptLlmJobResult,
} from './llmTaskTypes';
import { resolveItemPipeline } from './projectPipeline';
import { createLlmTaskLedgerId, isTaskLedgerCancelRequested } from './taskLedgerBuilders';
import { runTranscriptLlmJob } from './tauri/llm';
import { transcriptAutoSaveRuntime } from './transcriptAutoSaveRuntime';

interface RetryPolishTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  polishPresetId?: string;
}

function buildPolishedSegmentMap(polishedChunk: PolishedSegment[]): Map<string, PolishedSegment> {
  const polishedMap = new Map<string, PolishedSegment>();
  for (let index = 0; index < polishedChunk.length; index += 1) {
    polishedMap.set(polishedChunk[index].id, polishedChunk[index]);
  }
  return polishedMap;
}

function applyPolishedChunkToSegments(
  segments: TranscriptSegment[],
  polishedMap: ReadonlyMap<string, PolishedSegment>
): TranscriptSegment[] {
  // Merge by id so polishing only rewrites transcript text and keeps each segment's
  // existing timing, speaker, and translation fields intact.
  return segments.map((segment) => {
    const polished = polishedMap.get(segment.id);
    return polished ? { ...segment, text: polished.text } : segment;
  });
}

export interface PolishServicePorts {
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  getTranscriptSidecarStore: typeof useTranscriptSidecarStore.getState;
  runConfiguredSegmentTask: typeof runConfiguredSegmentTask;
  runTranscriptSegmentTaskJob: typeof runTranscriptSegmentTaskJob;
  runTranscriptLlmJob: typeof runTranscriptLlmJob;
  listenToTranscriptLlmJobUpdates: typeof listenToTranscriptLlmJobUpdates;
  rebaselineTranscriptAutoSave?: (historyId: string, segments: TranscriptSegment[]) => void;
}

export class PolishService {
  constructor(private readonly ports: PolishServicePorts) {}

  async polishSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets'>,
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
    taskIdOverride?: string
  ): Promise<PolishedSegment[]> {
    return this.ports.runConfiguredSegmentTask({
      feature: 'polish',
      taskType: 'polish',
      config,
      segments,
      onChunk: onChunkPolished,
      taskIdOverride,
      buildRequest: ({ taskId, llmConfig, segments: inputSegments }) =>
        this.buildRequest(taskId, llmConfig, config, inputSegments),
    });
  }

  async polishSegments(
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>
  ): Promise<PolishedSegment[]> {
    return this.polishSegmentsWithConfig(
      this.ports.getEffectiveConfigSnapshot(),
      segments,
      onChunkPolished
    );
  }

  async polishTranscript() {
    const sessionStore = this.ports.getTranscriptSessionStore();
    await this.retryPolishTranscriptJob({
      segments: sessionStore.segments,
      historyId: sessionStore.sourceHistoryId,
    });
  }

  async retryPolishTranscriptJob(options: RetryPolishTranscriptJobOptions): Promise<void> {
    const { segments, historyId, polishPresetId } = options;
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const llm = getFeatureLlmConfig(config, 'polish');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    await this.ports.runTranscriptSegmentTaskJob({
      taskType: 'polish',
      segments,
      sourceHistoryId: historyId,
      onStart: (jobHistoryId) => {
        sidecarStore.updateLlmState({ isPolishing: true, polishProgress: 0 }, jobHistoryId);
      },
      onProgress: (polishProgress, jobHistoryId) => {
        // Progress tracks the transcript that launched the job, not whichever record the
        // user is viewing by the time a late chunk event arrives.
        this.ports.getTranscriptSidecarStore().updateLlmState({ polishProgress }, jobHistoryId);
      },
      runTask: async (taskId, jobHistoryId) => {
        const activeProjectId = useProjectStore.getState().activeProjectId;
        const currentItem =
          jobHistoryId && jobHistoryId !== 'current'
            ? useHistoryStore.getState().items.find((i) => i.id === jobHistoryId)
            : null;
        const projectId = currentItem?.projectId ?? activeProjectId;
        const pipeline = resolveItemPipeline(
          projectId,
          useProjectStore.getState().projects,
          config
        );
        const resolvedPresetId = polishPresetId || pipeline.polishPresetId || config.polishPresetId;
        const preset = resolvePolishPreset(resolvedPresetId, config.polishCustomPresets);
        const unlistenJobUpdates = await this.ports.listenToTranscriptLlmJobUpdates(
          taskId,
          'polish',
          (payload) => {
            if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
              return;
            }
            this.applyTranscriptJobUpdate(payload);
          }
        );
        try {
          const mode: PolishMode =
            preset.id === 'verbatim' || preset.id === 'formal' ? preset.id : 'clean';
          const result = await this.ports.runTranscriptLlmJob({
            taskId,
            taskType: 'polish',
            jobHistoryId: jobHistoryId === 'current' ? null : jobHistoryId,
            config: llm!,
            segments,
            context: preset.context || undefined,
            mode,
          });
          if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
            return;
          }
          this.applyTranscriptJobUpdate(result);
          if (result.segments && jobHistoryId && jobHistoryId !== 'current') {
            this.ports.rebaselineTranscriptAutoSave?.(jobHistoryId, result.segments);
          }
        } finally {
          unlistenJobUpdates();
        }
      },
      onFinally: (jobHistoryId) => {
        this.ports.getTranscriptSidecarStore().updateLlmState(
          {
            isPolishing: false,
            polishProgress: 0,
          },
          jobHistoryId
        );
      },
    });
  }

  applyPolishedSegmentsInMemory(
    segments: TranscriptSegment[],
    polishedChunk: PolishedSegment[]
  ): TranscriptSegment[] {
    return applyPolishedChunkToSegments(segments, buildPolishedSegmentMap(polishedChunk));
  }

  private buildRequest(
    taskId: string,
    llmConfig: NonNullable<ReturnType<typeof getFeatureLlmConfig>>,
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets'>,
    segments: TranscriptSegment[]
  ): PolishSegmentsRequest {
    const preset = resolvePolishPreset(config.polishPresetId, config.polishCustomPresets);
    const mode: PolishMode =
      preset.id === 'verbatim' || preset.id === 'formal' ? preset.id : 'clean';

    return {
      taskId,
      config: llmConfig,
      segments: segments.map(({ id, text }) => ({ id, text })),
      context: preset.context || undefined,
      mode,
    };
  }
  private applyTranscriptJobUpdate(payload: TranscriptLlmJobResult) {
    if (!payload.segments) {
      return;
    }

    const sessionStore = this.ports.getTranscriptSessionStore();
    const currentHistoryId = sessionStore.sourceHistoryId || 'current';
    const payloadHistoryId = payload.jobHistoryId || 'current';
    if (currentHistoryId !== payloadHistoryId) {
      return;
    }

    sessionStore.setSegments(payload.segments);
  }
}

export function createPolishService(ports: PolishServicePorts): PolishService {
  return new PolishService(ports);
}

export const polishService = createPolishService({
  getEffectiveConfigSnapshot,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  getTranscriptSidecarStore: useTranscriptSidecarStore.getState,
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
  runTranscriptLlmJob,
  listenToTranscriptLlmJobUpdates,
  rebaselineTranscriptAutoSave: (historyId, segments) =>
    transcriptAutoSaveRuntime.rebaseline(historyId, segments),
});
