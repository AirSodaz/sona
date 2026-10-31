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

export function isAiWritingConfigured(config: LlmAssistantConfig): boolean {
  return isFeatureLlmConfigComplete(config, 'polish');
}

export function isTranslationConfigured(config: LlmAssistantConfig): boolean {
  return isFeatureLlmConfigComplete(config, 'translation');
}

export {
  LARGE_DOWNLOAD_BYTES,
  parseModelSizeToBytes,
  resolveModelDownloadBytes,
} from '../../../utils/modelDownloadSize';
