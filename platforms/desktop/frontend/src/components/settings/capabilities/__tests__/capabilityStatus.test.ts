import { describe, expect, it } from 'vitest';
import {
  clearProviderProbeCache,
  fingerprintApiKey,
  getProviderProbeCacheKey,
  isAiWritingConfigured,
  isSpeechRecognitionReady,
  isTranslationConfigured,
  readProviderProbeCache,
  writeProviderProbeCache,
} from '../capabilityStatus';
import {
  LARGE_DOWNLOAD_BYTES,
  parseModelSizeToBytes,
  resolveModelDownloadBytes,
} from '../../../../utils/modelDownloadSize';

describe('capabilityStatus helpers', () => {
  it('detects speech readiness from streaming path or asr selection', () => {
    expect(isSpeechRecognitionReady({})).toBe(false);
    expect(isSpeechRecognitionReady({ streamingModelPath: '/models/sensevoice' })).toBe(true);
    expect(
      isSpeechRecognitionReady({
        asr: {
          selections: {
            live: { engine: 'local', modelPath: '/models/a', modelId: 'a' },
          },
        } as never,
      })
    ).toBe(true);
  });

  it('treats translation as configured when feature llm config is complete', () => {
    expect(
      isTranslationConfigured({
        llmSettings: {
          activeProvider: 'google_translate_free',
          providers: {
            google_translate_free: { apiHost: '', apiKey: '' },
          },
          models: {
            'google_translate_free:default': {
              id: 'google_translate_free:default',
              provider: 'google_translate_free',
              model: 'default',
            },
          },
          modelOrder: ['google_translate_free:default'],
          selections: {
            translationModelId: 'google_translate_free:default',
          },
          customProviders: {},
        },
      } as never)
    ).toBe(true);
  });

  it('requires polish model for AI writing readiness', () => {
    expect(isAiWritingConfigured({} as never)).toBe(false);
  });

  it('stores probe cache keyed by provider host and key fingerprint', () => {
    clearProviderProbeCache();
    const key = getProviderProbeCacheKey('open_ai', 'https://api.openai.com/v1', fingerprintApiKey('sk-test'));
    writeProviderProbeCache(key, { status: 'connected', checkedAt: 1, modelIds: ['gpt-4o-mini'] });
    expect(readProviderProbeCache(key)?.status).toBe('connected');
    clearProviderProbeCache(key);
    expect(readProviderProbeCache(key)).toBeUndefined();
  });
});

describe('modelDownloadSize', () => {
  it('parses human sizes and respects the large-download threshold', () => {
    expect(parseModelSizeToBytes('350 MB')).toBe(350 * 1024 * 1024);
    expect(parseModelSizeToBytes('1.2 GB')).toBe(Math.round(1.2 * 1024 ** 3));
    expect(resolveModelDownloadBytes({ size: '100 MB', artifacts: [] })).toBeLessThan(
      LARGE_DOWNLOAD_BYTES
    );
    expect(resolveModelDownloadBytes({ size: '500 MB', artifacts: [] })).toBeGreaterThanOrEqual(
      LARGE_DOWNLOAD_BYTES
    );
  });
});
