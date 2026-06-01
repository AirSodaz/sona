export type BuiltInLlmProvider =
  | 'open_ai'
  | 'open_ai_responses'
  | 'azure_openai'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'deep_seek'
  | 'moonshot_ai'
  | 'moonshot_cn'
  | 'xiaomi'
  | 'kimi'
  | 'silicon_flow'
  | 'qwen'
  | 'qwen_portal'
  | 'minimax_global'
  | 'minimax_cn'
  | 'openrouter'
  | 'lm_studio'
  | 'groq'
  | 'x_ai'
  | 'mistral_ai'
  | 'perplexity'
  | 'volcengine'
  | 'chatglm'
  | 'google_translate'
  | 'google_translate_free';

export type CustomLlmProviderId = `custom-${string}`;
export type LlmProvider = BuiltInLlmProvider | CustomLlmProviderId;

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

export interface LlmModelMetadata {
  /** Input token price, if the provider reports it. */
  inputPrice?: number;
  /** Output token price, if the provider reports it. */
  outputPrice?: number;
  /** Maximum context window supported by the model. */
  contextWindow?: number;
  /** Maximum output token limit supported by the model. */
  maxOutputTokens?: number;
  /** Whether the model supports multimodal input/output. */
  supportsMultimodal?: boolean;
  /** Whether the model supports tool calling. */
  supportsTools?: boolean;
  /** Whether the model supports deeper reasoning modes. */
  supportsReasoning?: boolean;
}

export interface LlmDiscoveredModelSummary extends LlmModelMetadata {
  /** Provider-reported model name. */
  model: string;
}

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

export type SummaryTemplateId = string;

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
