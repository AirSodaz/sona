import type { AppConfig, AsrSelectionSlot } from '../../types/config';
import {
  type AsrTranscriptionRequest,
  isAsrRequestConfigured,
  isLlamaCppBatchRequest,
  resolveAsrTranscriptionRequest,
} from '../asrConfigService';
import { speakerService } from '../speakerService';
import type { ProcessBatchFileRequest } from '../tauri/recognizer';

interface StreamingRequestOptions {
  appConfig: AppConfig;
  instanceId: string;
  modelPathOverride?: string;
  language: string;
  enableItn: boolean;
  projectId?: string | null;
}

interface BatchRequestOptions {
  appConfig: AppConfig;
  filePath: string;
  saveToPath?: string | null;
  modelPathOverride?: string;
  language: string;
  enableItn: boolean;
  instanceId?: string;
  projectId?: string | null;
}

interface ResolvedBatchTranscriptionRequest {
  request: ProcessBatchFileRequest;
  asrRequest: AsrTranscriptionRequest;
}

export function resolveStreamingSlot(
  instanceId: string
): Extract<AsrSelectionSlot, 'live' | 'caption' | 'voiceTyping'> {
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
  enableItn: boolean
): AsrTranscriptionRequest {
  if (request.engine === 'local') {
    return {
      ...request,
      modelPath: modelPathOverride ? modelPathOverride : request.modelPath,
      enableItn,
    };
  }

  return {
    ...request,
    enableItn,
  };
}

export function buildStreamingAsrRequest({
  appConfig,
  instanceId,
  modelPathOverride,
  language,
  enableItn,
  projectId,
}: StreamingRequestOptions): AsrTranscriptionRequest {
  const request = resolveAsrTranscriptionRequest(appConfig, resolveStreamingSlot(instanceId), {
    language,
  });

  return {
    ...applyRuntimeOptions(request, modelPathOverride, enableItn),
    normalizationOptions: {
      enableTimeline: instanceId === 'record' ? (appConfig.enableTimeline ?? false) : false,
    },
    speakerProcessing:
      request.engine === 'online'
        ? null
        : speakerService.buildProcessingConfig(appConfig, 'live', projectId),
  };
}

export function buildBatchTranscriptionRequest({
  appConfig,
  filePath,
  saveToPath,
  modelPathOverride,
  language,
  enableItn,
  instanceId,
  projectId,
}: BatchRequestOptions): ResolvedBatchTranscriptionRequest {
  const resolvedBatchRequest = resolveAsrTranscriptionRequest(appConfig, 'batch', { language });
  const runtimeRequest = applyRuntimeOptions(resolvedBatchRequest, modelPathOverride, enableItn);
  const asrRequest = isLlamaCppBatchRequest(runtimeRequest)
    ? {
        ...runtimeRequest,
        language: 'auto',
        enableItn: false,
        hotwords: null,
        punctuationModel: null,
      }
    : runtimeRequest;
  const isOnline = asrRequest.engine === 'online';
  const isLlamaCpp = isLlamaCppBatchRequest(asrRequest);

  return {
    asrRequest,
    request: {
      filePath,
      saveToPath: isLlamaCpp || isOnline ? null : saveToPath || null,
      speakerProcessing:
        isLlamaCpp || isOnline
          ? null
          : speakerService.buildProcessingConfig(appConfig, 'batch', projectId),
      asrRequest,
      ...(instanceId ? { instanceId } : {}),
    },
  };
}

export function isTranscriptionRequestConfigured(request: AsrTranscriptionRequest): boolean {
  return isAsrRequestConfigured(request);
}
