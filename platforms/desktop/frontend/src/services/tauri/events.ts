type DeepValueOf<T> = T extends string
  ? T
  : {
      [K in keyof T]: DeepValueOf<T[K]>;
    }[keyof T];

export const TauriEvent = {
  app: {
    downloadProgress: 'download-progress',
    extractProgress: 'extract-progress',
    batchProgress: 'batch-progress',
  },
  audio: {
    microphonePeak: 'microphone-audio',
    systemPeak: 'system-audio',
    captureError: 'audio-capture-error',
  },
  tray: {
    openSettings: 'open-settings',
    toggleCaption: 'toggle-caption',
    checkUpdates: 'check-updates',
    requestQuit: 'request-quit',
    stopRecording: 'tray-stop-recording',
  },
  storage: {
    migrationProgress: 'storage-migration-progress',
  },
  automation: {
    runtimeCandidate: 'automation-runtime-candidate',
  },
  taskLedger: {
    updated: 'task-ledger-updated',
  },
  llm: {
    taskProgress: 'llm-task-progress',
    taskChunk: 'llm-task-chunk',
    taskText: 'llm-task-text',
    transcriptJobUpdate: 'llm-transcript-job-update',
    usageRecorded: 'llm-usage-recorded',
  },
  auxWindow: {
    captionState: 'caption:state',
    voiceTypingText: 'voice-typing:text',
    voiceTypingCancel: 'voice-typing:cancel',
    voiceTypingReinject: 'voice-typing:reinject',
  },
  agent: {
    recordingStatus: 'agent-control-recording-status',
    transcriptUpdated: 'transcript-updated',
  },
} as const;

export interface AgentRecordingStatusPayload {
  active: boolean;
  historyId?: string;
}

export interface TranscriptUpdatedPayload {
  historyId: string;
}

export function buildRecognizerOutputEvent(instanceId: string): `recognizer-output-${string}` {
  return `recognizer-output-${instanceId}`;
}

export type TauriEventName = DeepValueOf<typeof TauriEvent> | `recognizer-output-${string}`;
