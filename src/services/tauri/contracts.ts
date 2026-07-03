import type {
  BackupManifestV1,
  BackupWebDavConfig,
  BackupWebDavTestResult,
  RemoteBackupEntry,
} from "../../types/backup";
import type {
  DashboardSnapshot,
  LlmGenerateCommandRequest,
} from "../../types/dashboard";
import type { HistoryItem } from "../../types/history";
import type {
  AsrRuntimeMetricsSnapshot,
  RuntimeEnvironmentStatus,
  RuntimePathStatus,
} from "../../types/runtime";
import type {
  AppConfig,
  AppLogLevel,
  TextReplacementRuleSet,
} from "../../types/config";
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuleValidationResult,
} from "../../types/automation";
import type { ProjectDefaults, ProjectRecord } from "../../types/project";
import type {
  SpeakerProfileSample,
  SpeakerProcessingConfig,
} from "../../types/speaker";
import type {
  HistorySummaryPayload,
  TranscriptSegment,
} from "../../types/transcript";
import type {
  TranscriptDiffRow,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from "../../types/transcriptSnapshot";
import type { BatchQueueItem } from "../../types/batchQueue";
import type {
  RecoveredQueueItem,
  RecoverySnapshot,
} from "../../types/recovery";
import type {
  TaskLedgerPatch,
  TaskLedgerRecord,
  TaskLedgerSnapshot,
} from "../../types/taskLedger";
import type {
  PolishedSegment,
  PolishSegmentsRequest,
  SummarizeTranscriptRequest,
  TranscriptLlmJobRequest,
  TranscriptLlmJobResult,
  TranscriptSummaryResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from "../llmTaskTypes";
import type { ModelFileConfig } from "../../types/model";
import type { ModelCatalogSnapshot } from "../modelService";
import type {
  DiagnosticsCoreInput,
  DiagnosticsCoreFactsSnapshot,
} from "../diagnosticsSnapshotBuilders";
import type {
  ApplySpeakerProfileToGroupRequest,
  SpeakerCorrectionResponse,
  SpeakerGroupRequest,
} from "../speakerCorrectionService";
import type {
  SpeakerReviewFilter,
  SpeakerReviewSnapshot,
} from "../speakerReviewService";
import { TauriCommand, type TauriCommandName } from "./commands";

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
  expectedSha256?: string;
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

type TranscriptDiffResult = {
  rows: TranscriptDiffRow[];
  changedCount: number;
};

type TranscriptPostprocessOptions = {
  textReplacementSets?: TextReplacementRuleSet[];
  dropFinalDotSegments?: boolean;
};

type AsrTranscriptionRequestBase = {
  mode: "streaming" | "offline" | "batch";
  language: string;
  enableItn: boolean;
  normalizationOptions: {
    enableTimeline: boolean;
  };
  postprocessOptions: TranscriptPostprocessOptions;
};

type LocalSherpaAsrRequest = AsrTranscriptionRequestBase & {
  engine: "local-sherpa";
  modelId?: string | null;
  modelPath: string;
  numThreads: number;
  punctuationModel: string | null;
  vadModel: string | null;
  vadBuffer: number;
  batchSegmentationMode?: "vad" | "whole";
  modelType: string;
  fileConfig?: ModelFileConfig;
  hotwords: string | null;
};

type OnlineAsrRequest = AsrTranscriptionRequestBase & {
  engine: "online";
  onlineProvider: {
    providerId: string;
    profileId: string;
    config: unknown;
  };
};

type AsrTranscriptionRequest = LocalSherpaAsrRequest | OnlineAsrRequest;

type WorkspaceQueryScope =
  { kind: "all" } | { kind: "inbox" } | { kind: "project"; projectId: string };

type WorkspaceQueryArgs = {
  scope: WorkspaceQueryScope;
  query: string;
  filterType: "all" | "recording" | "batch";
  dateFilter: "all" | "today" | "week" | "month";
  sortOrder:
    "newest" | "oldest" | "duration_desc" | "duration_asc" | "title_asc";
};

type WorkspaceSearchRange = {
  start: number;
  end: number;
};

type WorkspaceSearchSnippet = {
  text: string;
  highlightStart: number;
  highlightEnd: number;
};

type WorkspaceItemSearchMatch = {
  matchedField: "title" | "previewText" | "searchContent";
  titleMatch: WorkspaceSearchRange | null;
  displaySnippet: WorkspaceSearchSnippet;
};

type WorkspaceQueryResult = {
  filteredItems: HistoryItem[];
  scopedItems: HistoryItem[];
  scopedItemIds: string[];
  searchMatchByItemId: Record<string, WorkspaceItemSearchMatch | null>;
  summary: {
    totalItems: number;
    totalDuration: number;
    latestTimestamp: number | null;
    recordingCount: number;
    batchCount: number;
  };
  itemCounts: {
    inbox: number;
    byProjectId: Record<string, number>;
  };
};

type ExportTranscriptFileArgs = {
  segments: TranscriptSegment[];
  format: "srt" | "json" | "txt" | "vtt" | "md";
  mode: "original" | "translation" | "bilingual";
  outputPath: string;
};

type ExportTranscriptFileResult = {
  outputPath: string;
  bytesWritten: number;
};

type ProjectListArgs = {
  fallbackEnabledPolishKeywordSetIds?: string[];
  fallbackEnabledSpeakerProfileIds?: string[];
};

type ProjectCreateArgs = {
  name: string;
  description?: string;
  icon?: string;
  defaults: ProjectDefaults;
};

type ProjectUpdateArgs = {
  projectId: string;
  updates: Partial<
    Pick<ProjectRecord, "name" | "description" | "icon" | "defaults">
  >;
};

type AutomationRepositoryState = {
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
};

type AutomationValidateActivationArgs = {
  rule: AutomationRule;
  globalConfig: AppConfig;
  project: ProjectRecord | null;
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
  strategy?: string;
  baseUrl?: string;
  apiKey?: string;
};

type LlmModelSummary = {
  model: string;
  inputPrice?: number;
  outputPrice?: number;
  contextWindow?: number;
  maxOutputTokens?: number;
  supportsMultimodal?: boolean;
  supportsTools?: boolean;
  supportsReasoning?: boolean;
};

export type ModelSelectionPaths = {
  streamingModelPath: string;
  offlineModelPath: string;
  speakerSegmentationModelPath: string;
  speakerEmbeddingModelPath: string;
};

export type ModelCatalogSelectedIds = {
  streaming: string | null;
  offline: string | null;
  speakerSegmentation: string | null;
  speakerEmbedding: string | null;
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
  [TauriCommand.app.getModelCatalogSnapshot]: {
    args: undefined;
    result: ModelCatalogSnapshot;
  };
  [TauriCommand.app.resolveModelCatalogSelectedIds]: {
    args: { paths: ModelSelectionPaths };
    result: ModelCatalogSelectedIds;
  };
  [TauriCommand.app.getDiagnosticsCoreSnapshot]: {
    args: { input: DiagnosticsCoreInput };
    result: DiagnosticsCoreFactsSnapshot;
  };
  [TauriCommand.app.loadAppConfig]: {
    args: undefined;
    result: AppConfig | null;
  };
  [TauriCommand.app.saveAppConfig]: {
    args: { config: AppConfig };
    result: void;
  };
  [TauriCommand.app.getAppSetting]: {
    args: { key: string };
    result: unknown | null;
  };
  [TauriCommand.app.setAppSetting]: {
    args: { key: string; value: unknown };
    result: void;
  };
  [TauriCommand.app.migrateAppConfig]: {
    args: {
      savedConfig: AppConfig | null;
      legacyConfig: unknown;
      defaultRuleSetName: string;
    };
    result: {
      config: AppConfig;
      migrated: boolean;
    };
  };
  [TauriCommand.app.resolveEffectiveConfig]: {
    args: {
      globalConfig: AppConfig;
      project: ProjectRecord | null;
    };
    result: AppConfig;
  };
  [TauriCommand.app.getRuntimeEnvironmentStatus]: {
    args: undefined;
    result: RuntimeEnvironmentStatus;
  };
  [TauriCommand.app.getAsrRuntimeMetrics]: {
    args: undefined;
    result: AsrRuntimeMetricsSnapshot;
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
  [TauriCommand.app.setLogLevel]: {
    args: { level: AppLogLevel };
    result: void;
  };
  [TauriCommand.app.checkMediaFormats]: {
    args: { paths: string[] };
    result: boolean[];
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
    args: { limit?: number | null; offset?: number | null } | undefined;
    result: Partial<HistoryItem>[];
  };
  [TauriCommand.history.createLiveDraft]: {
    args: {
      id?: string | null;
      audioExtension: string;
      projectId: string | null;
      icon: string | null;
    };
    result: HistoryDraftTransportHandle;
  };
  [TauriCommand.history.completeLiveDraft]: {
    args: {
      historyId: string;
      segments: TranscriptSegment[];
      duration: number;
    };
    result: Partial<HistoryItem>;
  };
  [TauriCommand.history.saveRecording]: {
    args: {
      segments: TranscriptSegment[];
      duration: number;
      projectId: string | null;
      nativeAudioPath?: string;
      audioBytes?: number[];
      audioExtension?: string;
    };
    result: Partial<HistoryItem>;
  };
  [TauriCommand.history.saveImportedFile]: {
    args: {
      sourcePath: string;
      segments: TranscriptSegment[];
      duration: number;
      projectId: string | null;
      convertedSourcePath?: string;
      id?: string | null;
    };
    result: Partial<HistoryItem>;
  };
  [TauriCommand.history.deleteItems]: {
    args: { ids: string[] };
    result: void;
  };
  [TauriCommand.history.loadTranscript]: {
    args: { historyId: string };
    result: TranscriptSegment[] | null;
  };
  [TauriCommand.history.updateTranscript]: {
    args: {
      historyId: string;
      segments: TranscriptSegment[];
    };
    result: Partial<HistoryItem>;
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
  [TauriCommand.history.buildTranscriptDiff]: {
    args: {
      snapshotSegments: TranscriptSegment[];
      currentSegments: TranscriptSegment[];
    };
    result: TranscriptDiffResult;
  };
  [TauriCommand.history.restoreTranscriptDiffRows]: {
    args: {
      rows: TranscriptDiffRow[];
      selectedRowIds: string[];
    };
    result: TranscriptSegment[];
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
    args: { historyId: string };
    result: string | null;
  };
  [TauriCommand.history.queryWorkspace]: {
    args: WorkspaceQueryArgs;
    result: WorkspaceQueryResult;
  };
  [TauriCommand.history.openFolder]: {
    args: undefined;
    result: void;
  };
  [TauriCommand.dashboard.getSnapshot]: {
    args: { request: { deep: boolean } };
    result: DashboardSnapshot;
  };
  [TauriCommand.export.transcriptFile]: {
    args: ExportTranscriptFileArgs;
    result: ExportTranscriptFileResult;
  };
  [TauriCommand.project.list]: {
    args: ProjectListArgs;
    result: ProjectRecord[];
  };
  [TauriCommand.project.saveAll]: {
    args: { projects: ProjectRecord[] };
    result: void;
  };
  [TauriCommand.project.create]: {
    args: ProjectCreateArgs;
    result: ProjectRecord;
  };
  [TauriCommand.project.update]: {
    args: ProjectUpdateArgs;
    result: ProjectRecord | null;
  };
  [TauriCommand.project.delete]: {
    args: { projectId: string };
    result: void;
  };
  [TauriCommand.project.reorder]: {
    args: { projectIds: string[] };
    result: ProjectRecord[];
  };
  [TauriCommand.project.getActiveId]: {
    args: undefined;
    result: string | null;
  };
  [TauriCommand.project.setActiveId]: {
    args: { projectId: string | null };
    result: void;
  };
  [TauriCommand.automationRepository.loadState]: {
    args: undefined;
    result: AutomationRepositoryState;
  };
  [TauriCommand.automationRepository.persistRules]: {
    args: { rules: AutomationRule[] };
    result: void;
  };
  [TauriCommand.automationRepository.persistProcessedEntries]: {
    args: { processedEntries: AutomationProcessedEntry[] };
    result: void;
  };
  [TauriCommand.automationRepository.persistState]: {
    args: AutomationRepositoryState;
    result: void;
  };
  [TauriCommand.automationRepository.validateActivation]: {
    args: AutomationValidateActivationArgs;
    result: AutomationRuleValidationResult;
  };
  [TauriCommand.llmUsage.ensureStorage]: {
    args: undefined;
    result: void;
  };
  [TauriCommand.llmUsage.readRaw]: {
    args: undefined;
    result: string;
  };
  [TauriCommand.llmUsage.replaceRaw]: {
    args: { content: string };
    result: void;
  };
  [TauriCommand.llm.generateText]: {
    args: { request: LlmGenerateCommandRequest };
    result: string;
  };
  [TauriCommand.llm.listModels]: {
    args: { request: ListLlmModelsRequest };
    result: LlmModelSummary[];
  };
  [TauriCommand.llm.polishTranscriptSegments]: {
    args: { request: PolishSegmentsRequest };
    result: PolishedSegment[];
  };
  [TauriCommand.llm.runTranscriptJob]: {
    args: { request: TranscriptLlmJobRequest };
    result: TranscriptLlmJobResult;
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
      asrRequest: AsrTranscriptionRequest;
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
      speakerProcessing: SpeakerProcessingConfig | null;
      asrRequest: AsrTranscriptionRequest;
    };
    result: TranscriptSegment[];
  };
  [TauriCommand.recovery.loadSnapshot]: {
    args: undefined;
    result: RecoverySnapshot;
  };
  [TauriCommand.recovery.saveSnapshot]: {
    args: { items: RecoveredQueueItem[] };
    result: RecoverySnapshot;
  };
  [TauriCommand.recovery.persistQueueSnapshot]: {
    args: { queueItems: BatchQueueItem[]; resolvedIds?: string[] };
    result: void;
  };
  [TauriCommand.taskLedger.loadSnapshot]: {
    args: undefined;
    result: TaskLedgerSnapshot;
  };
  [TauriCommand.taskLedger.upsertTask]: {
    args: { record: TaskLedgerRecord };
    result: TaskLedgerSnapshot;
  };
  [TauriCommand.taskLedger.patchTask]: {
    args: { id: string; patch: TaskLedgerPatch };
    result: TaskLedgerSnapshot;
  };
  [TauriCommand.taskLedger.removeTask]: {
    args: { id: string };
    result: TaskLedgerSnapshot;
  };
  [TauriCommand.taskLedger.clearResolved]: {
    args: undefined;
    result: TaskLedgerSnapshot;
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
  [TauriCommand.speaker.buildReviewSnapshot]: {
    args: {
      segments: TranscriptSegment[];
      activeFilter: SpeakerReviewFilter;
    };
    result: SpeakerReviewSnapshot;
  };
  [TauriCommand.speaker.applyProfileToGroup]: {
    args: { request: ApplySpeakerProfileToGroupRequest };
    result: SpeakerCorrectionResponse;
  };
  [TauriCommand.speaker.resetGroupToAnonymous]: {
    args: { request: SpeakerGroupRequest };
    result: SpeakerCorrectionResponse;
  };
  [TauriCommand.speaker.confirmGroupReview]: {
    args: { request: SpeakerGroupRequest };
    result: SpeakerCorrectionResponse;
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
  [TauriCommand.apiServer.start]: {
    args: {
      host: string;
      port: number;
      apiKey: string;
      maxConcurrent: number;
      maxQueueSize: number;
      maxUploadSizeMb: number;
      jobTtlMinutes: number;
      maxStreaming: number;
      ipWhitelist: string;
      gpuAcceleration: "auto" | "cpu" | "cuda" | "coreml" | "directml";
    };
    result: string;
  };
  [TauriCommand.apiServer.stop]: {
    args: undefined;
    result: void;
  };
};

type Assert<T extends true> = T;
export type TauriAllCommandsCovered = Assert<
  Exclude<TauriCommandName, keyof TauriCommandContractMap> extends never
    ? true
    : false
>;
export type TauriNoExtraContracts = Assert<
  Exclude<keyof TauriCommandContractMap, TauriCommandName> extends never
    ? true
    : false
>;

export type KnownTauriCommandName = keyof TauriCommandContractMap;
export type TauriCommandArgs<TCommand extends KnownTauriCommandName> =
  TauriCommandContractMap[TCommand]["args"];
export type TauriCommandResult<TCommand extends KnownTauriCommandName> =
  TauriCommandContractMap[TCommand]["result"];
export type TauriCommandsWithArgs = {
  [
    TCommand in KnownTauriCommandName
  ]: TauriCommandArgs<TCommand> extends undefined ? never : TCommand;
}[KnownTauriCommandName];
export type TauriCommandsWithoutArgs = Exclude<
  KnownTauriCommandName,
  TauriCommandsWithArgs
>;
