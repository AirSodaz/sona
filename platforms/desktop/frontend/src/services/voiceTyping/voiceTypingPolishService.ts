import { useConfigStore } from '../../stores/configStore';
import { logger } from '../../utils/logger';
import { getActiveLlmConfig, getFeatureLlmConfig } from '../llm/configUtils';
import { completeLlm } from '../tauri/llm';

export const DEFAULT_VOICE_TYPING_POLISH_PROMPT = `你是一个智能语音输入润色助手。你的任务是将用户的语音识别草稿整理为通顺、专业、自然的最终书面文本。
规则：
1. 去除所有无意义口头禅与填充语气词（如“那个”、“然后”、“就是说”、“嗯”、“啊”等）。
2. 修正同音错别字、口语倒装与重复碎句，理顺句子逻辑与表达。
3. 补全或修正标点符号，保持层级分明。
4. 严格保留用户的原意与专有名词，严禁删减核心信息。
5. 严禁生成任何问候、解释、代码块标记（\`\`\`）或前后缀，仅直接输出最终处理后的纯文本内容。`;

const POLISH_TIMEOUT_MS = 6000;

export function cleanPolishedOutput(rawOutput: string): string {
  let cleaned = (rawOutput || '').trim();
  // Strip code block fences if any LLM accidentally output them
  if (cleaned.startsWith('```')) {
    cleaned = cleaned
      .replace(/^```[a-zA-Z]*\n?/, '')
      .replace(/\n?```$/, '')
      .trim();
  }
  // Strip surrounding quotes if wrapped
  if (
    (cleaned.startsWith('"') && cleaned.endsWith('"')) ||
    (cleaned.startsWith('“') && cleaned.endsWith('”'))
  ) {
    cleaned = cleaned.slice(1, -1).trim();
  }
  return cleaned;
}

export async function polishVoiceTypingText(
  text: string,
  options?: {
    customPrompt?: string;
    timeoutMs?: number;
  }
): Promise<string> {
  const trimmed = (text || '').trim();
  if (!trimmed) {
    return '';
  }

  const config = useConfigStore.getState().config;
  const llmConfig = getFeatureLlmConfig(config, 'polish') ?? getActiveLlmConfig(config);

  if (!llmConfig?.provider) {
    logger.warn('[VoiceTypingPolish] No active LLM provider configured, using raw text');
    return trimmed;
  }
  const systemPrompt =
    options?.customPrompt?.trim() ||
    config.voiceTypingPolishPrompt?.trim() ||
    DEFAULT_VOICE_TYPING_POLISH_PROMPT;

  const timeoutMs = options?.timeoutMs ?? POLISH_TIMEOUT_MS;

  try {
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error(`LLM polish timed out after ${timeoutMs}ms`)), timeoutMs);
    });

    const completionPromise = completeLlm({
      config: {
        ...llmConfig,
        temperature: 0.2,
      },
      systemPrompt,
      input: trimmed,
      options: {
        maxOutputTokens: 2048,
      },
    });

    const response = await Promise.race([completionPromise, timeoutPromise]);
    const polished = cleanPolishedOutput(response.text);

    if (!polished) {
      logger.warn('[VoiceTypingPolish] Empty output from LLM polish, falling back to raw text');
      return trimmed;
    }

    logger.info('[VoiceTypingPolish] Text successfully polished', {
      originalLength: trimmed.length,
      polishedLength: polished.length,
    });
    return polished;
  } catch (error) {
    logger.warn(
      '[VoiceTypingPolish] LLM polish failed or timed out, falling back to raw text:',
      error
    );
    return trimmed;
  }
}
