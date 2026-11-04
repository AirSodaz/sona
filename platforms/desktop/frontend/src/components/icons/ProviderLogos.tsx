import type React from 'react';
import anthropicSvg from '../../assets/providers/anthropic.svg';
import anyscaleSvg from '../../assets/providers/anyscale.svg';
import azureSvg from '../../assets/providers/azure.svg';
import baiduSvg from '../../assets/providers/baidu.svg';
import basetenSvg from '../../assets/providers/baseten.svg';
import bedrockSvg from '../../assets/providers/bedrock.svg';
import cerebrasSvg from '../../assets/providers/cerebras.svg';
import cloudflareSvg from '../../assets/providers/cloudflare.svg';
import cohereSvg from '../../assets/providers/cohere.svg';
import copilotSvg from '../../assets/providers/copilot.svg';
import databricksSvg from '../../assets/providers/databricks.svg';
import deepinfraSvg from '../../assets/providers/deepinfra.svg';
import deepseekSvg from '../../assets/providers/deepseek.svg';
import falSvg from '../../assets/providers/fal.svg';
import fireworksSvg from '../../assets/providers/fireworks.svg';
import friendliaiSvg from '../../assets/providers/friendliai.svg';
import geminiSvg from '../../assets/providers/gemini.svg';
import gladiaSvg from '../../assets/providers/gladia.svg';
import googletranslateSvg from '../../assets/providers/googletranslate.svg';
import groqSvg from '../../assets/providers/groq.svg';
import huggingfaceSvg from '../../assets/providers/huggingface.svg';
import hyperbolicSvg from '../../assets/providers/hyperbolic.svg';
import layaSvg from '../../assets/providers/laya.svg';
import llamafileSvg from '../../assets/providers/llamafile.svg';
import lmstudioSvg from '../../assets/providers/lmstudio.svg';
import minimaxSvg from '../../assets/providers/minimax.svg';
import mistralSvg from '../../assets/providers/mistral.svg';
import moonshotSvg from '../../assets/providers/moonshot.svg';
import novitaSvg from '../../assets/providers/novita.svg';
import nvidiaSvg from '../../assets/providers/nvidia.svg';
import ollamaSvg from '../../assets/providers/ollama.svg';
import openaiSvg from '../../assets/providers/openai.svg';
import openrouterSvg from '../../assets/providers/openrouter.svg';
import perplexitySvg from '../../assets/providers/perplexity.svg';
import qwenSvg from '../../assets/providers/qwen.svg';
import revaiSvg from '../../assets/providers/revai.svg';
import siliconflowSvg from '../../assets/providers/siliconflow.svg';
import stepfunSvg from '../../assets/providers/stepfun.svg';
import tencentSvg from '../../assets/providers/tencent.svg';
import togetherSvg from '../../assets/providers/together.svg';
import veniceSvg from '../../assets/providers/venice.svg';
import vllmSvg from '../../assets/providers/vllm.svg';
import volcengineSvg from '../../assets/providers/volcengine.svg';
import xaiSvg from '../../assets/providers/xai.svg';
import xiaomiSvg from '../../assets/providers/xiaomi.svg';
import zerooneSvg from '../../assets/providers/zeroone.svg';
import zhipuSvg from '../../assets/providers/zhipu.svg';

export interface ProviderIconMeta {
  src: string;
  monochrome?: boolean;
}

export const PROVIDER_ICON_MAP: Record<string, ProviderIconMeta> = {
  // OpenAI
  open_ai: { src: openaiSvg, monochrome: true },
  open_ai_responses: { src: openaiSvg, monochrome: true },
  // Azure
  azure_openai: { src: azureSvg },
  // Anthropic
  anthropic: { src: anthropicSvg },
  // Google Gemini
  gemini: { src: geminiSvg },
  // Ollama
  ollama: { src: ollamaSvg, monochrome: true },
  // DeepSeek
  deep_seek: { src: deepseekSvg },
  // Moonshot / Kimi
  moonshot_ai: { src: moonshotSvg, monochrome: true },
  moonshot_cn: { src: moonshotSvg, monochrome: true },
  kimi: { src: moonshotSvg, monochrome: true },
  // Xiaomi
  xiaomi: { src: xiaomiSvg },
  // SiliconFlow
  silicon_flow: { src: siliconflowSvg },
  // Qwen
  qwen: { src: qwenSvg },
  qwen_portal: { src: qwenSvg },
  // MiniMax
  minimax_global: { src: minimaxSvg },
  minimax_cn: { src: minimaxSvg },
  // OpenRouter
  openrouter: { src: openrouterSvg, monochrome: true },
  // LM Studio
  lm_studio: { src: lmstudioSvg, monochrome: true },
  // Groq
  groq: { src: groqSvg, monochrome: true },
  // xAI
  x_ai: { src: xaiSvg, monochrome: true },
  // Mistral AI
  mistral_ai: { src: mistralSvg },
  // Perplexity
  perplexity: { src: perplexitySvg },
  // Volcengine
  volcengine: { src: volcengineSvg },
  // Zhipu / ChatGLM
  chatglm: { src: zhipuSvg },
  // GitHub Copilot
  copilot: { src: copilotSvg, monochrome: true },
  github_copilot: { src: copilotSvg, monochrome: true },
  // Cohere
  cohere: { src: cohereSvg },
  // Together AI
  together: { src: togetherSvg },
  // Venice AI
  venice: { src: veniceSvg },
  // Hyperbolic
  hyperbolic: { src: hyperbolicSvg },
  // Llamafile
  llamafile: { src: llamafileSvg, monochrome: true },
  // Baidu Qianfan
  baidu: { src: baiduSvg },
  // Tencent Hunyuan
  tencent: { src: tencentSvg },
  // StepFun
  stepfun: { src: stepfunSvg },
  // 01.AI (Yi)
  lingyiwanwu: { src: zerooneSvg },
  // Cerebras
  cerebras: { src: cerebrasSvg },
  // Google Translate
  google_translate: { src: googletranslateSvg },
  google_translate_free: { src: googletranslateSvg },
  // Amazon Bedrock
  amazon_bedrock: { src: bedrockSvg },
  bedrock: { src: bedrockSvg },
  // Cloudflare Workers AI
  cloudflare_workers_ai: { src: cloudflareSvg },
  cloudflare: { src: cloudflareSvg },
  // Hugging Face
  huggingface: { src: huggingfaceSvg },
  hugging_face: { src: huggingfaceSvg },
  // NVIDIA NIM
  nvidia_nim: { src: nvidiaSvg },
  nvidia: { src: nvidiaSvg },
  // Fireworks
  fireworks: { src: fireworksSvg },
  fireworks_ai: { src: fireworksSvg },
  // DeepInfra
  deepinfra: { src: deepinfraSvg },
  deep_infra: { src: deepinfraSvg },
  // Novita
  novita: { src: novitaSvg },
  novita_ai: { src: novitaSvg },
  // Anyscale
  anyscale: { src: anyscaleSvg },
  // Baseten
  baseten: { src: basetenSvg, monochrome: true },
  // Databricks
  databricks: { src: databricksSvg },
  // FriendliAI
  friendliai: { src: friendliaiSvg },
  friendli_ai: { src: friendliaiSvg },
  friendli: { src: friendliaiSvg },
  // vLLM
  vllm: { src: vllmSvg },
  // Laya
  laya: { src: layaSvg },
  // ASR Providers (supporting unified lookups in ProviderLogo)
  azure_speech: { src: azureSvg },
  'azure-speech': { src: azureSvg },
  google_gemini: { src: geminiSvg },
  'google-gemini': { src: geminiSvg },
  xai: { src: xaiSvg, monochrome: true },
  gladia: { src: gladiaSvg },
  gladia_v2: { src: gladiaSvg },
  'gladia-v2': { src: gladiaSvg },
  revai: { src: revaiSvg },
  rev_ai: { src: revaiSvg },
  'rev-ai': { src: revaiSvg },
  fal_whisper: { src: falSvg },
  'fal-whisper': { src: falSvg },
  fal: { src: falSvg },
};

export interface ProviderLogoProps extends React.ImgHTMLAttributes<HTMLImageElement> {
  provider?: string | null;
  label?: string;
  size?: number;
  className?: string;
}

export function ProviderLogo({
  provider,
  label = '',
  size = 28,
  className = '',
  alt,
  ...props
}: ProviderLogoProps): React.JSX.Element {
  const iconEntry = provider ? PROVIDER_ICON_MAP[provider] : null;

  if (iconEntry) {
    return (
      <img
        src={iconEntry.src}
        alt={alt !== undefined ? alt : label || provider || 'Provider logo'}
        width={size}
        height={size}
        className={`provider-brand-logo ${iconEntry.monochrome ? 'provider-brand-logo-monochrome' : ''} ${className}`.trim()}
        style={{ width: `${size}px`, height: `${size}px` }}
        loading="lazy"
        decoding="async"
        {...props}
      />
    );
  }

  const initial = (label || provider || '?').charAt(0).toUpperCase();

  return (
    <span
      className={`provider-picker-fallback-icon ${className}`.trim()}
      style={{
        width: `${size}px`,
        height: `${size}px`,
        fontSize: `${Math.max(10, Math.round(size * 0.42))}px`,
      }}
      aria-hidden="true"
    >
      {initial}
    </span>
  );
}

export default ProviderLogo;
