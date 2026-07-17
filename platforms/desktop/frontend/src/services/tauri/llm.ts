import type {
  BuiltinLlmProvider_Serialize as CoreBuiltinLlmProvider,
  LlmCompletionOptions as CoreLlmCompletionOptions,
  LlmCompletionRequest_Serialize as CoreLlmCompletionRequest,
  LlmCompletionResponse_Serialize as CoreLlmCompletionResponse,
  LlmConfig_Serialize as CoreLlmConfig,
  LlmGenerateRequest_Serialize as CoreLlmGenerateRequest,
  LlmModelSummary as CoreLlmModelSummary,
  LlmModelsRequest_Serialize as CoreLlmModelsRequest,
  LlmProvider_Serialize as CoreLlmProvider,
  LlmProviderStrategy as CoreLlmProviderStrategy,
  LlmResponseFormat as CoreLlmResponseFormat,
  PolishSegmentsRequest_Serialize as CorePolishSegmentsRequest,
  SummarizeTranscriptRequest_Serialize as CoreSummarizeTranscriptRequest,
  TranscriptLlmJobRequest_Serialize as CoreTranscriptLlmJobRequest,
  TranscriptSegment_Serialize as CoreTranscriptSegment,
  TranslateSegmentsRequest_Serialize as CoreTranslateSegmentsRequest,
} from '../../bindings';
import type {
  PolishedSegment,
  PolishSegmentsRequest,
  SummarizeTranscriptRequest,
  TranscriptLlmJobRequest,
  TranscriptLlmJobResult,
  TranscriptSummaryResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from '../llmTaskTypes';
import type { LlmGenerateCommandRequest } from '../../types/dashboard';
import type {
  LlmCompletionRequest,
  LlmCompletionResponse,
  LlmConfig,
  LlmDiscoveredModelSummary,
  LlmJsonValue,
  LlmResponseFormat,
} from '../../types/transcript';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

const STRATEGY_ALIASES: Readonly<Record<string, CoreLlmProviderStrategy>> = {
  openai_compatible: 'open_ai_compatible',
  openai_compatible_custom_path: 'open_ai_compatible_custom_path',
  openai_responses: 'open_ai_responses',
};

const PROVIDER_ALIASES: Readonly<Record<string, CoreBuiltinLlmProvider>> = {
  github_copilot: 'copilot',
  openai_compatible: 'custom-openai-compatible',
  open_ai_compatible: 'custom-openai-compatible',
};

function finiteNumber(value: number, path: string): number {
  if (!Number.isFinite(value)) {
    throw new TypeError(`${path} must be a finite number`);
  }
  return value;
}

function nonNegativeSafeInteger(value: number, path: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${path} must be a non-negative safe integer`);
  }
  return value;
}

function nullableFiniteNumber(value: number | undefined, path: string): number | null {
  return value === undefined ? null : finiteNumber(value, path);
}

function nullableNonNegativeSafeInteger(
  value: number | undefined,
  path: string,
): number | null {
  return value === undefined ? null : nonNegativeSafeInteger(value, path);
}

function normalizeJsonValue(value: unknown, path: string): LlmJsonValue {
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
      Object.entries(value).map(([key, item]) => [
        key,
        normalizeJsonValue(item, `${path}.${key}`),
      ]),
    );
  }
  throw new TypeError(`${path} must be a JSON value`);
}

function normalizeProvider(provider: string): CoreLlmProvider {
  if (provider.startsWith('custom-') && provider !== 'custom-openai-compatible') {
    return { Custom: provider };
  }
  return {
    Builtin: PROVIDER_ALIASES[provider] ?? provider as CoreBuiltinLlmProvider,
  };
}

function defaultStrategy(provider: CoreLlmProvider): CoreLlmProviderStrategy {
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

function normalizeStrategy(strategy: string): CoreLlmProviderStrategy {
  return STRATEGY_ALIASES[strategy] ?? strategy as CoreLlmProviderStrategy;
}

function normalizeConfig(config: LlmConfig, path: string): CoreLlmConfig {
  const provider = normalizeProvider(config.provider);
  return {
    provider,
    strategy: config.strategy
      ? normalizeStrategy(config.strategy)
      : defaultStrategy(provider),
    baseUrl: config.baseUrl,
    apiKey: config.apiKey,
    model: config.model,
    apiPath: config.apiPath ?? null,
    apiVersion: config.apiVersion ?? null,
    temperature: nullableFiniteNumber(config.temperature, `${path}.temperature`),
    reasoningEnabled: config.reasoningEnabled ?? null,
    reasoningLevel: config.reasoningLevel ?? null,
    timeoutSeconds: nullableNonNegativeSafeInteger(
      config.timeoutSeconds,
      `${path}.timeoutSeconds`,
    ),
  };
}

function normalizeResponseFormat(
  format: LlmResponseFormat | undefined,
  path: string,
): CoreLlmResponseFormat {
  if (!format) {
    return { type: 'text' };
  }
  if (format.type !== 'json_schema') {
    return { type: format.type };
  }
  return {
    type: 'json_schema',
    name: format.name,
    schema: normalizeJsonValue(format.schema, `${path}.schema`),
  };
}

function normalizeCompletionOptions(
  options: LlmCompletionRequest['options'],
  path: string,
): CoreLlmCompletionOptions {
  return {
    temperature: nullableFiniteNumber(options?.temperature, `${path}.temperature`),
    maxOutputTokens: nullableNonNegativeSafeInteger(
      options?.maxOutputTokens,
      `${path}.maxOutputTokens`,
    ),
    reasoningEnabled: options?.reasoningEnabled ?? null,
    reasoningLevel: options?.reasoningLevel ?? null,
    responseFormat: normalizeResponseFormat(options?.responseFormat, `${path}.responseFormat`),
    promptCache: options?.promptCache ?? 'disabled',
    capabilityPolicy: options?.capabilityPolicy ?? 'compatible',
  };
}

function normalizeCompletionRequest(request: LlmCompletionRequest): CoreLlmCompletionRequest {
  return {
    config: normalizeConfig(request.config, 'request.config'),
    systemPrompt: request.systemPrompt ?? null,
    input: request.input,
    options: normalizeCompletionOptions(request.options, 'request.options'),
    source: request.source ?? null,
  };
}

function normalizeGenerateRequest(request: LlmGenerateCommandRequest): CoreLlmGenerateRequest {
  return {
    config: normalizeConfig(request.config, 'request.config'),
    input: request.input,
    source: request.source ?? null,
  };
}

function normalizeModelsRequest(request: {
  provider: string;
  strategy?: string;
  baseUrl: string;
  apiKey: string;
}): CoreLlmModelsRequest {
  return {
    provider: normalizeProvider(request.provider),
    strategy: request.strategy ? normalizeStrategy(request.strategy) : null,
    baseUrl: request.baseUrl,
    apiKey: request.apiKey,
  };
}

function normalizeModelSummary(
  summary: CoreLlmModelSummary,
  path: string,
): LlmDiscoveredModelSummary {
  for (const key of ['inputPrice', 'outputPrice', 'cacheReadPrice', 'cacheWritePrice'] as const) {
    const value = summary[key];
    if (value != null) {
      finiteNumber(value, `${path}.${key}`);
    }
  }
  for (const key of ['contextWindow', 'maxOutputTokens'] as const) {
    const value = summary[key];
    if (value != null) {
      nonNegativeSafeInteger(value, `${path}.${key}`);
    }
  }
  return summary;
}

function normalizeCompletionResponse(
  response: CoreLlmCompletionResponse,
): LlmCompletionResponse {
  const json = response.json == null
    ? undefined
    : normalizeJsonValue(response.json, 'result.json');
  const usage = response.usage == null
    ? null
    : {
        promptTokens: nonNegativeSafeInteger(response.usage.promptTokens, 'result.usage.promptTokens'),
        completionTokens: nonNegativeSafeInteger(
          response.usage.completionTokens,
          'result.usage.completionTokens',
        ),
        totalTokens: nonNegativeSafeInteger(response.usage.totalTokens, 'result.usage.totalTokens'),
        cachedInputTokens: nonNegativeSafeInteger(
          response.usage.cachedInputTokens ?? 0,
          'result.usage.cachedInputTokens',
        ),
        cacheCreationInputTokens: nonNegativeSafeInteger(
          response.usage.cacheCreationInputTokens ?? 0,
          'result.usage.cacheCreationInputTokens',
        ),
        reasoningTokens: nonNegativeSafeInteger(
          response.usage.reasoningTokens ?? 0,
          'result.usage.reasoningTokens',
        ),
      };

  nonNegativeSafeInteger(response.execution.attempts, 'result.execution.attempts');
  return {
    text: response.text,
    ...(json === undefined ? {} : { json }),
    usage,
    execution: response.execution,
  };
}

function normalizePolishRequest(request: PolishSegmentsRequest): CorePolishSegmentsRequest {
  return {
    taskId: request.taskId,
    config: normalizeConfig(request.config, 'request.config'),
    segments: request.segments,
    chunkSize: nullableNonNegativeSafeInteger(request.chunkSize, 'request.chunkSize'),
    context: request.context ?? null,
    keywords: request.keywords ?? null,
  };
}

function normalizeTranslateRequest(request: TranslateSegmentsRequest): CoreTranslateSegmentsRequest {
  return {
    taskId: request.taskId,
    config: normalizeConfig(request.config, 'request.config'),
    segments: request.segments,
    chunkSize: nullableNonNegativeSafeInteger(request.chunkSize, 'request.chunkSize'),
    targetLanguage: request.targetLanguage,
    targetLanguageName: request.targetLanguageName ?? null,
  };
}

function normalizeSummaryTemplate(template: {
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

function normalizeSummarizeRequest(
  request: SummarizeTranscriptRequest,
): CoreSummarizeTranscriptRequest {
  return {
    taskId: request.taskId,
    config: normalizeConfig(request.config, 'request.config'),
    template: normalizeSummaryTemplate(request.template),
    segments: request.segments.map((segment, index) => ({
      ...segment,
      start: finiteNumber(segment.start, `request.segments[${index}].start`),
      end: finiteNumber(segment.end, `request.segments[${index}].end`),
    })),
    chunkCharBudget: nullableNonNegativeSafeInteger(
      request.chunkCharBudget,
      'request.chunkCharBudget',
    ),
  };
}

function normalizeTranscriptSegment(
  segment: TranscriptLlmJobRequest['segments'][number],
  path: string,
): CoreTranscriptSegment {
  return {
    ...segment,
    start: finiteNumber(segment.start, `${path}.start`),
    end: finiteNumber(segment.end, `${path}.end`),
    ...(segment.timing
      ? {
          timing: {
            ...segment.timing,
            units: segment.timing.units.map((unit, index) => ({
              ...unit,
              start: finiteNumber(unit.start, `${path}.timing.units[${index}].start`),
              end: finiteNumber(unit.end, `${path}.timing.units[${index}].end`),
            })),
          },
        }
      : {}),
    ...(segment.timestamps
      ? {
          timestamps: segment.timestamps.map((value, index) => (
            finiteNumber(value, `${path}.timestamps[${index}]`)
          )),
        }
      : {}),
    ...(segment.durations
      ? {
          durations: segment.durations.map((value, index) => (
            finiteNumber(value, `${path}.durations[${index}]`)
          )),
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
                `${path}.speakerAttribution.candidates[${index}].score`,
              ),
              rank: nonNegativeSafeInteger(
                candidate.rank,
                `${path}.speakerAttribution.candidates[${index}].rank`,
              ),
            })),
          },
        }
      : {}),
  };
}

type CoreTranscriptJobFields = Pick<
  CoreTranscriptLlmJobRequest,
  | 'targetLanguage'
  | 'targetLanguageName'
  | 'context'
  | 'keywords'
  | 'template'
  | 'chunkSize'
  | 'chunkCharBudget'
>;

function normalizeTranscriptJobFields(
  request: TranscriptLlmJobRequest,
): CoreTranscriptJobFields {
  const emptyFields: CoreTranscriptJobFields = {
    targetLanguage: null,
    targetLanguageName: null,
    context: null,
    keywords: null,
    template: null,
    chunkSize: null,
    chunkCharBudget: null,
  };

  switch (request.taskType) {
    case 'translate':
      return {
        ...emptyFields,
        targetLanguage: request.targetLanguage,
        targetLanguageName: request.targetLanguageName ?? null,
      };
    case 'polish':
      return {
        ...emptyFields,
        context: request.context ?? null,
        keywords: request.keywords ?? null,
      };
    case 'summary':
      return {
        ...emptyFields,
        template: normalizeSummaryTemplate(request.template),
      };
  }
}

function normalizeTranscriptJobRequest(
  request: TranscriptLlmJobRequest,
): CoreTranscriptLlmJobRequest {
  return {
    taskId: request.taskId,
    taskType: request.taskType,
    jobHistoryId: request.jobHistoryId ?? null,
    config: normalizeConfig(request.config, 'request.config'),
    segments: request.segments.map((segment, index) => (
      normalizeTranscriptSegment(segment, `request.segments[${index}]`)
    )),
    ...normalizeTranscriptJobFields(request),
  };
}

export async function generateLlmText(request: LlmGenerateCommandRequest): Promise<string> {
  return invokeTauri(TauriCommand.llm.generateText, {
    request: normalizeGenerateRequest(request),
  });
}

export async function completeLlm(request: LlmCompletionRequest): Promise<LlmCompletionResponse> {
  const response = await invokeTauri(TauriCommand.llm.complete, {
    request: normalizeCompletionRequest(request),
  });
  return normalizeCompletionResponse(response);
}

export async function describeLlmModel(config: LlmConfig): Promise<LlmDiscoveredModelSummary | null> {
  const result = await invokeTauri(TauriCommand.llm.describeModel, {
    config: normalizeConfig(config, 'config'),
  });
  return result == null ? null : normalizeModelSummary(result, 'result');
}

export async function listLlmModels(request: {
  provider: string;
  strategy?: string;
  baseUrl: string;
  apiKey: string;
}): Promise<LlmDiscoveredModelSummary[]> {
  const result = await invokeTauri(TauriCommand.llm.listModels, {
    request: normalizeModelsRequest(request),
  });
  return result.map((model, index) => normalizeModelSummary(model, `result[${index}]`));
}

export async function polishTranscriptSegments(
  request: PolishSegmentsRequest,
): Promise<PolishedSegment[]> {
  return invokeTauri(TauriCommand.llm.polishTranscriptSegments, {
    request: normalizePolishRequest(request),
  });
}

export async function runTranscriptLlmJob(
  request: TranscriptLlmJobRequest,
): Promise<TranscriptLlmJobResult> {
  return invokeTauri(TauriCommand.llm.runTranscriptJob, {
    request: normalizeTranscriptJobRequest(request),
  });
}

export async function summarizeTranscript(
  request: SummarizeTranscriptRequest,
): Promise<TranscriptSummaryResult> {
  return invokeTauri(TauriCommand.llm.summarizeTranscript, {
    request: normalizeSummarizeRequest(request),
  });
}

export async function translateTranscriptSegments(
  request: TranslateSegmentsRequest,
): Promise<TranslatedSegment[]> {
  return invokeTauri(TauriCommand.llm.translateTranscriptSegments, {
    request: normalizeTranslateRequest(request),
  });
}
