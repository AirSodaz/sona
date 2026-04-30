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
  },
  tray: {
    openSettings: 'open-settings',
    toggleCaption: 'toggle-caption',
    checkUpdates: 'check-updates',
    requestQuit: 'request-quit',
  },
  automation: {
    runtimeCandidate: 'automation-runtime-candidate',
  },
  llm: {
    taskProgress: 'llm-task-progress',
    taskChunk: 'llm-task-chunk',
    taskText: 'llm-task-text',
    usageRecorded: 'llm-usage-recorded',
  },
  auxWindow: {
    captionState: 'caption:state',
    voiceTypingText: 'voice-typing:text',
  },
} as const;

export function buildRecognizerOutputEvent(instanceId: string): `recognizer-output-${string}` {
  return `recognizer-output-${instanceId}`;
}

export type TauriEventName =
  | DeepValueOf<typeof TauriEvent>
  | `recognizer-output-${string}`;
