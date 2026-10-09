import type {
  BuiltinLlmProvider_Serialize as CoreBuiltinLlmProvider,
  LlmConfig_Serialize as CoreLlmConfig,
  LlmProvider_Serialize as CoreLlmProvider,
  LlmProviderStrategy as CoreLlmProviderStrategy,
  TranscriptLlmJobRequest_Serialize as CoreTranscriptLlmJobRequest,
  TranscriptSegment_Serialize as CoreTranscriptSegment,
} from '../../bindings';
import type { TranscriptLlmJobRequest } from '../../types/llmTask';
import type { LlmConfig, LlmJsonValue } from '../../types/transcript';

export const PROVIDER_ALIASES: Readonly<Record<string, CoreBuiltinLlmProvider>> = {
  github_copilot: 'copilot',
  openai_compatible: 'custom-openai-compatible',
  open_ai_compatible: 'custom-openai-compatible',
  llama_cpp: 'local',
  local_model: 'local',
};

export function finiteNumber(value: number, path: string): number {
  if (!Number.isFinite(value)) {
    throw new TypeError(`${path} must be a finite number`);
  }
  return value;
}

export function nonNegativeSafeInteger(value: number, path: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${path} must be a non-negative safe integer`);
  }
  return value;
}

export function nullableFiniteNumber(value: number | undefined, path: string): number | null {
  return value === undefined ? null : finiteNumber(value, path);
}

export function nullableNonNegativeSafeInteger(
  value: number | undefined,
  path: string
): number | null {
  return value === undefined ? null : nonNegativeSafeInteger(value, path);
}

export function normalizeJsonValue(value: unknown, path: string): LlmJsonValue {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number') {
    finiteNumber(value, path);
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      throw new TypeError(`${path} must be a safe integer`);
    }
    return value;
  }
  if (Array.isArray(value)) {
    return value.map((item, index) => normalizeJsonValue(item, `${path}[${index}]`));
  }
  if (typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, normalizeJsonValue(item, `${path}.${key}`)])
    );
  }
  throw new TypeError(`${path} must be a JSON value`);
}

export function normalizeProvider(provider: string): CoreLlmProvider {
  if (provider.startsWith('custom-') && provider !== 'custom-openai-compatible') {
    return { Custom: provider };
  }
  return {
    Builtin: PROVIDER_ALIASES[provider] ?? (provider as CoreBuiltinLlmProvider),
  };
}

export function defaultStrategy(provider: CoreLlmProvider): CoreLlmProviderStrategy {
  if ('Custom' in provider) {
    return 'open_ai_compatible';
  }

  switch (provider.Builtin) {
    case 'open_ai':
      return 'open_ai';
    case 'open_ai_responses':
      return 'open_ai_responses';
    case 'azure_openai':
      return 'azure_openai';
    case 'anthropic':
    case 'gemini':
    case 'ollama':
    case 'moonshot_ai':
    case 'moonshot_cn':
    case 'xiaomi':
    case 'perplexity':
    case 'copilot':
    case 'google_translate':
    case 'google_translate_free':
      return provider.Builtin;
    case 'volcengine':
      return 'open_ai_compatible_custom_path';
    default:
      return 'open_ai_compatible';
  }
}

export function normalizeStrategy(strategy: string): CoreLlmProviderStrategy {
  return strategy as CoreLlmProviderStrategy;
}

export function normalizeConfig(config: LlmConfig, path: string): CoreLlmConfig {
  const provider = normalizeProvider(config.provider);
  return {
    provider,
    strategy: config.strategy ? normalizeStrategy(config.strategy) : defaultStrategy(provider),
    baseUrl: config.baseUrl,
    apiKey: config.apiKey,
    model: config.model,
    apiPath: config.apiPath ?? null,
    apiVersion: config.apiVersion ?? null,
    temperature: nullableFiniteNumber(config.temperature, `${path}.temperature`),
    reasoningEnabled: config.reasoningEnabled ?? null,
    reasoningLevel: config.reasoningLevel ?? null,
    timeoutSeconds: nullableNonNegativeSafeInteger(config.timeoutSeconds, `${path}.timeoutSeconds`),
  };
}

export function normalizeSummaryTemplate(template: {
  id: string;
  name: string;
  instructions: string;
}): NonNullable<CoreTranscriptLlmJobRequest['template']> {
  return {
    id: template.id,
    name: template.name,
    instructions: template.instructions,
  };
}

export function normalizeTranscriptSegment(
  segment: TranscriptLlmJobRequest['segments'][number],
  path: string
): CoreTranscriptSegment {
  return {
    id: segment.id,
    text: segment.text,
    start: finiteNumber(segment.start, `${path}.start`),
    end: finiteNumber(segment.end, `${path}.end`),
    isFinal: segment.isFinal,
    ...(segment.timing ? { timing: segment.timing } : {}),
    ...(segment.tokens ? { tokens: segment.tokens } : {}),
    ...(segment.timestamps ? { timestamps: segment.timestamps } : {}),
    ...(segment.translation ? { translation: segment.translation } : {}),
    ...(segment.durations
      ? {
          durations: segment.durations.map((value, index) =>
            finiteNumber(value, `${path}.durations[${index}]`)
          ),
        }
      : {}),
    ...(segment.speaker?.score === undefined
      ? {}
      : {
          speaker: {
            ...segment.speaker,
            score: finiteNumber(segment.speaker.score, `${path}.speaker.score`),
          },
        }),
    ...(segment.speakerAttribution
      ? {
          speakerAttribution: {
            ...segment.speakerAttribution,
            candidates: segment.speakerAttribution.candidates.map((candidate, index) => ({
              ...candidate,
              score: finiteNumber(
                candidate.score,
                `${path}.speakerAttribution.candidates[${index}].score`
              ),
              rank: nonNegativeSafeInteger(
                candidate.rank,
                `${path}.speakerAttribution.candidates[${index}].rank`
              ),
            })),
          },
        }
      : {}),
  };
}

export function normalizeTranscriptJobRequest(
  request: TranscriptLlmJobRequest
): CoreTranscriptLlmJobRequest {
  return {
    taskId: request.taskId,
    taskType: request.taskType,
    jobHistoryId: request.jobHistoryId ?? null,
    config: normalizeConfig(request.config, 'request.config'),
    segments: request.segments.map((segment, index) =>
      normalizeTranscriptSegment(segment, `request.segments[${index}]`)
    ),
    targetLanguage: 'targetLanguage' in request ? request.targetLanguage : null,
    targetLanguageName:
      'targetLanguageName' in request ? (request.targetLanguageName ?? null) : null,
    context: 'context' in request ? (request.context ?? null) : null,
    keywords: 'keywords' in request ? (request.keywords ?? null) : null,
    mode: 'mode' in request ? (request.mode ?? null) : null,
    template:
      'template' in request && request.template ? normalizeSummaryTemplate(request.template) : null,
    chunkSize: null,
    chunkCharBudget: null,
  };
}
