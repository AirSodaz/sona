import type { BackupManifestV1, PreparedBackupImport } from "../../types/backup";
import type { DashboardSnapshot } from "../../types/dashboard";
import type {
  AsrTranscriptionRequest_Serialize as CoreAsrTranscriptionRequest,
  DiagnosticsCoreInput,
  DiagnosticsCoreSnapshot,
  ExportTranscriptFileRequest_Serialize,
  ExportTranscriptFileResult,
  LlmCompletionRequest_Serialize as CoreLlmCompletionRequest,
  LlmCompletionResponse_Serialize as CoreLlmCompletionResponse,
  LlmConfig_Serialize as CoreLlmConfig,
  LlmGenerateRequest_Serialize as CoreLlmGenerateRequest,
  LlmModelSummary as CoreLlmModelSummary,
  LlmModelsRequest_Serialize as CoreLlmModelsRequest,
  ModelCatalogSelectedIds as CoreModelCatalogSelectedIds,
  ModelCatalogSnapshot as CoreModelCatalogSnapshot,
  ModelSelectionPaths as CoreModelSelectionPaths,
  PolishSegmentsRequest_Serialize as CorePolishSegmentsRequest,
  RustTauriCommandContractMap,
  SpeakerProcessingConfig as CoreSpeakerProcessingConfig,
  StorageUsageSnapshot_Serialize,
  SummarizeTranscriptRequest_Serialize as CoreSummarizeTranscriptRequest,
  TranscriptLlmJobRequest_Serialize as CoreTranscriptLlmJobRequest,
  TranslateSegmentsRequest_Serialize as CoreTranslateSegmentsRequest,
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
} from "../../types/config";
import type {
  ProjectRecord,
} from "../../types/project";
import type {
  SpeakerProfileSample,
  SpeakerProcessingConfig,
} from "../../types/speaker";
import type { TranscriptSegment } from "../../types/transcript";
import type { ApiServerDashboardSnapshot } from "../../types/apiServer";
import type {
  SyncChangePasswordRequest,
  SyncConflictDetail,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncCreateResult,
  SyncCreateTransportRequest,
  SyncJoinPreview,
  SyncJoinTransportRequest,
  SyncPresetV1,
  SyncPreviewJoinTransportRequest,
  SyncProviderDescriptor,
  SyncProviderTransportInput,
  SyncRunResult,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
  WebDavObjectStoreConfig,
  LegacyRemoteBackupListResult,
} from "../../types/sync";
import type {
  PolishedSegment,
  TranscriptLlmJobResult,
  TranscriptSummaryResult,
  TranslatedSegment,
} from "../llmTaskTypes";
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

type ExportBackupArchiveRequest = {
  archivePath: string;
  appVersion: string;
};

export type ModelSelectionPaths = CoreModelSelectionPaths;

export type ModelCatalogSelectedIds = CoreModelCatalogSelectedIds;

type ManualTauriCommandContractMap = {
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
    args: { request: CoreLlmGenerateRequest };
    result: string;
  };
  [TauriCommand.llm.complete]: {
    args: { request: CoreLlmCompletionRequest };
    result: CoreLlmCompletionResponse;
  };
  [TauriCommand.llm.describeModel]: {
    args: { config: CoreLlmConfig };
    result: CoreLlmModelSummary | null;
  };
  [TauriCommand.llm.listModels]: {
    args: { request: CoreLlmModelsRequest };
    result: CoreLlmModelSummary[];
  };
  [TauriCommand.llm.polishTranscriptSegments]: {
    args: { request: CorePolishSegmentsRequest };
    result: PolishedSegment[];
  };
  [TauriCommand.llm.runTranscriptJob]: {
    args: { request: CoreTranscriptLlmJobRequest };
    result: TranscriptLlmJobResult;
  };
  [TauriCommand.llm.summarizeTranscript]: {
    args: { request: CoreSummarizeTranscriptRequest };
    result: TranscriptSummaryResult;
  };
  [TauriCommand.llm.translateTranscriptSegments]: {
    args: { request: CoreTranslateSegmentsRequest };
    result: TranslatedSegment[];
  };
  [TauriCommand.recognizer.init]: {
    args: {
      instanceId: string;
      asrRequest: CoreAsrTranscriptionRequest;
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
      speakerProcessing: CoreSpeakerProcessingConfig | null;
      asrRequest: CoreAsrTranscriptionRequest;
      instanceId?: string;
    };
    result: TranscriptSegment[];
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
  [TauriCommand.sync.testProvider]: {
    args: { provider: SyncProviderTransportInput };
    result: SyncProviderDescriptor;
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
    args: { request: SyncCreateTransportRequest };
    result: SyncCreateResult;
  };
  [TauriCommand.sync.previewJoin]: {
    args: { request: SyncPreviewJoinTransportRequest };
    result: SyncJoinPreview;
  };
  [TauriCommand.sync.joinVault]: {
    args: { request: SyncJoinTransportRequest };
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

export type TauriCommandContractMap = RustTauriCommandContractMap &
  ManualTauriCommandContractMap;

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
