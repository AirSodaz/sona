import { isFeatureLlmConfigComplete } from '../../../services/llm/configUtils';
import type { AppConfig, LlmAssistantConfig } from '../../../types/config';
import type { LlmProvider } from '../../../types/transcript';

export type CapabilityStatusKind =
  | 'needs_setup'
  | 'downloading'
  | 'ready'
  | 'connected'
  | 'not_running'
  | 'needs_key'
  | 'error';

export type ProbeResultStatus = 'connected' | 'not_running' | 'needs_key' | 'error';

export interface ProviderProbeCacheEntry {
  status: ProbeResultStatus;
  message?: string;
  checkedAt: number;
  modelIds?: string[];
}

const probeCache = new Map<string, ProviderProbeCacheEntry>();

export function getProviderProbeCacheKey(
  provider: LlmProvider,
  apiHost: string,
  apiKeyFingerprint: string
): string {
  return `${provider}::${apiHost.trim()}::${apiKeyFingerprint}`;
}

export function readProviderProbeCache(key: string): ProviderProbeCacheEntry | undefined {
  return probeCache.get(key);
}

export function writeProviderProbeCache(key: string, entry: ProviderProbeCacheEntry): void {
  probeCache.set(key, entry);
}

export function clearProviderProbeCache(key?: string): void {
  if (key) {
    probeCache.delete(key);
    return;
  }
  probeCache.clear();
}

export function fingerprintApiKey(apiKey: string): string {
  const trimmed = apiKey.trim();
  if (!trimmed) {
    return 'empty';
  }
  return `${trimmed.length}:${trimmed.slice(0, 4)}:${trimmed.slice(-4)}`;
}

export function isSpeechRecognitionReady(config: {
  streamingModelPath?: string | null;
  asr?: AppConfig['asr'] | null;
}): boolean {
  const liveSelection = config.asr?.selections?.live;
  if (liveSelection?.engine === 'online') {
    return Boolean(liveSelection.providerId?.trim());
  }
  if (liveSelection?.engine === 'local' && liveSelection.modelPath?.trim()) {
    return true;
  }
  return Boolean(config.streamingModelPath?.trim());
}

/** Prefer a human catalog name over a raw path/filename for Speech Ready labels. */
export function resolveSpeechModelDisplayName(args: {
  streamingModelId?: string | null;
  streamingModelPath?: string | null;
  liveModelPath?: string | null;
  catalogModels?: Array<{ id: string; name: string; installPath?: string | null }>;
  modelPathById?: Record<string, string>;
}): string | null {
  const {
    streamingModelId,
    streamingModelPath,
    liveModelPath,
    catalogModels = [],
    modelPathById = {},
  } = args;

  if (streamingModelId) {
    const fromCatalog = catalogModels.find((model) => model.id === streamingModelId);
    if (fromCatalog?.name) {
      return fromCatalog.name;
    }
  }

  const livePath = streamingModelPath || liveModelPath || null;
  if (!livePath) {
    return null;
  }

  const byPath = catalogModels.find(
    (model) => model.installPath === livePath || modelPathById[model.id] === livePath
  );
  if (byPath?.name) {
    return byPath.name;
  }

  const parts = livePath.split(/[/\\]/);
  const fileName = parts[parts.length - 1] || livePath;
  return fileName.replace(/\.(onnx|bin|gguf|tar|gz|zip)$/i, '') || fileName;
}

export function isAiWritingConfigured(config: LlmAssistantConfig): boolean {
  return isFeatureLlmConfigComplete(config, 'polish');
}

export function isTranslationConfigured(config: LlmAssistantConfig): boolean {
  return isFeatureLlmConfigComplete(config, 'translation');
}

export { parseModelSizeToBytes, resolveModelDownloadBytes } from '../../../utils/modelDownloadSize';
