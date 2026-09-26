import type React from 'react';
import assemblyaiImg from '../../assets/models/assemblyai.png';
import deepgramImg from '../../assets/models/deepgram.png';
import dolphinImg from '../../assets/models/dolphin.png';
import elevenlabsImg from '../../assets/models/elevenlabs.png';
import fireredImg from '../../assets/models/firered.png';
import funasrImg from '../../assets/models/funasr.png';
import groqImg from '../../assets/models/groq.png';
import metaImg from '../../assets/models/meta.png';
import mistralImg from '../../assets/models/mistral.png';
import moonshineImg from '../../assets/models/moonshine.png';
import nvidiaImg from '../../assets/models/nvidia.png';
import openaiImg from '../../assets/models/openai.png';
import paraformerImg from '../../assets/models/paraformer.png';
import sensevoiceImg from '../../assets/models/sensevoice.png';
import volcengineImg from '../../assets/models/volcengine.png';
import whisperImg from '../../assets/models/whisper.png';
import zipformerImg from '../../assets/models/zipformer.png';

export type AsrBrand =
  | 'qwen'
  | 'sensevoice'
  | 'firered'
  | 'whisper'
  | 'meta'
  | 'paraformer'
  | 'funasr-nano'
  | 'zipformer'
  | 'nvidia'
  | 'dolphin'
  | 'moonshine'
  | 'volcengine'
  | 'groq'
  | 'mistral'
  | 'openai'
  | 'deepgram'
  | 'assemblyai'
  | 'elevenlabs'
  | 'generic';

export interface ModelLogoProps extends React.ImgHTMLAttributes<HTMLImageElement> {
  size?: number;
}

export interface GenericLogoProps extends React.SVGProps<SVGSVGElement> {
  size?: number;
}

/** 1. Qwen (通义千问) Logo: Official QwenAudio project avatar (unified with SenseVoice) */
export function QwenLogo({
  size = 32,
  className = '',
  alt = 'Qwen',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={sensevoiceImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 2. SenseVoice (QwenAudio) Logo: Official QwenAudio project avatar */
export function SenseVoiceLogo({
  size = 32,
  className = '',
  alt = 'SenseVoice',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={sensevoiceImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 3. FireRed (小红书 FireRedASR) Logo: Official FireRedTeam project avatar */
export function FireRedLogo({
  size = 32,
  className = '',
  alt = 'FireRedASR',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={fireredImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 4. Whisper (OpenAI) Logo: Official OpenAI project avatar */
export function WhisperLogo({
  size = 32,
  className = '',
  alt = 'Whisper',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={whisperImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 5. Meta (Omnilingual ASR) Logo: Official Meta / facebookresearch project avatar */
export function MetaLogo({
  size = 32,
  className = '',
  alt = 'Meta Omnilingual',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={metaImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 6. Paraformer (阿里 FunASR / ModelScope) Logo: Official ModelScope project avatar */
export function ParaformerLogo({
  size = 32,
  className = '',
  alt = 'Paraformer',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={paraformerImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 7. FunASR Nano (阿里 FunASR / ModelScope) Logo: Official ModelScope project avatar */
export function FunAsrNanoLogo({
  size = 32,
  className = '',
  alt = 'FunASR Nano',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={funasrImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 8. Zipformer (k2-fsa) Logo: Official k2-fsa project avatar */
export function ZipformerLogo({
  size = 32,
  className = '',
  alt = 'Zipformer',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={zipformerImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 9. Parakeet TDT (NVIDIA NeMo) Logo: Official NVIDIA emblem without text */
export function NvidiaLogo({
  size = 32,
  className = '',
  alt = 'NVIDIA Parakeet',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={nvidiaImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 10. Dolphin (Dataocean AI) Logo: Official DataoceanAI project avatar */
export function DolphinLogo({
  size = 32,
  className = '',
  alt = 'Dolphin',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={dolphinImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 11. Moonshine Logo: Official Moonshine AI project logo (https://github.com/moonshine-ai/moonshine) */
export function MoonshineLogo({
  size = 32,
  className = '',
  alt = 'Moonshine',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={moonshineImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 12. Volcengine / Doubao (火山引擎 / 豆包) Logo */
export function VolcengineLogo({
  size = 32,
  className = '',
  alt = 'Volcengine',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={volcengineImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 13. Groq Logo */
export function GroqLogo({
  size = 32,
  className = '',
  alt = 'Groq',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={groqImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 14. Mistral AI Logo */
export function MistralLogo({
  size = 32,
  className = '',
  alt = 'Mistral AI',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={mistralImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 15. OpenAI Logo */
export function OpenAILogo({
  size = 32,
  className = '',
  alt = 'OpenAI',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={openaiImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 16. Deepgram Logo */
export function DeepgramLogo({
  size = 32,
  className = '',
  alt = 'Deepgram',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={deepgramImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 17. AssemblyAI Logo */
export function AssemblyAiLogo({
  size = 32,
  className = '',
  alt = 'AssemblyAI',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={assemblyaiImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** 18. ElevenLabs Logo */
export function ElevenLabsLogo({
  size = 32,
  className = '',
  alt = 'ElevenLabs',
  ...props
}: ModelLogoProps): React.JSX.Element {
  return (
    <img
      src={elevenlabsImg}
      alt={alt}
      width={size}
      height={size}
      className={`model-brand-logo ${className}`.trim()}
      loading="lazy"
      decoding="async"
      {...props}
    />
  );
}

/** Generic fallback model logo */
export function GenericModelLogo({ size = 32, ...props }: GenericLogoProps): React.JSX.Element {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      {...props}
    >
      <rect width="48" height="48" rx="10" fill="#1E293B" />
      {/* CPU / Neural Chip with speech audio waveform */}
      <rect x="14" y="14" width="20" height="20" rx="4" stroke="#94A3B8" strokeWidth="2" />
      <path
        d="M19 24V24M22 21V27M25 18V30M28 22V26M31 24V24"
        stroke="#38BDF8"
        strokeWidth="2.5"
        strokeLinecap="round"
      />
      <path
        d="M20 10V14M24 10V14M28 10V14M20 34V38M24 34V38M28 34V38M10 20H14M10 24H14M10 28H14M34 20H38M34 24H38M34 28H38"
        stroke="#64748B"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  );
}

export const ASR_MODEL_TYPES = new Set([
  'zipformer',
  'sensevoice',
  'paraformer',
  'whisper',
  'funasr-nano',
  'fire-red-asr',
  'dolphin',
  'qwen3-asr',
  'parakeet-tdt',
  'moonshine',
  'omnilingual',
  'volcengine',
  'doubao',
  'groq',
  'mistral',
  'voxtral',
  'openai',
  'deepgram',
  'assemblyai',
  'elevenlabs',
]);

const NON_ASR_MODEL_TYPES = new Set([
  'punctuation',
  'vad',
  'itn',
  'speaker-segmentation',
  'speaker-embedding',
]);

/**
 * Checks whether a given model is a speech recognition (ASR) model.
 */
export function isAsrModel(model: {
  id?: string;
  groupId?: string;
  name?: string;
  type?: string;
}): boolean {
  if (model.type && ASR_MODEL_TYPES.has(model.type)) {
    return true;
  }
  if (model.type && NON_ASR_MODEL_TYPES.has(model.type)) {
    return false;
  }
  const token =
    `${model.groupId ?? ''} ${model.id ?? ''} ${model.type ?? ''} ${model.name ?? ''}`.toLowerCase();
  if (
    (token.includes('punct') && !token.includes('asr') && !token.includes('zipformer')) ||
    token.includes('ct-transformer') ||
    token.includes('vad') ||
    token.includes('silero') ||
    token.includes('speaker') ||
    token.includes('pyannote') ||
    token.includes('campplus') ||
    token.includes('eres2net')
  ) {
    return false;
  }
  if (
    token.includes('asr') ||
    token.includes('speech') ||
    token.includes('recogni') ||
    token.includes('qwen') ||
    token.includes('sense-voice') ||
    token.includes('sensevoice') ||
    token.includes('fire-red') ||
    token.includes('firered') ||
    token.includes('whisper') ||
    token.includes('omnilingual') ||
    token.includes('paraformer') ||
    token.includes('funasr') ||
    token.includes('zipformer') ||
    token.includes('icefall') ||
    token.includes('k2') ||
    token.includes('parakeet') ||
    token.includes('nemo') ||
    token.includes('dolphin') ||
    token.includes('moonshine') ||
    token.includes('volcengine') ||
    token.includes('doubao') ||
    token.includes('groq') ||
    token.includes('mistral') ||
    token.includes('voxtral') ||
    token.includes('deepgram') ||
    token.includes('assemblyai') ||
    token.includes('assembly') ||
    token.includes('elevenlabs')
  ) {
    return true;
  }
  return false;
}

/**
 * Resolves an ASR model's brand identity based on its ID, group ID, or model type.
 * Returns null for non-ASR models.
 */
export function resolveModelBrand(model: {
  id?: string;
  groupId?: string;
  name?: string;
  type?: string;
}): AsrBrand | null {
  if (!isAsrModel(model)) {
    return null;
  }

  const token =
    `${model.groupId ?? ''} ${model.id ?? ''} ${model.type ?? ''} ${model.name ?? ''}`.toLowerCase();

  if (token.includes('volcengine') || token.includes('doubao')) {
    return 'volcengine';
  }
  if (token.includes('groq')) {
    return 'groq';
  }
  if (token.includes('mistral') || token.includes('voxtral')) {
    return 'mistral';
  }
  if (token.includes('openai')) {
    return 'openai';
  }
  if (token.includes('deepgram')) {
    return 'deepgram';
  }
  if (token.includes('assemblyai') || token.includes('assembly')) {
    return 'assemblyai';
  }
  if (token.includes('elevenlabs')) {
    return 'elevenlabs';
  }
  if (token.includes('qwen')) {
    return 'qwen';
  }
  if (token.includes('sense-voice') || token.includes('sensevoice')) {
    return 'sensevoice';
  }
  if (token.includes('fire-red') || token.includes('firered')) {
    return 'firered';
  }
  if (token.includes('whisper')) {
    return 'whisper';
  }
  if (token.includes('omnilingual')) {
    return 'meta';
  }
  if (token.includes('paraformer')) {
    return 'paraformer';
  }
  if (token.includes('funasr-nano') || token.includes('funasr_nano') || token.includes('funasr')) {
    return 'funasr-nano';
  }
  if (
    token.includes('zipformer') ||
    token.includes('icefall') ||
    token.includes('k2') ||
    token.includes('x-asr')
  ) {
    return 'zipformer';
  }
  if (token.includes('parakeet') || token.includes('nemo')) {
    return 'nvidia';
  }
  if (token.includes('dolphin')) {
    return 'dolphin';
  }
  if (token.includes('moonshine')) {
    return 'moonshine';
  }

  return 'generic';
}

export interface ModelBrandLogoProps extends React.HTMLAttributes<HTMLElement> {
  brand?: AsrBrand;
  model?: { id?: string; groupId?: string; name?: string; type?: string };
  size?: number;
  className?: string;
  alt?: string;
}

/**
 * Unified Model Brand Logo component for ASR models.
 * Pass either `brand` or the `model` metadata object directly.
 * Non-ASR models render null.
 */
export function ModelBrandLogo({
  brand,
  model,
  size = 36,
  className = '',
  ...props
}: ModelBrandLogoProps): React.JSX.Element | null {
  const resolvedBrand = brand ?? (model ? resolveModelBrand(model) : null);
  if (!resolvedBrand) {
    return null;
  }

  switch (resolvedBrand) {
    case 'qwen':
      return <QwenLogo size={size} className={className} {...props} />;
    case 'sensevoice':
      return <SenseVoiceLogo size={size} className={className} {...props} />;
    case 'firered':
      return <FireRedLogo size={size} className={className} {...props} />;
    case 'whisper':
      return <WhisperLogo size={size} className={className} {...props} />;
    case 'meta':
      return <MetaLogo size={size} className={className} {...props} />;
    case 'paraformer':
      return <ParaformerLogo size={size} className={className} {...props} />;
    case 'funasr-nano':
      return <FunAsrNanoLogo size={size} className={className} {...props} />;
    case 'zipformer':
      return <ZipformerLogo size={size} className={className} {...props} />;
    case 'nvidia':
      return <NvidiaLogo size={size} className={className} {...props} />;
    case 'dolphin':
      return <DolphinLogo size={size} className={className} {...props} />;
    case 'moonshine':
      return <MoonshineLogo size={size} className={className} {...props} />;
    case 'volcengine':
      return <VolcengineLogo size={size} className={className} {...props} />;
    case 'groq':
      return <GroqLogo size={size} className={className} {...props} />;
    case 'mistral':
      return <MistralLogo size={size} className={className} {...props} />;
    case 'openai':
      return <OpenAILogo size={size} className={className} {...props} />;
    case 'deepgram':
      return <DeepgramLogo size={size} className={className} {...props} />;
    case 'assemblyai':
      return <AssemblyAiLogo size={size} className={className} {...props} />;
    case 'elevenlabs':
      return <ElevenLabsLogo size={size} className={className} {...props} />;
    case 'generic':
      return <GenericModelLogo size={size} className={className} />;
    default:
      return null;
  }
}
