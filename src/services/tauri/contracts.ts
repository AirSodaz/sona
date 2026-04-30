import type {
  BackupManifestV1,
  BackupWebDavConfig,
  BackupWebDavTestResult,
  RemoteBackupEntry,
} from '../../types/backup';
import type { LlmGenerateCommandRequest } from '../../types/dashboard';
import type { HistoryItem } from '../../types/history';
import type { RuntimeEnvironmentStatus, RuntimePathStatus } from '../../types/runtime';
import type { SpeakerProfileSample, SpeakerProcessingConfig } from '../../types/speaker';
import type { HistorySummaryPayload, TranscriptSegment } from '../../types/transcript';
import type {
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from '../../types/transcriptSnapshot';
import type {
  PolishedSegment,
  PolishSegmentsRequest,
  SummarizeTranscriptRequest,
  TranscriptSummaryResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from '../llmTaskService';
import type { ModelFileConfig } from '../modelService';
import { TauriCommand, type TauriCommandName } from './commands';

type AudioDevice = {
  name: string;
};

type ExtractTarBz2Args = {
  archivePath: string;
  targetDir: string;
};

type DownloadFileArgs = {
  url: string;
  outputPath: string;
  id: string;
};

type UpdateTrayMenuArgs = {
  showText: string;
  settingsText: string;
  updatesText: string;
  quitText: string;
  captionText: string;
  captionChecked: boolean;
};

type StartAudioCaptureArgs = {
  deviceName: string | null;
  instanceId: string;
  outputPath?: string;
};

type SetCapturePausedArgs = {
  instanceId: string;
  paused: boolean;
};

type HistoryDraftTransportHandle = {
  item: HistoryItem;
  audioAbsolutePath: string;
};

type ExportBackupArchiveRequest = {
  archivePath: string;
  appVersion: string;
  config: unknown;
  projects: unknown[];
  automationRules: unknown[];
  automationProcessedEntries: unknown[];
  analyticsContent: string;
};

type ListLlmModelsRequest = {
  provider: string;
  baseUrl?: string;
  apiKey?: string;
};

export type TauriCommandContractMap = {
  [TauriCommand.app.extractTarBz2]: {
    args: ExtractTarBz2Args;
    result: void;
  };
  [TauriCommand.app.downloadFile]: {
    args: DownloadFileArgs;
    result: void;
  };
  [TauriCommand.app.cancelDownload]: {
    args: { id: string };
    result: void;
  };
  [TauriCommand.app.openLogFolder]: {
    args: undefined;
    result: void;
  };
  [TauriCommand.app.getRuntimeEnvironmentStatus]: {
    args: undefined;
    result: RuntimeEnvironmentStatus;
  };
  [TauriCommand.app.getPathStatuses]: {
    args: { paths: string[] };
    result: RuntimePathStatus[];
  };
  [TauriCommand.app.hasActiveDownloads]: {
    args: undefined;
    result: boolean;
  };
  [TauriCommand.app.forceExit]: {
    args: undefined;
    result: void;
  };
  [TauriCommand.app.updateTrayMenu]: {
    args: UpdateTrayMenuArgs;
    result: void;
  };
  [TauriCommand.app.setMinimizeToTray]: {
    args: { enabled: boolean };
    result: void;
  };
  [TauriCommand.audio.setSystemAudioMute]: {
    args: { mute: boolean };
    result: void;
  };
  [TauriCommand.audio.getSystemAudioDevices]: {
    args: undefined;
    result: AudioDevice[];
  };
  [TauriCommand.audio.startSystemAudioCapture]: {
    args: StartAudioCaptureArgs;
    result: void;
  };
  [TauriCommand.audio.stopSystemAudioCapture]: {
    args: { instanceId: string };
    result: string;
  };
  [TauriCommand.audio.setSystemAudioCapturePaused]: {
    args: SetCapturePausedArgs;
    result: void;
  };
  [TauriCommand.audio.setMicrophoneBoost]: {
    args: { boost: number };
    result: void;
  };
  [TauriCommand.audio.getMicrophoneDevices]: {
    args: undefined;
    result: AudioDevice[];
  };
  [TauriCommand.audio.startMicrophoneCapture]: {
    args: StartAudioCaptureArgs;
    result: void;
  };
  [TauriCommand.audio.stopMicrophoneCapture]: {
    args: { instanceId: string };
    result: string;
  };
  [TauriCommand.audio.setMicrophoneCapturePaused]: {
    args: SetCapturePausedArgs;
    result: void;
  };
  [TauriCommand.history.listItems]: {
    args: undefined;
    result: Partial<HistoryItem>[];
  };
  [TauriCommand.history.createLiveDraft]: {
    args: { item: HistoryItem };
    result: HistoryDraftTransportHandle;
  };
  [TauriCommand.history.completeLiveDraft]: {
    args: {
      historyId: string;
      segments: TranscriptSegment[];
      previewText: string;
      searchContent: string;
      duration: number;
    };
    result: Partial<HistoryItem>;
  };
  [TauriCommand.history.saveRecording]: {
    args: {
      item: Partial<HistoryItem>;
      segments: TranscriptSegment[];
      nativeAudioPath?: string;
      audioBytes?: number[];
    };
    result: Partial<HistoryItem>;
  };
  [TauriCommand.history.saveImportedFile]: {
    args: {
      item: Partial<HistoryItem>;
      segments: TranscriptSegment[];
      sourcePath: string;
    };
    result: Partial<HistoryItem>;
  };
  [TauriCommand.history.deleteItems]: {
    args: { ids: string[] };
    result: void;
  };
  [TauriCommand.history.loadTranscript]: {
    args: { filename: string };
    result: unknown;
  };
  [TauriCommand.history.updateTranscript]: {
    args: {
      historyId: string;
      segments: TranscriptSegment[];
      previewText: string;
      searchContent: string;
    };
    result: void;
  };
  [TauriCommand.history.createTranscriptSnapshot]: {
    args: {
      historyId: string;
      reason: TranscriptSnapshotReason;
      segments: TranscriptSegment[];
    };
    result: TranscriptSnapshotMetadata;
  };
  [TauriCommand.history.listTranscriptSnapshots]: {
    args: {
      historyId: string;
    };
    result: TranscriptSnapshotMetadata[];
  };
  [TauriCommand.history.loadTranscriptSnapshot]: {
    args: {
      historyId: string;
      snapshotId: string;
    };
    result: TranscriptSnapshotRecord | null;
  };
  [TauriCommand.history.updateItemMeta]: {
    args: {
      historyId: string;
      updates: Partial<HistoryItem>;
    };
    result: void;
  };
  [TauriCommand.history.updateProjectAssignments]: {
    args: {
      ids: string[];
      projectId: string | null;
    };
    result: void;
  };
  [TauriCommand.history.reassignProject]: {
    args: {
      currentProjectId: string;
      nextProjectId: string | null;
    };
    result: void;
  };
  [TauriCommand.history.loadSummary]: {
    args: { historyId: string };
    result: HistorySummaryPayload | null;
  };
  [TauriCommand.history.saveSummary]: {
    args: {
      historyId: string;
      summaryPayload: HistorySummaryPayload;
    };
    result: void;
  };
  [TauriCommand.history.deleteSummary]: {
    args: { historyId: string };
    result: void;
  };
  [TauriCommand.history.resolveAudioPath]: {
    args: { filename: string };
    result: string | null;
  };
  [TauriCommand.history.openFolder]: {
    args: undefined;
    result: void;
  };
  [TauriCommand.llm.generateText]: {
    args: { request: LlmGenerateCommandRequest };
    result: string;
  };
  [TauriCommand.llm.listModels]: {
    args: { request: ListLlmModelsRequest };
    result: string[];
  };
  [TauriCommand.llm.polishTranscriptSegments]: {
    args: { request: PolishSegmentsRequest };
    result: PolishedSegment[];
  };
  [TauriCommand.llm.summarizeTranscript]: {
    args: { request: SummarizeTranscriptRequest };
    result: TranscriptSummaryResult;
  };
  [TauriCommand.llm.translateTranscriptSegments]: {
    args: { request: TranslateSegmentsRequest };
    result: TranslatedSegment[];
  };
  [TauriCommand.recognizer.init]: {
    args: {
      instanceId: string;
      modelPath: string;
      numThreads: number;
      enableItn: boolean;
      language: string;
      punctuationModel: string | null;
      vadModel: string | null;
      vadBuffer: number;
      modelType: string;
      fileConfig?: ModelFileConfig;
      hotwords: string | null;
      normalizationOptions: {
        enableTimeline: boolean;
      };
    };
    result: void;
  };
  [TauriCommand.recognizer.start]: {
    args: { instanceId: string };
    result: void;
  };
  [TauriCommand.recognizer.stop]: {
    args: { instanceId: string };
    result: void;
  };
  [TauriCommand.recognizer.flush]: {
    args: { instanceId: string };
    result: void;
  };
  [TauriCommand.recognizer.feedAudioChunk]: {
    args: {
      instanceId: string;
      samples: Uint8Array;
    };
    result: void;
  };
  [TauriCommand.recognizer.processBatchFile]: {
    args: {
      filePath: string;
      saveToPath: string | null;
      modelPath: string;
      numThreads: number;
      enableItn: boolean;
      language: string;
      punctuationModel: string | null;
      vadModel: string | null;
      vadBuffer: number;
      modelType: string;
      fileConfig?: ModelFileConfig;
      hotwords: string | null;
      speakerProcessing: SpeakerProcessingConfig | null;
      normalizationOptions: {
        enableTimeline: boolean;
      };
    };
    result: TranscriptSegment[];
  };
  [TauriCommand.automation.replaceRuntimeRules]: {
    args: { rules: unknown[] };
    result: unknown;
  };
  [TauriCommand.automation.scanRuntimeRule]: {
    args: { rule: unknown };
    result: void;
  };
  [TauriCommand.automation.collectRuntimeRulePaths]: {
    args: {
      rule: unknown;
      filePaths: string[];
    };
    result: unknown;
  };
  [TauriCommand.backup.exportArchive]: {
    args: { request: ExportBackupArchiveRequest };
    result: BackupManifestV1;
  };
  [TauriCommand.backup.prepareImport]: {
    args: { archivePath: string };
    result: unknown;
  };
  [TauriCommand.backup.applyPreparedImport]: {
    args: { importId: string };
    result: void;
  };
  [TauriCommand.backup.disposePreparedImport]: {
    args: { importId: string };
    result: void;
  };
  [TauriCommand.backup.webdavTestConnection]: {
    args: { config: BackupWebDavConfig };
    result: BackupWebDavTestResult;
  };
  [TauriCommand.backup.webdavListBackups]: {
    args: { config: BackupWebDavConfig };
    result: RemoteBackupEntry[];
  };
  [TauriCommand.backup.webdavUploadBackup]: {
    args: {
      config: BackupWebDavConfig;
      localArchivePath: string;
    };
    result: void;
  };
  [TauriCommand.backup.webdavDownloadBackup]: {
    args: {
      config: BackupWebDavConfig;
      href: string;
      outputPath: string;
    };
    result: void;
  };
  [TauriCommand.speaker.annotateSegmentsFromFile]: {
    args: {
      filePath: string;
      segments: TranscriptSegment[];
      speakerProcessing: SpeakerProcessingConfig;
    };
    result: TranscriptSegment[];
  };
  [TauriCommand.speaker.importProfileSample]: {
    args: {
      profileId: string;
      sourcePath: string;
      sourceName?: string | null;
    };
    result: SpeakerProfileSample;
  };
  [TauriCommand.system.setAuxWindowState]: {
    args: {
      label: string;
      payload: unknown;
    };
    result: void;
  };
  [TauriCommand.system.getAuxWindowState]: {
    args: { label: string };
    result: unknown | null;
  };
  [TauriCommand.system.clearAuxWindowState]: {
    args: { label: string };
    result: void;
  };
  [TauriCommand.system.injectText]: {
    args: {
      text: string;
      shortcutModifiers?: string[];
    };
    result: void;
  };
  [TauriCommand.system.getMousePosition]: {
    args: undefined;
    result: [number, number];
  };
  [TauriCommand.system.getTextCursorPosition]: {
    args: undefined;
    result: [number, number] | null;
  };
};

type Assert<T extends true> = T;
export type TauriAllCommandsCovered = Assert<
  Exclude<TauriCommandName, keyof TauriCommandContractMap> extends never ? true : false
>;
export type TauriNoExtraContracts = Assert<
  Exclude<keyof TauriCommandContractMap, TauriCommandName> extends never ? true : false
>;

export type KnownTauriCommandName = keyof TauriCommandContractMap;
export type TauriCommandArgs<TCommand extends KnownTauriCommandName> =
  TauriCommandContractMap[TCommand]['args'];
export type TauriCommandResult<TCommand extends KnownTauriCommandName> =
  TauriCommandContractMap[TCommand]['result'];
export type TauriCommandsWithArgs = {
  [TCommand in KnownTauriCommandName]:
    TauriCommandArgs<TCommand> extends undefined ? never : TCommand;
}[KnownTauriCommandName];
export type TauriCommandsWithoutArgs = Exclude<KnownTauriCommandName, TauriCommandsWithArgs>;
