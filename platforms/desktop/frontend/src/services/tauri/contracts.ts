import type {
  AsrTranscriptionRequest_Serialize as CoreAsrTranscriptionRequest,
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
  SpeakerProcessingConfig as CoreSpeakerProcessingConfig,
  SummarizeTranscriptRequest_Serialize as CoreSummarizeTranscriptRequest,
  TranscriptLlmJobRequest_Serialize as CoreTranscriptLlmJobRequest,
  TranslateSegmentsRequest_Serialize as CoreTranslateSegmentsRequest,
  CudaAddonInspection,
  DiagnosticsCoreInput,
  DiagnosticsCoreSnapshot,
  ExportTranscriptFileRequest_Serialize,
  ExportTranscriptFileResult,
  RustTauriCommandContractMap,
  StorageUsageSnapshot_Serialize,
  WebviewBrowsingDataClearResult,
} from '../../bindings';
import type { ApiServerDashboardSnapshot } from '../../types/apiServer';
import type { BackupManifestV1, PreparedBackupImport } from '../../types/backup';
import type { AppConfig, AppLogLevel, ResolvedAppTheme } from '../../types/config';
import type { DashboardSnapshot } from '../../types/dashboard';
import type {
  PolishedSegment,
  TranscriptLlmJobResult,
  TranscriptSummaryResult,
  TranslatedSegment,
} from '../../types/llmTask';
import type { ProjectRecord } from '../../types/project';
import type {
  AsrRuntimeMetricsSnapshot,
  RuntimeEnvironmentStatus,
  RuntimePathStatus,
} from '../../types/runtime';
import type {
  SpeakerProcessingConfig,
  SpeakerProfileSample,
  SpeakerReviewFilter,
  SpeakerReviewSnapshot,
} from '../../types/speaker';
import type {
  ApplySpeakerProfileToGroupRequest,
  SpeakerCorrectionResponse,
  SpeakerGroupRequest,
} from '../../types/speakerCommands';
import type { StorageDirectoriesInfo } from '../../types/storage';
import type {
  DiscoveredVaultSummary,
  LegacyRemoteBackupListResult,
  SyncChangePasswordRequest,
  SyncConflictDetail,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncCreateResult,
  SyncCreateTransportRequest,
  SyncJoinPreview,
  SyncJoinTransportRequest,
  SyncPairingInfo,
  SyncPresetV1,
  SyncPreviewJoinTransportRequest,
  SyncProviderDescriptor,
  SyncProviderTransportInput,
  SyncRunResult,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
  WebDavObjectStoreConfig,
} from '../../types/sync';
import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand, type TauriCommandName } from './commands';

export interface LiveTranscriptionSubscription {
  consumerId: string;
  pipelineId: string;
  shared: boolean;
  transient: boolean;
}

export interface LiveCaptureLease {
  sourceId: string;
  sourceGeneration: number;
  sourceCursor: number;
}

export interface LiveNativeTranscriptionStart {
  lease: LiveCaptureLease;
  subscription: LiveTranscriptionSubscription;
}

export interface LiveTranscriptionMetrics {
  activeSources: number;
  activePipelines: number;
  activeConsumers: number;
  sharedPipelines: number;
  avoidedFeedCount: number;
}

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

type DownloadPresetModelArgs = {
  modelId: string;
  downloadId: string;
  /** Mirror strategy: `auto` | `direct` | `ghproxy` | `ghnet` | `hf-mirror`. */
  mirror?: string;
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
    result: undefined;
  };
  [TauriCommand.app.downloadFile]: {
    args: DownloadFileArgs;
    result: undefined;
  };
  [TauriCommand.app.downloadPresetModel]: {
    args: DownloadPresetModelArgs;
    result: string;
  };
  [TauriCommand.app.deletePresetModel]: {
    args: { modelId: string };
    result: undefined;
  };
  [TauriCommand.app.cancelDownload]: {
    args: { id: string };
    result: undefined;
  };
  [TauriCommand.app.openLogFolder]: {
    args: undefined;
    result: undefined;
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
    result: undefined;
  };
  [TauriCommand.app.getAppSetting]: {
    args: { key: string };
    result: unknown | null;
  };
  [TauriCommand.app.setAppSetting]: {
    args: { key: string; value: unknown };
    result: undefined;
  };
  [TauriCommand.app.migrateAppConfig]: {
    args: {
      savedConfig: AppConfig | null;
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
    result: undefined;
  };
  [TauriCommand.app.updateTrayMenu]: {
    args: UpdateTrayMenuArgs;
    result: undefined;
  };
  [TauriCommand.app.setMinimizeToTray]: {
    args: { enabled: boolean };
    result: undefined;
  };
  [TauriCommand.app.setLogLevel]: {
    args: { level: AppLogLevel };
    result: undefined;
  };
  [TauriCommand.app.setWindowTheme]: {
    args: { theme: ResolvedAppTheme };
    result: undefined;
  };
  [TauriCommand.app.checkMediaFormats]: {
    args: { paths: string[] };
    result: boolean[];
  };
  [TauriCommand.audio.setSystemAudioMute]: {
    args: { mute: boolean };
    result: undefined;
  };
  [TauriCommand.audio.getSystemAudioDevices]: {
    args: undefined;
    result: AudioDevice[];
  };
  [TauriCommand.audio.startSystemAudioCapture]: {
    args: StartAudioCaptureArgs;
    result: undefined;
  };
  [TauriCommand.audio.stopSystemAudioCapture]: {
    args: { instanceId: string };
    result: string;
  };
  [TauriCommand.audio.setSystemAudioCapturePaused]: {
    args: SetCapturePausedArgs;
    result: undefined;
  };
  [TauriCommand.audio.getMicrophoneDevices]: {
    args: undefined;
    result: AudioDevice[];
  };
  [TauriCommand.audio.startMicrophoneCapture]: {
    args: StartAudioCaptureArgs;
    result: undefined;
  };
  [TauriCommand.audio.stopMicrophoneCapture]: {
    args: { instanceId: string };
    result: string;
  };
  [TauriCommand.audio.setMicrophoneCapturePaused]: {
    args: SetCapturePausedArgs;
    result: undefined;
  };
  [TauriCommand.storage.getUsageSnapshot]: {
    args: undefined;
    result: StorageUsageSnapshot_Serialize;
  };
  [TauriCommand.storage.clearWebviewBrowsingData]: {
    args: undefined;
    result: WebviewBrowsingDataClearResult;
  };
  [TauriCommand.storage.getDirectories]: {
    args: undefined;
    result: StorageDirectoriesInfo;
  };
  [TauriCommand.storage.migrateDataDirectory]: {
    args: {
      targetDir: string;
      copyExisting: boolean;
    };
    result: StorageDirectoriesInfo;
  };
  [TauriCommand.storage.resetDataDirectory]: {
    args: undefined;
    result: StorageDirectoriesInfo;
  };
  [TauriCommand.storage.setModelsDirectory]: {
    args: {
      targetDir: string;
      moveExisting: boolean;
    };
    result: StorageDirectoriesInfo;
  };
  [TauriCommand.storage.resetModelsDirectory]: {
    args: undefined;
    result: StorageDirectoriesInfo;
  };
  [TauriCommand.storage.openPath]: {
    args: {
      path: string;
    };
    result: undefined;
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
    result: undefined;
  };
  [TauriCommand.llmUsage.readRaw]: {
    args: undefined;
    result: string;
  };
  [TauriCommand.llmUsage.replaceRaw]: {
    args: { content: string };
    result: undefined;
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
  [TauriCommand.recognizer.prepareLive]: {
    args: { asrRequest: CoreAsrTranscriptionRequest };
    result: undefined;
  };
  [TauriCommand.recognizer.createExternalSource]: {
    args: undefined;
    result: {
      sourceToken: string;
      sourceId: string;
      sourceGeneration: number;
      sourceCursor: number;
    };
  };
  [TauriCommand.recognizer.startExternalLive]: {
    args: {
      consumerId: string;
      sourceToken: string;
      gain: number;
      asrRequest: CoreAsrTranscriptionRequest;
    };
    result: LiveTranscriptionSubscription;
  };
  [TauriCommand.recognizer.feedExternalSource]: {
    args: { sourceToken: string; samples: Uint8Array };
    result: undefined;
  };
  [TauriCommand.recognizer.retireExternalSource]: {
    args: { sourceToken: string };
    result: undefined;
  };
  [TauriCommand.recognizer.startNativeLive]: {
    args: {
      consumerId: string;
      sourceKind: 'system' | 'microphone';
      deviceName: string | null;
      outputPath: string | null;
      gain: number;
      asrRequest: CoreAsrTranscriptionRequest;
    };
    result: LiveNativeTranscriptionStart;
  };
  [TauriCommand.recognizer.pauseNativeLive]: {
    args: { consumerId: string; sourceKind: 'system' | 'microphone' };
    result: undefined;
  };
  [TauriCommand.recognizer.resumeNativeLive]: {
    args: {
      consumerId: string;
      sourceKind: 'system' | 'microphone';
      gain: number;
      asrRequest: CoreAsrTranscriptionRequest;
    };
    result: LiveNativeTranscriptionStart;
  };
  [TauriCommand.recognizer.stopNativeLive]: {
    args: { consumerId: string; sourceKind: 'system' | 'microphone' };
    result: string;
  };
  [TauriCommand.recognizer.stopLive]: {
    args: { consumerId: string };
    result: undefined;
  };
  [TauriCommand.recognizer.getLiveMetrics]: {
    args: undefined;
    result: LiveTranscriptionMetrics;
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
    result: undefined;
  };
  [TauriCommand.backup.disposePreparedImport]: {
    args: { importId: string };
    result: undefined;
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
  [TauriCommand.sync.discoverVaults]: {
    args: { provider: SyncProviderTransportInput };
    result: DiscoveredVaultSummary[];
  };
  [TauriCommand.sync.discoverWebDavVaults]: {
    args: { config: WebDavObjectStoreConfig };
    result: DiscoveredVaultSummary[];
  };
  [TauriCommand.sync.getPairingInfo]: {
    args: undefined;
    result: SyncPairingInfo | null;
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
    result: undefined;
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
    result: undefined;
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
    result: undefined;
  };
  [TauriCommand.system.getAuxWindowState]: {
    args: { label: string };
    result: unknown | null;
  };
  [TauriCommand.system.clearAuxWindowState]: {
    args: { label: string };
    result: undefined;
  };
  [TauriCommand.system.injectText]: {
    args: {
      text: string;
      shortcutModifiers?: string[];
    };
    result: undefined;
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
      gpuAcceleration: 'auto' | 'cpu' | 'vulkan' | 'metal' | 'cuda';
    };
    result: string;
  };
  [TauriCommand.apiServer.stop]: {
    args: undefined;
    result: undefined;
  };
  [TauriCommand.apiServer.dashboardSnapshot]: {
    args: undefined;
    result: ApiServerDashboardSnapshot;
  };
  [TauriCommand.cudaAddon.getStatus]: {
    args: undefined;
    result: CudaAddonInspection;
  };
  [TauriCommand.cudaAddon.activate]: {
    args: undefined;
    result: CudaAddonInspection;
  };
  [TauriCommand.cudaAddon.download]: {
    args: {
      downloadId: string;
      mirror?: string;
      version?: string;
      customUrl?: string;
      expectedSha256?: string;
    };
    result: CudaAddonInspection;
  };
};

export type TauriCommandContractMap = RustTauriCommandContractMap & ManualTauriCommandContractMap;

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
  [TCommand in KnownTauriCommandName]: TauriCommandArgs<TCommand> extends undefined
    ? never
    : TCommand;
}[KnownTauriCommandName];
export type TauriCommandsWithoutArgs = Exclude<KnownTauriCommandName, TauriCommandsWithArgs>;
