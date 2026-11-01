import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { ModelInfo } from '../../../types/modelCatalog';
import {
  curatedGroupOrder,
  friendlyVersion,
  modesAreLinked,
  pathIsInstalled,
  recommendedVariantId,
  SENSEVOICE_STANDARD_ID,
  speakerHelpersReady,
  versionRowLabels,
  WHISPER_TURBO_ID,
} from './speechAiLogic';

const t = (key: string, options?: Record<string, unknown>) =>
  typeof options?.defaultValue === 'string' ? options.defaultValue : key;

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

  it('maps quantization labels onto friendly version names', () => {
    expect(friendlyVersion({ versionLabel: 'Int8', engine: 'sherpa-onnx' }).titleDefault).toBe(
      'Standard'
    );
    expect(friendlyVersion({ versionLabel: 'Q5_0', engine: 'llama-cpp' }).titleDefault).toBe(
      'Smaller'
    );
    expect(friendlyVersion({ versionLabel: 'Fp32', engine: 'sherpa-onnx' }).tier).toBe('full');
    expect(friendlyVersion({ versionLabel: 'Int8', engine: 'sherpa-onnx' }).hint).toBe('Int8 ONNX');
  });

  it('uses the translator stub so copy tests stay local', () => {
    expect(t('settings.speech_ai.ready', { defaultValue: 'Ready' })).toBe('Ready');
  });

  it('keeps size and latency in the primary label when several rows share a tier', () => {
    const models = [
      model('a', '0.6B Q8_0', 'llama-cpp'),
      model('b', '1.7B Q8_0', 'llama-cpp'),
      model('c', '0.6B Int8', 'sherpa-onnx'),
      model('d', '300M v2 Fp32', 'sherpa-onnx'),
      model('e', '160ms Int8', 'sherpa-onnx'),
    ];
    const labels = Object.fromEntries(
      versionRowLabels(models, t).map((row) => [row.id, row.title])
    );
    expect(labels.a).toBe('Standard · 0.6B · Q8_0');
    expect(labels.b).toBe('Standard · 1.7B');
    expect(labels.c).toBe('Standard · 0.6B · Int8');
    expect(labels.d).toBe('Full precision · 300M v2');
    expect(labels.e).toBe('Standard · 160ms');
    expect(new Set(Object.values(labels)).size).toBe(models.length);
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
