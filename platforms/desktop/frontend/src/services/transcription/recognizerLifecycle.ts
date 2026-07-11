import type { TranscriptUpdate } from '../../types/transcript';
import { logger } from '../../utils/logger';
import { normalizeTranscriptUpdate } from '../../utils/transcriptTiming';
import { buildRecognizerOutputEvent } from '../tauri/events';
import {
  feedAudioChunk,
  flushRecognizer,
  startRecognizer,
  stopRecognizer,
} from '../tauri/recognizer';
import { listen, type UnlistenFn } from '../tauri/platform/events';

const LOG_PREVIEW_MAX_CHARS = 24;

export type RecognizerUpdateCallback = (update: TranscriptUpdate) => void;
export type RecognizerErrorCallback = (error: string) => void;

export interface RecognizerCallbackOptions {
  owner?: string;
  sessionId?: string | null;
}

interface RecognizerCallbackRegistration {
  onUpdate: RecognizerUpdateCallback;
  onError: RecognizerErrorCallback;
  owner: string;
  sessionId: string | null;
  registrationId: number;
}

const globalListeners: Map<string, UnlistenFn> = new Map();
const instanceCallbacks: Map<string, RecognizerCallbackRegistration> = new Map();
let callbackRegistrationCounter = 0;

function previewTextForLog(text: string) {
  const flattened = text.replace(/\r?\n/g, ' ');
  const preview = flattened.slice(0, LOG_PREVIEW_MAX_CHARS);
  return flattened.length > LOG_PREVIEW_MAX_CHARS ? `${preview}...` : preview;
}

function shouldLogVoiceTypingDiagnostics(instanceId: string) {
  return instanceId === 'voice-typing';
}

function isDiagnosticsInstance(instanceId: string): boolean {
  return (
    instanceId === 'record' ||
    instanceId === 'voice-typing' ||
    instanceId === 'caption'
  );
}

function formatSession(sessionId: string | null | undefined): string {
  return sessionId ?? 'none';
}

export class RecognizerLifecycle {
  private isRunning = false;

  constructor(private readonly instanceId: string) {}

  get running(): boolean {
    return this.isRunning;
  }

  registerCallback(
    onUpdate: RecognizerUpdateCallback,
    onError: RecognizerErrorCallback,
    options?: RecognizerCallbackOptions,
  ): void {
    const existingRegistration = instanceCallbacks.get(this.instanceId);
    if (
      isDiagnosticsInstance(this.instanceId) &&
      existingRegistration
    ) {
      logger.info(
        `[TranscriptionService:${this.instanceId}] Replacing callback registration. previous_registration=${existingRegistration.registrationId} previous_owner=${existingRegistration.owner} previous_session=${formatSession(existingRegistration.sessionId)}`
      );
    }

    const owner = options?.owner ?? this.instanceId;
    const sessionId = options?.sessionId ?? null;
    const registrationId = ++callbackRegistrationCounter;
    const wrappedOnUpdate: RecognizerUpdateCallback = (update) => {
      const currentRegistration = instanceCallbacks.get(this.instanceId);
      if (!currentRegistration || currentRegistration.registrationId !== registrationId) {
        if (shouldLogVoiceTypingDiagnostics(this.instanceId)) {
          logger.info(
            '[TranscriptionService:voice-typing] Skipped callback invocation',
            {
              instanceId: this.instanceId,
              registrationId,
              currentRegistrationId: currentRegistration?.registrationId ?? null,
              segmentIds: update.upsertSegments.map((segment) => segment.id),
              removeIds: update.removeIds,
              rawTextLength: null,
              processedTextLength: update.upsertSegments.reduce((sum, segment) => sum + segment.text.length, 0),
              preview: previewTextForLog(update.upsertSegments.map((segment) => segment.text).join(' ')),
              callbackInvoked: false,
              dropReason: 'stale_registration',
            }
          );
        }
        if (isDiagnosticsInstance(this.instanceId)) {
          logger.info(
            `[TranscriptionService:${this.instanceId}] Ignored stale callback invocation. registration=${registrationId} owner=${owner} session=${formatSession(sessionId)} current_registration=${currentRegistration?.registrationId ?? 'none'}`
          );
        }
        return;
      }

      if (shouldLogVoiceTypingDiagnostics(this.instanceId)) {
        logger.info(
          '[TranscriptionService:voice-typing] Invoking callback',
          {
            instanceId: this.instanceId,
            registrationId,
            segmentIds: update.upsertSegments.map((segment) => segment.id),
            removeIds: update.removeIds,
            rawTextLength: null,
            processedTextLength: update.upsertSegments.reduce((sum, segment) => sum + segment.text.length, 0),
            preview: previewTextForLog(update.upsertSegments.map((segment) => segment.text).join(' ')),
            callbackInvoked: true,
          }
        );
      }
      onUpdate(update);
    };

    instanceCallbacks.set(this.instanceId, {
      onUpdate: wrappedOnUpdate,
      onError,
      owner,
      sessionId,
      registrationId
    });

    if (isDiagnosticsInstance(this.instanceId)) {
      logger.info(
        `[TranscriptionService:${this.instanceId}] Registered callback. registration=${registrationId} owner=${owner} session=${formatSession(sessionId)}`
      );
    }
  }

  hasCallbackRegistration(): boolean {
    return instanceCallbacks.has(this.instanceId);
  }

  async ensureGlobalBus(): Promise<void> {
    if (globalListeners.has(this.instanceId)) return;

    const eventName = buildRecognizerOutputEvent(this.instanceId);
    const unlisten = await listen<TranscriptUpdate>(eventName, (event) => {
      const update = normalizeTranscriptUpdate(event.payload);
      const instance = instanceCallbacks.get(this.instanceId);
      if (!instance) {
        if (isDiagnosticsInstance(this.instanceId)) {
          logger.info(
            `[TranscriptionService:${this.instanceId}] Received recognizer event without an active callback. removes=${update.removeIds.length} upserts=${update.upsertSegments.length}`
          );
        }
        return;
      }

      if (isDiagnosticsInstance(this.instanceId)) {
        logger.info(
          `[TranscriptionService:${this.instanceId}] Received recognizer event. registration=${instance.registrationId} owner=${instance.owner} session=${formatSession(instance.sessionId)} removes=${update.removeIds.length} upserts=${update.upsertSegments.length}`
        );
      }

      try {
        for (const segment of update.upsertSegments) {
          if (shouldLogVoiceTypingDiagnostics(this.instanceId)) {
            logger.info(
              '[TranscriptionService:voice-typing] Prepared recognizer segment for callback',
              {
                instanceId: this.instanceId,
                registrationId: instance.registrationId,
                segmentId: segment.id,
                isFinal: segment.isFinal,
                rawTextLength: segment.text.length,
                processedTextLength: segment.text.length,
                preview: previewTextForLog(segment.text),
                callbackInvoked: false,
              }
            );
          }
        }

        instance.onUpdate(update);
      } catch (e) {
        logger.error(`[TranscriptionService:BUS] Error in ${this.instanceId} callback:`, e);
      }
    });

    globalListeners.set(this.instanceId, unlisten);
  }

  async start(onError: RecognizerErrorCallback): Promise<void> {
    if (this.isRunning) {
      return;
    }
    try {
      await startRecognizer(this.instanceId);
      this.isRunning = true;
    } catch (error) {
      logger.error(`[TranscriptionService:${this.instanceId}] Failed to start stream:`, error);
      onError(`Failed to start stream: ${error}`);
      this.isRunning = false;
      throw error;
    }
  }

  async stop(): Promise<void> {
    if (!this.isRunning) return;
    try {
      await stopRecognizer(this.instanceId);
    } finally {
      this.isRunning = false;
    }
  }

  async flushAndStop(): Promise<void> {
    if (this.isRunning) {
      try {
        await flushRecognizer(this.instanceId);
      } catch (error) {
        logger.error('Flush failed:', error);
      }
    }
    await this.stop();
  }

  async feedAudioInt16(samples: Int16Array): Promise<void> {
    if (!this.isRunning) return;
    try {
      const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
      await feedAudioChunk(this.instanceId, bytes);
    } catch (error) {
      logger.error('Feed audio failed:', error);
    }
  }
}

export function resetRecognizerLifecycleForTest(): void {
  for (const unlisten of globalListeners.values()) {
    unlisten();
  }
  globalListeners.clear();
  instanceCallbacks.clear();
  callbackRegistrationCounter = 0;
}
