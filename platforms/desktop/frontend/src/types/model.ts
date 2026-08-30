export interface ModelFileConfig {
  encoder?: string;
  decoder?: string;
  model?: string;
  joiner?: string;
  tokens?: string;
  convFrontend?: string;
  encoderAdaptor?: string;
  llm?: string;
  embedding?: string;
  tokenizer?: string;
  mmproj?: string;
  preprocessor?: string;
  uncachedDecoder?: string;
  cachedDecoder?: string;
  mergedDecoder?: string;
}
