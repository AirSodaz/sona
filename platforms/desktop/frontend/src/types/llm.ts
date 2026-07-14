import type {
  LlmProvider as GeneratedLlmProvider,
  BuiltinLlmProvider as GeneratedBuiltinLlmProvider,
  SummaryTemplateId as GeneratedSummaryTemplateId,
  PolishPresetId as GeneratedPolishPresetId
} from '../bindings';

export type BuiltInLlmProvider = GeneratedBuiltinLlmProvider;
export type CustomLlmProviderId = `custom-${string}`;

// Frontend strictly uses flat strings for UI state and config map keys
export type LlmProvider = BuiltInLlmProvider | CustomLlmProviderId;

// The payload shape expected by Tauri IPC
export type LlmProviderPayload = GeneratedLlmProvider;

export function flattenLlmProvider(provider: LlmProviderPayload | LlmProvider): LlmProvider {
  if (typeof provider === 'string') return provider;
  if ('Builtin' in provider && provider.Builtin) return provider.Builtin as LlmProvider;
  if ('Custom' in provider && provider.Custom) return provider.Custom as CustomLlmProviderId;
  return String(provider) as LlmProvider;
}

export function unflattenLlmProvider(provider: LlmProvider): LlmProviderPayload {
  if (provider.startsWith('custom-')) {
    return { Custom: provider };
  }
  return { Builtin: provider as BuiltInLlmProvider };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flattenAppConfig(config: any): any {
  if (!config) return config;
  const result = { ...config };

  if (result.llmSettings && typeof result.llmSettings === 'object') {
    result.llmSettings = { ...result.llmSettings };
    if (result.llmSettings.activeProvider && typeof result.llmSettings.activeProvider === 'object') {
      result.llmSettings.activeProvider = flattenLlmProvider(result.llmSettings.activeProvider);
    }
  }

  if (result.summaryTemplateId && typeof result.summaryTemplateId === 'object') {
    result.summaryTemplateId = result.summaryTemplateId.Builtin || result.summaryTemplateId.Custom;
  }
  if (result.polishPresetId && typeof result.polishPresetId === 'object') {
    result.polishPresetId = result.polishPresetId.Builtin || result.polishPresetId.Custom;
  }

  return result;
}


export type LlmProviderStrategy =
  | 'openai_compatible'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'azure_openai'
  | 'openai_responses'
  | 'openai_compatible_custom_path'
  | 'google_translate'
  | 'google_translate_free'
  | 'perplexity';

export type CustomLlmProviderStrategy =
  | 'openai_compatible'
  | 'openai_responses'
  | 'anthropic'
  | 'gemini';

export interface CustomLlmProvider {
  /** Stable custom provider id generated from the user-visible name. */
  id: CustomLlmProviderId;
  /** User-visible provider name. */
  name: string;
  /** API compatibility mode used by the native runtime. */
  strategy: CustomLlmProviderStrategy;
  /** ISO timestamp when this provider was added. */
  createdAt: string;
}

export type LlmFeature = 'polish' | 'translation' | 'summary';

export interface LlmProviderSetting {
  /** Provider API host or endpoint. */
  apiHost: string;
  /** Provider API key or token. */
  apiKey: string;
  /** Optional provider-specific API path override. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
}

export type LlmModelSource = 'manual' | 'discovered';

export type LlmModality = 'text' | 'image' | 'audio' | 'video' | 'pdf';

export type LlmModelMetadataSource = 'provider' | 'models_dev';

export interface LlmModelMetadata {
  /** Human-readable model name supplied by the provider or model catalog. */
  displayName?: string;
  /** Input token price, if the provider reports it. */
  inputPrice?: number;
  /** Output token price, if the provider reports it. */
  outputPrice?: number;
  /** Cached input token price, if the model catalog reports it. */
  cacheReadPrice?: number;
  /** Cache creation token price, if the model catalog reports it. */
  cacheWritePrice?: number;
  /** Maximum context window supported by the model. */
  contextWindow?: number;
  /** Maximum output token limit supported by the model. */
  maxOutputTokens?: number;
  /** Knowledge cutoff reported by the model catalog. */
  knowledgeCutoff?: string;
  /** Model release date reported by the model catalog. */
  releaseDate?: string;
  /** Last model update date reported by the model catalog. */
  lastUpdated?: string;
  /** Modalities accepted by the model. */
  inputModalities?: LlmModality[];
  /** Modalities emitted by the model. */
  outputModalities?: LlmModality[];
  /** Whether the model supports multimodal input/output. */
  supportsMultimodal?: boolean;
  /** Whether the model supports tool calling. */
  supportsTools?: boolean;
  /** Whether the model supports deeper reasoning modes. */
  supportsReasoning?: boolean;
  /** Whether the model supports a native structured-output schema. */
  supportsStructuredOutput?: boolean;
  /** Whether the provider supports managed prompt caching for this model. */
  supportsPromptCaching?: boolean;
  /** Sources that contributed metadata; provider values take precedence. */
  metadataSources?: LlmModelMetadataSource[];
}

export type LlmDiscoveredModelSummary = {
  /** Provider-reported model name. */
  model: string;
} & {
  [Key in keyof LlmModelMetadata]?: LlmModelMetadata[Key] | null;
};

export interface LlmModelEntry {
  /** Stable identifier for the configured model entry. */
  id: string;
  /** Provider used for this model entry. */
  provider: LlmProvider;
  /** Model name or deployment name. */
  model: string;
  /** Whether this model was added manually or discovered from the provider. */
  source?: LlmModelSource;
  /** Optional metadata reported by the provider for this model. */
  metadata?: LlmModelMetadata;
  /** Metadata fields manually edited by the user and protected from provider refreshes. */
  metadataOverrides?: Partial<Record<keyof LlmModelMetadata, true>>;
}

export interface LlmFeatureSelections {
  /** Selected model entry for polish. */
  polishModelId?: string;
  /** Selected model entry for translation. */
  translationModelId?: string;
  /** Selected model entry for summary. */
  summaryModelId?: string;
  /** Temperature override for polish. */
  polishTemperature?: number;
  /** Temperature override for translation. */
  translationTemperature?: number;
  /** Temperature override for summary. */
  summaryTemperature?: number;
  polishReasoningEnabled?: boolean;
  polishReasoningLevel?: 'low' | 'medium' | 'high';
  translationReasoningEnabled?: boolean;
  translationReasoningLevel?: 'low' | 'medium' | 'high';
  summaryReasoningEnabled?: boolean;
  summaryReasoningLevel?: 'low' | 'medium' | 'high';
}

export interface LlmModelDiscoveryStatus {
  /** ISO timestamp when provider models were last fetched successfully. */
  fetchedAt: string;
  /** ISO timestamp when the fetched provider model list should be refreshed. */
  expiresAt: string;
}

export interface LlmSettings {
  /** Currently active provider. */
  activeProvider: LlmProvider;
  /** User-added providers keyed by stable custom ids. */
  customProviders?: Record<CustomLlmProviderId, CustomLlmProvider>;
  /** Per-provider saved settings. */
  providers: Partial<Record<LlmProvider, LlmProviderSetting>>;
  /** Added models keyed by ID. */
  models: Record<string, LlmModelEntry>;
  /** Ordered list of added model IDs. */
  modelOrder: string[];
  /** Per-provider freshness metadata for discovered model lists. */
  modelDiscovery?: Partial<Record<LlmProvider, LlmModelDiscoveryStatus>>;
  /** Feature-specific model selections. */
  selections: LlmFeatureSelections;
}

export interface LlmConfig {
  /** LLM provider kind. */
  provider: LlmProvider;
  /** API strategy used by the native runtime. */
  strategy?: LlmProviderStrategy;
  /** LLM base URL. */
  baseUrl: string;
  /** LLM API key. */
  apiKey: string;
  /** LLM model name. */
  model: string;
  /** Optional provider-specific path. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
  /** LLM temperature (0.0 to 2.0). */
  temperature?: number;
  reasoningEnabled?: boolean;
  reasoningLevel?: 'low' | 'medium' | 'high';
  /** Global LLM Request timeout in seconds. */
  timeoutSeconds?: number;
}

export type LlmJsonValue =
  | null
  | boolean
  | number
  | string
  | LlmJsonValue[]
  | { [key: string]: LlmJsonValue };

export type LlmResponseFormat =
  | { type: 'text' }
  | { type: 'json_object' }
  | { type: 'json_schema'; name: string; schema: LlmJsonValue };

export type LlmResponseFormatKind = 'text' | 'json_object' | 'json_schema';

export type LlmPromptCachePolicy = 'disabled' | 'automatic';

export type LlmCapabilityPolicy = 'strict' | 'compatible';

export interface LlmCompletionOptions {
  temperature?: number;
  maxOutputTokens?: number;
  reasoningEnabled?: boolean;
  reasoningLevel?: string;
  responseFormat?: LlmResponseFormat;
  promptCache?: LlmPromptCachePolicy;
  capabilityPolicy?: LlmCapabilityPolicy;
}

export type LlmGenerateSource = 'title_generation' | 'connection_test' | 'generic';

export interface LlmCompletionRequest {
  config: LlmConfig;
  systemPrompt?: string;
  input: string;
  options?: LlmCompletionOptions;
  source?: LlmGenerateSource;
}

export interface LlmTokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  cachedInputTokens: number;
  cacheCreationInputTokens: number;
  reasoningTokens: number;
}

export interface LlmExecutionMetadata {
  requestedFormat: LlmResponseFormatKind;
  appliedFormat: LlmResponseFormatKind;
  warnings: string[];
  attempts: number;
}

export interface LlmCompletionResponse {
  text: string;
  json?: LlmJsonValue;
  usage: LlmTokenUsage | null;
  execution: LlmExecutionMetadata;
}

export type SummaryTemplateId = string;
export type SummaryTemplateIdPayload = GeneratedSummaryTemplateId;

export type PolishPresetId = string;
export type PolishPresetIdPayload = GeneratedPolishPresetId;

export interface SummaryCustomTemplate {
  /** Stable template id. */
  id: string;
  /** User-visible template name. */
  name: string;
  /** Template instructions sent to the summary prompt. */
  instructions: string;
}

export interface ResolvedSummaryTemplate {
  /** Stable template id. */
  id: SummaryTemplateId;
  /** Resolved display name. */
  name: string;
  /** Instructions sent to the prompt builder. */
  instructions: string;
  /** Whether this template is built into the app. */
  builtIn: boolean;
}

export type SummaryTemplate = SummaryTemplateId;

export const DEFAULT_SUMMARY_TEMPLATE_ID: SummaryTemplateId = 'general';
export const DEFAULT_SUMMARY_TEMPLATE: SummaryTemplate = DEFAULT_SUMMARY_TEMPLATE_ID;
