import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import {
  AssemblyAiLogo,
  DeepgramLogo,
  DolphinLogo,
  ElevenLabsLogo,
  FireRedLogo,
  FunAsrNanoLogo,
  GenericModelLogo,
  GroqLogo,
  MetaLogo,
  MistralLogo,
  ModelBrandLogo,
  MoonshineLogo,
  NvidiaLogo,
  OpenAILogo,
  ParaformerLogo,
  QwenLogo,
  resolveModelBrand,
  SenseVoiceLogo,
  VolcengineLogo,
  WhisperLogo,
  ZipformerLogo,
} from '../ModelLogos';

describe('ModelLogos - resolveModelBrand', () => {
  it('correctly resolves Qwen ASR models', () => {
    expect(resolveModelBrand({ id: 'qwen3-asr-0.6b-q8-gguf', groupId: 'qwen3-asr' })).toBe('qwen');
    expect(resolveModelBrand({ id: 'sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25' })).toBe('qwen');
  });

  it('correctly resolves SenseVoice models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
        groupId: 'sensevoice',
      })
    ).toBe('sensevoice');
  });

  it('correctly resolves FireRedASR models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26',
        groupId: 'firered-asr2-aed',
      })
    ).toBe('firered');
  });

  it('correctly resolves Whisper models', () => {
    expect(resolveModelBrand({ id: 'sherpa-onnx-whisper-turbo', groupId: 'whisper' })).toBe(
      'whisper'
    );
    expect(resolveModelBrand({ id: 'sherpa-onnx-whisper-large-v3' })).toBe('whisper');
  });

  it('correctly resolves Omnilingual (Meta) models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-omnilingual-asr-1600-languages-1B-ctc-v2-int8-2026-02-05',
        groupId: 'omnilingual-asr',
      })
    ).toBe('meta');
  });

  it('correctly resolves Paraformer models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en-int8',
        groupId: 'paraformer',
      })
    ).toBe('paraformer');
  });

  it('correctly resolves FunASR Nano models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-funasr-nano-int8-2025-12-30',
        groupId: 'funasr-nano',
      })
    ).toBe('funasr-nano');
  });

  it('correctly resolves Zipformer models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30',
        type: 'zipformer',
      })
    ).toBe('zipformer');
  });

  it('correctly resolves Parakeet TDT (NVIDIA) models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8',
        groupId: 'parakeet-tdt',
      })
    ).toBe('nvidia');
  });

  it('correctly resolves Dolphin models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-dolphin-small-ctc-multi-lang-int8-2025-04-02',
        groupId: 'dolphin',
      })
    ).toBe('dolphin');
  });

  it('correctly resolves Moonshine models', () => {
    expect(
      resolveModelBrand({
        id: 'sherpa-onnx-moonshine-base-zh-quantized-2026-02-27',
        groupId: 'moonshine-v2',
      })
    ).toBe('moonshine');
  });

  it('correctly resolves online cloud ASR provider models', () => {
    expect(resolveModelBrand({ id: 'volcengine-doubao', name: '豆包语音 (火山)' })).toBe(
      'volcengine'
    );
    expect(resolveModelBrand({ id: 'groq-whisper', name: 'Whisper (Groq)' })).toBe('groq');
    expect(resolveModelBrand({ id: 'mistral-voxtral', name: 'Voxtral (Mistral)' })).toBe('mistral');
    expect(resolveModelBrand({ id: 'openai-whisper', name: 'Whisper (OpenAI)' })).toBe('openai');
    expect(resolveModelBrand({ id: 'deepgram', name: 'Deepgram' })).toBe('deepgram');
    expect(resolveModelBrand({ id: 'assemblyai', name: 'AssemblyAI' })).toBe('assemblyai');
    expect(resolveModelBrand({ id: 'elevenlabs', name: 'ElevenLabs' })).toBe('elevenlabs');
  });

  it('falls back to generic for unrecognized ASR models', () => {
    expect(resolveModelBrand({ id: 'my-custom-fine-tuned-asr-v1', name: 'Custom ASR' })).toBe(
      'generic'
    );
  });

  it('returns null for non-ASR models (VAD, punctuation, speaker diarization/embedding)', () => {
    expect(resolveModelBrand({ id: 'silero-vad', type: 'vad' })).toBeNull();
    expect(
      resolveModelBrand({ id: 'sherpa-onnx-punct-ct-transformer', type: 'punctuation' })
    ).toBeNull();
    expect(
      resolveModelBrand({ id: 'pyannote-segmentation-3-0', type: 'speaker-segmentation' })
    ).toBeNull();
    expect(
      resolveModelBrand({ id: '3dspeaker_speech_campplus', type: 'speaker-embedding' })
    ).toBeNull();
  });
});

describe('ModelLogos - Component Rendering', () => {
  it('renders Qwen logo image', () => {
    const { container } = render(<QwenLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('width')).toBe('36');
    expect(img?.getAttribute('alt')).toBe('Qwen');
  });

  it('renders SenseVoice logo image', () => {
    const { container } = render(<SenseVoiceLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('SenseVoice');
  });

  it('renders FireRed logo image', () => {
    const { container } = render(<FireRedLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('FireRedASR');
  });

  it('renders Whisper logo image', () => {
    const { container } = render(<WhisperLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Whisper');
  });

  it('renders Meta logo image', () => {
    const { container } = render(<MetaLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Meta Omnilingual');
  });

  it('renders Paraformer logo image', () => {
    const { container } = render(<ParaformerLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Paraformer');
  });

  it('renders FunASR Nano logo image', () => {
    const { container } = render(<FunAsrNanoLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('FunASR Nano');
  });

  it('renders Zipformer logo image', () => {
    const { container } = render(<ZipformerLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Zipformer');
  });

  it('renders Nvidia logo image', () => {
    const { container } = render(<NvidiaLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('NVIDIA Parakeet');
  });

  it('renders Dolphin logo image', () => {
    const { container } = render(<DolphinLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Dolphin');
  });

  it('renders Moonshine logo image', () => {
    const { container } = render(<MoonshineLogo size={36} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Moonshine');
  });

  it('renders Generic fallback logo SVG', () => {
    const { container } = render(<GenericModelLogo size={36} />);
    expect(container.querySelector('svg')).toBeTruthy();
  });

  it('renders via unified ModelBrandLogo with model prop', () => {
    const { container } = render(
      <ModelBrandLogo model={{ id: 'qwen3-asr-0.6b-q8-gguf' }} size={40} className="test-badge" />
    );
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('width')).toBe('40');
    expect(img?.getAttribute('class')).toContain('test-badge');
  });

  it('renders via unified ModelBrandLogo with brand prop', () => {
    const { container } = render(<ModelBrandLogo brand="whisper" size={24} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('width')).toBe('24');
  });

  it('renders null via ModelBrandLogo for non-ASR models', () => {
    const { container } = render(
      <ModelBrandLogo model={{ id: 'silero-vad', type: 'vad', name: 'Silero VAD' }} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders online ASR brand logos directly and via ModelBrandLogo', () => {
    expect(render(<VolcengineLogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<GroqLogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<MistralLogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<OpenAILogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<DeepgramLogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<AssemblyAiLogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<ElevenLabsLogo size={32} />).container.querySelector('img')).toBeTruthy();
    expect(render(<SenseVoiceLogo size={32} />).container.querySelector('img')).toBeTruthy();

    const brands = [
      'volcengine',
      'groq',
      'mistral',
      'openai',
      'deepgram',
      'assemblyai',
      'elevenlabs',
    ] as const;

    for (const brand of brands) {
      const { container } = render(<ModelBrandLogo brand={brand} size={28} />);
      const img = container.querySelector('img');
      expect(img).toBeTruthy();
      expect(img?.getAttribute('width')).toBe('28');
      expect(img?.getAttribute('class')).toContain('model-brand-logo');
    }
  });
});
