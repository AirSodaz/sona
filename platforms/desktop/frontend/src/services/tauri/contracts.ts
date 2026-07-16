import type { BackupManifestV1, PreparedBackupImport } from "../../types/backup";
import type {
  DashboardSnapshot,
  LlmGenerateCommandRequest,
} from "../../types/dashboard";
import type {
  AutomationProcessedInput_Serialize,
  AutomationRepositoryState_Serialize,
  AutomationRule,
  AutomationRuleInput_Serialize,
  AutomationRuleValidationResult_Serialize,
  AutomationRuntimePathCollectionResult,
  AutomationRuntimeReplaceResult,
  AutomationRuntimeRuleConfig,
  DiagnosticsCoreInput,
  DiagnosticsCoreSnapshot,
  ExportTranscriptFileRequest_Serialize,
  ExportTranscriptFileResult,
  HistoryAudioCleanupReport,
  HistoryAudioCleanupRequest_Serialize,
  HistoryCompleteLiveDraftRequest_Serialize,
  HistoryCreateLiveDraftRequest,
  HistoryCreateTranscriptSnapshotRequest_Serialize,
  HistoryDeleteItemsRequest,
  HistoryItemRecord,
  HistoryReassignProjectRequest,
  HistorySaveImportedFileRequest_Serialize,
  HistorySaveRecordingRequest_Serialize,
  HistorySummaryPayload_Serialize,
  HistoryUpdateItemMetaRequest_Serialize,
  HistoryUpdateProjectAssignmentsRequest,
  HistoryUpdateTranscriptRequest_Serialize,
  HistoryWorkspaceQueryRequest,
  HistoryWorkspaceQueryResult,
  LiveRecordingDraftResult,
  ModelCatalogSelectedIds as CoreModelCatalogSelectedIds,
  ModelCatalogSnapshot as CoreModelCatalogSnapshot,
  ModelSelectionPaths as CoreModelSelectionPaths,
  RecoveryItemInput_Serialize,
  RecoverySnapshot_Serialize,
  StorageUsageSnapshot_Serialize,
  TranscriptDiffResult_Serialize,
  TranscriptDiffRow_Serialize,
  TranscriptSegment_Serialize,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotRecord_Serialize,
  WebviewBrowsingDataClearResult,
} from "../../bindings";
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
  ProjectCreateInput,
  ProjectRecord,
  ProjectUpdateInput,
} from "../../types/project";
import type {
  SpeakerProfileSample,
  SpeakerProcessingConfig,
} from "../../types/speaker";
import type { TranscriptSegment } from "../../types/transcript";
import type {
  LlmCompletionRequest,
  LlmCompletionResponse,
  LlmConfig,
  LlmDiscoveredModelSummary,
} from "../../types/llm";
import type {
  TaskLedgerPatch,
  TaskLedgerRecord,
  TaskLedgerSnapshot,
} from "../../types/taskLedger";
import type { ApiServerDashboardSnapshot } from "../../types/apiServer";
import type {
  SyncChangePasswordRequest,
  SyncConflictDetail,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncCreateRequest,
  SyncCreateResult,
  SyncJoinPreview,
  SyncJoinRequest,
  SyncPresetV1,
  SyncPreviewJoinRequest,
  SyncProviderDescriptor,
  SyncRunResult,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
  WebDavObjectStoreConfig,
  LegacyRemoteBackupListResult,
} from "../../types/sync";
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

type HistoryDraftTransportHandle = LiveRecordingDraftResult;

type TranscriptPostprocessOptions = {
  textReplacementSets?: TextReplacementRuleSet[];
  dropFinalDotSegments?: boolean;
};

type AsrTranscriptionRequestBase = {
  mode: "streaming" | "batch";
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

type ProjectListArgs = {
  fallbackEnabledPolishKeywordSetIds?: string[];
  fallbackEnabledSpeakerProfileIds?: string[];
};

type ProjectCreateArgs = ProjectCreateInput;

type ProjectUpdateArgs = {
  projectId: string;
  updates: ProjectUpdateInput;
};

type AutomationValidateActivationArgs = {
  rule: AutomationRule;
  globalConfig: AppConfig;
  project: ProjectRecord | null;
};

type ExportBackupArchiveRequest = {
  archivePath: string;
  appVersion: string;
};

type ListLlmModelsRequest = {
  provider: string;
  strategy?: string;
  baseUrl: string;
  apiKey: string;
};

export type ModelSelectionPaths = CoreModelSelectionPaths;

export type ModelCatalogSelectedIds = CoreModelCatalogSelectedIds;

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
    result: CoreModelCatalogSnapshot;
  };
  [TauriCommand.app.resolveModelCatalogSelectedIds]: {
    args: { paths: CoreModelSelectionPaths };
    result: CoreModelCatalogSelectedIds;
  };
  [TauriCommand.app.getDiagnosticsCoreSnapshot]: {
    args: { input: DiagnosticsCoreInput };
    result: DiagnosticsCoreSnapshot;
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
    result: HistoryItemRecord[];
  };
  [TauriCommand.history.createLiveDraft]: {
    args: HistoryCreateLiveDraftRequest;
    result: HistoryDraftTransportHandle;
  };
  [TauriCommand.history.completeLiveDraft]: {
    args: HistoryCompleteLiveDraftRequest_Serialize;
    result: HistoryItemRecord;
  };
  [TauriCommand.history.saveRecording]: {
    args: HistorySaveRecordingRequest_Serialize;
    result: HistoryItemRecord;
  };
  [TauriCommand.history.saveImportedFile]: {
    args: HistorySaveImportedFileRequest_Serialize;
    result: HistoryItemRecord;
  };
  [TauriCommand.history.deleteItems]: {
    args: HistoryDeleteItemsRequest;
    result: void;
  };
  [TauriCommand.history.loadTranscript]: {
    args: { historyId: string };
    result: TranscriptSegment_Serialize[] | null;
  };
  [TauriCommand.history.updateTranscript]: {
    args: HistoryUpdateTranscriptRequest_Serialize;
    result: HistoryItemRecord;
  };
  [TauriCommand.history.createTranscriptSnapshot]: {
    args: HistoryCreateTranscriptSnapshotRequest_Serialize;
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
    result: TranscriptSnapshotRecord_Serialize | null;
  };
  [TauriCommand.history.buildTranscriptDiff]: {
    args: {
      snapshotSegments: TranscriptSegment_Serialize[];
      currentSegments: TranscriptSegment_Serialize[];
    };
    result: TranscriptDiffResult_Serialize;
  };
  [TauriCommand.history.restoreTranscriptDiffRows]: {
    args: {
      rows: TranscriptDiffRow_Serialize[];
      selectedRowIds: string[];
    };
    result: TranscriptSegment_Serialize[];
  };
  [TauriCommand.history.updateItemMeta]: {
    args: HistoryUpdateItemMetaRequest_Serialize;
    result: void;
  };
  [TauriCommand.history.updateProjectAssignments]: {
    args: HistoryUpdateProjectAssignmentsRequest;
    result: void;
  };
  [TauriCommand.history.reassignProject]: {
    args: HistoryReassignProjectRequest;
    result: void;
  };
  [TauriCommand.history.loadSummary]: {
    args: { historyId: string };
    result: HistorySummaryPayload_Serialize | null;
  };
  [TauriCommand.history.saveSummary]: {
    args: {
      historyId: string;
      summaryPayload: HistorySummaryPayload_Serialize;
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
  [TauriCommand.history.previewAudioCleanup]: {
    args: HistoryAudioCleanupRequest_Serialize;
    result: HistoryAudioCleanupReport;
  };
  [TauriCommand.history.cleanupAudio]: {
    args: HistoryAudioCleanupRequest_Serialize;
    result: HistoryAudioCleanupReport;
  };
  [TauriCommand.history.queryWorkspace]: {
    args: HistoryWorkspaceQueryRequest;
    result: HistoryWorkspaceQueryResult;
  };
  [TauriCommand.history.openFolder]: {
    args: undefined;
    result: void;
  };
  [TauriCommand.storage.getUsageSnapshot]: {
    args: undefined;
    result: StorageUsageSnapshot_Serialize;
  };
  [TauriCommand.storage.clearWebviewBrowsingData]: {
    args: undefined;
    result: WebviewBrowsingDataClearResult;
  };
  [TauriCommand.dashboard.getSnapshot]: {
    args: { request: { deep: boolean } };
    result: DashboardSnapshot;
  };
  [TauriCommand.export.transcriptFile]: {
    args: ExportTranscriptFileRequest_Serialize;
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
    result: AutomationRepositoryState_Serialize;
  };
  [TauriCommand.automationRepository.persistRules]: {
    args: { rules: AutomationRuleInput_Serialize[] };
    result: void;
  };
  [TauriCommand.automationRepository.persistProcessedEntries]: {
    args: { processedEntries: AutomationProcessedInput_Serialize[] };
    result: void;
  };
  [TauriCommand.automationRepository.persistState]: {
    args: {
      rules: AutomationRuleInput_Serialize[];
      processedEntries: AutomationProcessedInput_Serialize[];
    };
    result: void;
  };
  [TauriCommand.automationRepository.validateActivation]: {
    args: AutomationValidateActivationArgs;
    result: AutomationRuleValidationResult_Serialize;
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
  [TauriCommand.llm.complete]: {
    args: { request: LlmCompletionRequest };
    result: LlmCompletionResponse;
  };
  [TauriCommand.llm.describeModel]: {
    args: { config: LlmConfig };
    result: LlmDiscoveredModelSummary | null;
  };
  [TauriCommand.llm.listModels]: {
    args: { request: ListLlmModelsRequest };
    result: LlmDiscoveredModelSummary[];
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
      instanceId?: string;
    };
    result: TranscriptSegment[];
  };
  [TauriCommand.recovery.loadSnapshot]: {
    args: undefined;
    result: RecoverySnapshot_Serialize;
  };
  [TauriCommand.recovery.saveSnapshot]: {
    args: { items: RecoveryItemInput_Serialize[] };
    result: RecoverySnapshot_Serialize;
  };
  [TauriCommand.recovery.persistQueueSnapshot]: {
    args: { queueItems: RecoveryItemInput_Serialize[]; resolvedIds?: string[] };
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
    args: { rules: AutomationRuntimeRuleConfig[] };
    result: AutomationRuntimeReplaceResult[];
  };
  [TauriCommand.automation.scanRuntimeRule]: {
    args: { rule: AutomationRuntimeRuleConfig };
    result: void;
  };
  [TauriCommand.automation.collectRuntimeRulePaths]: {
    args: {
      rule: AutomationRuntimeRuleConfig;
      filePaths: string[];
    };
    result: AutomationRuntimePathCollectionResult[];
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
  [TauriCommand.sync.getStatus]: {
    args: undefined;
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.testWebDavProvider]: {
    args: { config: WebDavObjectStoreConfig };
    result: SyncProviderDescriptor;
  };
  [TauriCommand.sync.listLegacyBackups]: {
    args: { config: WebDavObjectStoreConfig };
    result: LegacyRemoteBackupListResult;
  };
  [TauriCommand.sync.prepareLegacyBackupImport]: {
    args: { config: WebDavObjectStoreConfig; key: string };
    result: PreparedBackupImport;
  };
  [TauriCommand.sync.createVault]: {
    args: { request: SyncCreateRequest };
    result: SyncCreateResult;
  };
  [TauriCommand.sync.previewJoin]: {
    args: { request: SyncPreviewJoinRequest };
    result: SyncJoinPreview;
  };
  [TauriCommand.sync.joinVault]: {
    args: { request: SyncJoinRequest };
    result: SyncRunResult;
  };
  [TauriCommand.sync.unlock]: {
    args: { request: SyncUnlockRequest };
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.unlockWithRecovery]: {
    args: { request: SyncUnlockRecoveryRequest };
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.lock]: {
    args: undefined;
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.setPaused]: {
    args: { paused: boolean };
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.disconnect]: {
    args: undefined;
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.runNow]: {
    args: undefined;
    result: SyncRunResult;
  };
  [TauriCommand.sync.changePreset]: {
    args: { preset: SyncPresetV1; confirmShrink: boolean };
    result: SyncStatusSnapshot;
  };
  [TauriCommand.sync.changeMasterPassword]: {
    args: { request: SyncChangePasswordRequest };
    result: void;
  };
  [TauriCommand.sync.generateRecoveryKey]: {
    args: undefined;
    result: string;
  };
  [TauriCommand.sync.listConflicts]: {
    args: undefined;
    result: SyncConflictSummary[];
  };
  [TauriCommand.sync.getConflict]: {
    args: { conflictId: string };
    result: SyncConflictDetail | null;
  };
  [TauriCommand.sync.resolveConflict]: {
    args: { conflictId: string; resolution: SyncConflictResolution };
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
  [TauriCommand.apiServer.dashboardSnapshot]: {
    args: undefined;
    result: ApiServerDashboardSnapshot;
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
