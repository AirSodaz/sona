import type { AppConfig, AsrSelectionSlot } from '../../types/config';
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
  type AsrTranscriptionRequest,
} from '../asrConfigService';
import { speakerService } from '../speakerService';
import type {
  InitRecognizerRequest,
  ProcessBatchFileRequest,
} from '../tauri/recognizer';

interface StreamingRequestOptions {
  appConfig: AppConfig;
  instanceId: string;
  modelPathOverride?: string;
  language: string;
  enableItn: boolean;
}

interface BatchRequestOptions {
  appConfig: AppConfig;
  filePath: string;
  saveToPath?: string | null;
  modelPathOverride?: string;
  language: string;
  enableItn: boolean;
}

interface ResolvedRecognizerInitRequest {
  request: InitRecognizerRequest;
  asrRequest: AsrTranscriptionRequest;
}

interface ResolvedBatchTranscriptionRequest {
  request: ProcessBatchFileRequest;
  asrRequest: AsrTranscriptionRequest;
}

export function resolveStreamingSlot(instanceId: string): Extract<AsrSelectionSlot, 'live' | 'caption' | 'voiceTyping'> {
  if (instanceId === 'voice-typing') {
    return 'voiceTyping';
  }
  if (instanceId === 'caption') {
    return 'caption';
  }
  return 'live';
}

function applyRuntimeOptions(
  request: AsrTranscriptionRequest,
  modelPathOverride: string | undefined,
  enableItn: boolean,
): AsrTranscriptionRequest {
  const modelPath = request.engine === 'local-sherpa' && modelPathOverride
    ? modelPathOverride
    : request.modelPath;

  return {
    ...request,
    modelPath,
    enableItn,
  };
}

export function buildStreamingAsrRequest({
  appConfig,
  instanceId,
  modelPathOverride,
  language,
  enableItn,
}: StreamingRequestOptions): AsrTranscriptionRequest {
  const request = resolveAsrTranscriptionRequest(
    appConfig,
    resolveStreamingSlot(instanceId),
    { language },
  );

  return {
    ...applyRuntimeOptions(request, modelPathOverride, enableItn),
    normalizationOptions: {
      enableTimeline: instanceId === 'record'
        ? (appConfig.enableTimeline ?? false)
        : false,
    },
  };
}

export function buildRecognizerInitRequest(options: StreamingRequestOptions): ResolvedRecognizerInitRequest {
  const asrRequest = buildStreamingAsrRequest(options);
  return {
    asrRequest,
    request: {
      instanceId: options.instanceId,
      asrRequest,
    },
  };
}

export function buildBatchTranscriptionRequest({
  appConfig,
  filePath,
  saveToPath,
  modelPathOverride,
  language,
  enableItn,
}: BatchRequestOptions): ResolvedBatchTranscriptionRequest {
  const resolvedBatchRequest = resolveAsrTranscriptionRequest(appConfig, 'batch', { language });
  const asrRequest = applyRuntimeOptions(resolvedBatchRequest, modelPathOverride, enableItn);

  return {
    asrRequest,
    request: {
      filePath,
      saveToPath: saveToPath || null,
      speakerProcessing: speakerService.buildProcessingConfig(appConfig),
      asrRequest,
    },
  };
}

export function isTranscriptionRequestConfigured(request: AsrTranscriptionRequest): boolean {
  return isAsrRequestConfigured(request);
}
