import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { ModelInfo } from '../../../types/modelCatalog';
import {
  curatedGroupOrder,
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
