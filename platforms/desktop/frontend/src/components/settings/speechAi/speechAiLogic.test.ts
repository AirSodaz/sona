import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { ModelInfo } from '../../../types/modelCatalog';
import {
  buildSpeechRow,
  curatedGroupOrder,
  matchesLanguageFilter,
  modesAreLinked,
  pathIsInstalled,
  recommendedVariantId,
  SENSEVOICE_STANDARD_ID,
  speakerHelpersReady,
  WHISPER_TURBO_ID,
} from './speechAiLogic';

function config(patch: Partial<AppConfig> = {}): AppConfig {
  return {
    asr: {
      selections: {
        live: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
        caption: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
        voiceTyping: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
        batch: { engine: 'local', mode: 'batch', modelId: 'a', modelPath: '/a' },
      },
    },
    liveSpeakerEmbeddingModelPath: '',
    batchSpeakerEmbeddingModelPath: '',
    liveSpeakerSegmentationModelPath: '',
    batchSpeakerSegmentationModelPath: '',
    ...patch,
  } as AppConfig;
}

describe('speech AI live versus batch', () => {
  it('links modes when recording and files already match', () => {
    expect(modesAreLinked(config())).toBe(true);
  });

  it('keeps modes separate when the stored selections differ and the flag is unset', () => {
    const diverged = config({
      asr: {
        selections: {
          live: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
          caption: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
          voiceTyping: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
          batch: { engine: 'local', mode: 'batch', modelId: 'b', modelPath: '/b' },
        },
      },
    });
    expect(modesAreLinked(diverged)).toBe(false);
  });

  it('honors an explicit unlink even when the values match', () => {
    expect(modesAreLinked(config({ speechModesLinked: false }))).toBe(false);
  });
});

describe('speech model picker naming', () => {
  it('recommends SenseVoice unless the UI language is English', () => {
    expect(recommendedVariantId('zh')).toBe(SENSEVOICE_STANDARD_ID);
    expect(recommendedVariantId('en')).toBe(WHISPER_TURBO_ID);
    expect(curatedGroupOrder('en')[0]).toBe('whisper');
    expect(curatedGroupOrder('zh')[0]).toBe('sensevoice');
  });
});

describe('speaker helper readiness', () => {
  it('stays ready when speakers are off', () => {
    expect(speakerHelpersReady(config(), [], new Set())).toBe(true);
  });

  it('is not ready when speakers are on but the helper file is not installed', () => {
    const withSpeakers = config({
      liveSpeakerEmbeddingModelPath: '/models/campplus',
      batchSpeakerEmbeddingModelPath: '/models/campplus',
      batchSpeakerSegmentationModelPath: '/models/pyannote',
    });
    const models = [
      { ...model('emb', 'emb', 'sherpa-onnx'), installPath: '/models/campplus' },
      { ...model('seg', 'seg', 'sherpa-onnx'), installPath: '/models/pyannote' },
    ];
    expect(pathIsInstalled('/models/campplus', models, new Set(['emb']))).toBe(true);
    expect(speakerHelpersReady(withSpeakers, models, new Set(['emb']))).toBe(false);
    expect(speakerHelpersReady(withSpeakers, models, new Set(['emb', 'seg']))).toBe(true);
  });
});

describe('matchesLanguageFilter', () => {
  it('includes multilingual models supporting English under the English filter', () => {
    const whisperTurbo: ModelInfo = {
      ...model('whisper-turbo', 'v3', 'sherpa-onnx'),
      languages: ['en', 'zh', 'ja', 'ko', 'es', 'fr', 'de'],
    };
    expect(matchesLanguageFilter(whisperTurbo, 'en')).toBe(true);
    expect(matchesLanguageFilter(whisperTurbo, 'zh')).toBe(true);
    expect(matchesLanguageFilter(whisperTurbo, 'any')).toBe(true);
  });

  it('filters out non-English models under English filter', () => {
    const zhOnly: ModelInfo = {
      ...model('zh-only', 'v1', 'sherpa-onnx'),
      languages: ['zh', 'yue'],
    };
    expect(matchesLanguageFilter(zhOnly, 'en')).toBe(false);
    expect(matchesLanguageFilter(zhOnly, 'zh')).toBe(true);
  });
});

describe('buildSpeechRow offline handling', () => {
  const dummyT = (key: string, opts?: Record<string, unknown>) => {
    if (opts && 'model' in opts) return `${key}:${opts.model}`;
    return key;
  };

  it('returns warn status when offline and online ASR is active', () => {
    const onlineCfg = config({
      asr: {
        selections: {
          live: {
            engine: 'online',
            mode: 'streaming',
            providerId: 'volcengine',
            modelId: 'bigmodel',
            modelPath: '',
          },
          caption: {
            engine: 'online',
            mode: 'streaming',
            providerId: 'volcengine',
            modelId: 'bigmodel',
            modelPath: '',
          },
          voiceTyping: {
            engine: 'online',
            mode: 'streaming',
            providerId: 'volcengine',
            modelId: 'bigmodel',
            modelPath: '',
          },
          batch: {
            engine: 'online',
            mode: 'batch',
            providerId: 'volcengine',
            modelId: 'bigmodel',
            modelPath: '',
          },
        },
      },
    });
    const row = buildSpeechRow({
      config: onlineCfg,
      catalogModels: [],
      installedIds: new Set(),
      recommendedId: WHISPER_TURBO_ID,
      offline: true,
      t: dummyT,
    });
    expect(row.status).toBe('warn');
    expect(row.statusLabel).toBe('settings.speech_ai.unavailable_offline');
  });

  it('stays ok when offline and local model is active', () => {
    const row = buildSpeechRow({
      config: config(),
      catalogModels: [model('a', 'v1', 'sherpa-onnx')],
      installedIds: new Set(['a']),
      recommendedId: WHISPER_TURBO_ID,
      offline: true,
      t: dummyT,
    });
    expect(row.status).toBe('ok');
    expect(row.statusLabel).toBe('settings.speech_ai.ready');
  });
});

function model(id: string, versionLabel: string, engine: ModelInfo['engine']): ModelInfo {
  return {
    id,
    name: id,
    description: '',
    type: 'whisper',
    languages: ['en'],
    languageMode: 'auto',
    size: '1 MB',
    engine,
    versionLabel,
  };
}
