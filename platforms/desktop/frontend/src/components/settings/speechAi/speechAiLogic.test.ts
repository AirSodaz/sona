import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../../../types/config';
import {
  curatedGroupOrder,
  friendlyVersion,
  modesAreLinked,
  recommendedVariantId,
  SENSEVOICE_STANDARD_ID,
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
});
