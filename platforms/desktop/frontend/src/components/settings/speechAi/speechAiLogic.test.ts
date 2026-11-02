import { describe, expect, it } from 'vitest';
import { addLlmModel, createLlmSettings } from '../../../services/llm/state';
import type { AppConfig } from '../../../types/config';
import type { ModelInfo } from '../../../types/modelCatalog';
import {
  assignConnectedModel,
  curatedGroupOrder,
  GOOGLE_TRANSLATE_VALUE,
  matchesLanguageFilter,
  modesAreLinked,
  pathIsInstalled,
  recommendedVariantId,
  SENSEVOICE_STANDARD_ID,
  setTranslationChoice,
  speakerHelpersReady,
  speechModelsAssigned,
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
      liveSpeakerDiarizationEnabled: true,
      batchSpeakerDiarizationEnabled: true,
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

describe('speech model assignment', () => {
  it('is ready when the linked recording model is installed', () => {
    expect(speechModelsAssigned(config(), new Set(['a']))).toBe(true);
  });

  it('is not ready when recording and files differ and one side is empty', () => {
    const split = config({
      speechModesLinked: false,
      asr: {
        selections: {
          live: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
          caption: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
          voiceTyping: { engine: 'local', mode: 'streaming', modelId: 'a', modelPath: '/a' },
          batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
        },
      },
    });
    expect(speechModelsAssigned(split, new Set(['a']))).toBe(false);
  });
});

describe('first AI connection', () => {
  it('sets polish and summary, and leaves translation alone', () => {
    const before = createLlmSettings();
    const withGoogle = setTranslationChoice(before, GOOGLE_TRANSLATE_VALUE);
    const googleId = withGoogle.selections.translationModelId;
    const added = addLlmModel(withGoogle, { provider: 'open_ai', model: 'gpt-4o-mini' });
    const modelId = added.modelOrder.find((id) => added.models[id]?.provider === 'open_ai');
    const next = assignConnectedModel(withGoogle, added, modelId, false);
    expect(next.selections.polishModelId).toBe(modelId);
    expect(next.selections.summaryModelId).toBe(modelId);
    expect(next.selections.translationModelId).toBe(googleId);
  });

  it('does not replace polish when a writing model already exists', () => {
    let before = createLlmSettings();
    before = addLlmModel(before, { provider: 'open_ai', model: 'gpt-4o-mini' });
    const firstId = before.modelOrder[0];
    before = assignConnectedModel(createLlmSettings(), before, firstId, false);
    const added = addLlmModel(before, { provider: 'anthropic', model: 'claude' });
    const secondId = added.modelOrder.find((id) => added.models[id]?.provider === 'anthropic');
    const next = assignConnectedModel(before, added, secondId, false);
    expect(next.selections.polishModelId).toBe(firstId);
    expect(next.selections.summaryModelId).toBe(firstId);
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
