import { useConfigStore } from '../../stores/configStore';
import { logger } from '../../utils/logger';
import { getActiveLlmConfig, getFeatureLlmConfig } from '../llm/configUtils';
import { completeLlm } from '../tauri/llm';
import { getContextDirective, type VoiceTypingContextState } from './voiceTypingContext';

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
    context?: VoiceTypingContextState | null;
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
  const baseSystemPrompt =
    options?.customPrompt?.trim() ||
    config.voiceTypingPolishPrompt?.trim() ||
    DEFAULT_VOICE_TYPING_POLISH_PROMPT;

  const contextDirective = options?.context?.mode
    ? getContextDirective(options.context.mode, options.context.windowTitle)
    : '';

  const systemPrompt = contextDirective
    ? `${baseSystemPrompt}\n\n${contextDirective}`
    : baseSystemPrompt;

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
export const DEFAULT_VOICE_TYPING_TRANSFORM_PROMPT = `你是一个智能文本重写与编辑助手。用户在宿主应用中选中了一段文本，并给出了口述修改指令。
你的任务是根据用户的口述指令对【选中文本】进行编辑、润色、重构或翻译。

规则：
1. 严格按照口述指令修改选中文本。如果指令要求精简，则进行提炼；如果要求翻译，则翻译为目标语言；如果要求修正语气，则调整为指定风格。
2. 严禁生成任何多余问候、解释、代码块标记（\`\`\`）或前后缀，仅直接输出最终处理后的纯文本内容。
3. 保持输出内容真实准确，保留关键专业术语。`;

export async function transformSelectedText(
  selectedText: string,
  instruction: string,
  options?: {
    timeoutMs?: number;
    context?: VoiceTypingContextState | null;
  }
): Promise<string> {
  const trimmedSelected = (selectedText || '').trim();
  const trimmedInstruction = (instruction || '').trim();

  if (!trimmedSelected) return trimmedInstruction;
  if (!trimmedInstruction) return trimmedSelected;

  const config = useConfigStore.getState().config;
  const llmConfig = getFeatureLlmConfig(config, 'polish') ?? getActiveLlmConfig(config);

  if (!llmConfig?.provider) {
    logger.warn(
      '[VoiceTypingPolish] No LLM provider configured for selection transform, using instruction'
    );
    return trimmedInstruction;
  }

  const promptInput = `【选中文本】：\n${trimmedSelected}\n\n【修改指令】：\n${trimmedInstruction}`;
  const timeoutMs = options?.timeoutMs ?? POLISH_TIMEOUT_MS;

  try {
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(
        () => reject(new Error(`LLM transform timed out after ${timeoutMs}ms`)),
        timeoutMs
      );
    });

    const completionPromise = completeLlm({
      config: {
        ...llmConfig,
        temperature: 0.2,
      },
      systemPrompt: options?.context?.mode
        ? `${DEFAULT_VOICE_TYPING_TRANSFORM_PROMPT}\n\n${getContextDirective(
            options.context.mode,
            options.context.windowTitle
          )}`
        : DEFAULT_VOICE_TYPING_TRANSFORM_PROMPT,
      input: promptInput,
      options: {
        maxOutputTokens: 2048,
      },
    });

    const response = await Promise.race([completionPromise, timeoutPromise]);
    const transformed = cleanPolishedOutput(response.text);

    if (!transformed) {
      return trimmedInstruction;
    }

    logger.info('[VoiceTypingPolish] Selection text successfully transformed', {
      originalLength: trimmedSelected.length,
      instructionLength: trimmedInstruction.length,
      transformedLength: transformed.length,
    });
    return transformed;
  } catch (error) {
    logger.warn('[VoiceTypingPolish] Selection transform failed or timed out:', error);
    return trimmedInstruction;
  }
}
