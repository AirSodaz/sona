use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use async_trait::async_trait;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use sona_core::llm::provider_protocol::{LlmModality, LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{LlmConfig, LlmModelsRequest};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmStreamDelta};
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortError, LlmPortErrorKind,
    LlmStreamingPort,
};

use crate::batch::{get_llama_backend, gpu_backend_available};

pub const DEFAULT_LOCAL_LLM_MODEL: &str = "Qwen/Qwen3.5-4B";
pub const DEFAULT_LOCAL_LLM_FILENAME: &str = "Qwen3.5-4B-Q4_K_M.gguf";
const N_BATCH: usize = 512;
const GPU_OFFLOAD_ALL_LAYERS: u32 = u32::MAX;

type ModelCacheKey = (PathBuf, u32);
static LLM_MODEL_CACHE: LazyLock<Mutex<HashMap<ModelCacheKey, Arc<LlamaModel>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn prune_idle_llm_models() {
    if let Ok(mut cache) = LLM_MODEL_CACHE.lock() {
        cache.retain(|_, model| Arc::strong_count(model) > 1);
    }
}

#[derive(Clone, Debug, Default)]
pub struct LlamaCppLlmEngine {
    models_dir: Option<PathBuf>,
}

impl LlamaCppLlmEngine {
    pub fn new() -> Self {
        Self { models_dir: None }
    }

    pub fn with_models_dir(models_dir: Option<PathBuf>) -> Self {
        Self { models_dir }
    }

    pub fn set_models_dir(&mut self, models_dir: Option<PathBuf>) {
        self.models_dir = models_dir;
    }

    pub fn models_dir(&self) -> Option<&Path> {
        self.models_dir.as_deref()
    }

    /// Resolves the candidate GGUF model path from configuration, models directory, and defaults.
    pub fn resolve_model_path(
        &self,
        model_name_or_path: &str,
        api_host: Option<&str>,
    ) -> Result<PathBuf, LlmPortError> {
        let trimmed_name = model_name_or_path.trim();
        let target_model = if trimmed_name.is_empty() {
            DEFAULT_LOCAL_LLM_MODEL
        } else {
            trimmed_name
        };

        // 1. Check if model name itself is an existing file
        let direct_path = Path::new(target_model);
        if direct_path.is_file() {
            return Ok(direct_path.to_path_buf());
        }

        // 2. Check if api_host is configured and is an existing file or directory
        let api_host_clean = api_host
            .map(str::trim)
            .filter(|h| !h.is_empty() && !h.starts_with("http://") && !h.starts_with("https://"));

        if let Some(host) = api_host_clean {
            let host_path = Path::new(host);
            if host_path.is_file() {
                return Ok(host_path.to_path_buf());
            }
            if host_path.is_dir()
                && let Some(found) = find_model_in_dir(host_path, target_model)
            {
                return Ok(found);
            }
        }

        // 3. Check models_dir
        if let Some(dir) = &self.models_dir
            && dir.is_dir()
            && let Some(found) = find_model_in_dir(dir, target_model)
        {
            return Ok(found);
        }

        // Model not found - build a descriptive error message with search paths
        let mut searched_locations = Vec::new();
        if let Some(host) = api_host_clean {
            searched_locations.push(format!("API Host path: '{host}'"));
        }
        if let Some(dir) = &self.models_dir {
            searched_locations.push(format!("Models directory: '{}'", dir.display()));
        }
        if searched_locations.is_empty() {
            searched_locations.push(format!("Working directory: '{}'", direct_path.display()));
        }

        let dir_display = self
            .models_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "models".to_string());
        let matched_preset = sona_core::llm::local_models::find_local_llm_model(target_model);
        let expected_filename = matched_preset
            .map(|p| p.filename.as_str())
            .unwrap_or(DEFAULT_LOCAL_LLM_FILENAME);
        let download_hint = matched_preset
            .and_then(|p| p.download.as_ref())
            .map(|d| format!("Model download: {}", d.url))
            .unwrap_or_else(|| {
                "Model download: https://huggingface.co/unsloth/Qwen3.5-4B-GGUF or https://hf-mirror.com/unsloth/Qwen3.5-4B-GGUF".to_string()
            });

        Err(LlmPortError::new(
            LlmPortErrorKind::Unavailable,
            format!(
                "Local model '{target_model}' not found.\n\
                 Searched in:\n  - {}\n\
                 Please place the model file '{expected_filename}' in '{dir_display}', \
                 or specify the full GGUF file path in the 'Model Path' field of Local provider settings.\n\
                 {download_hint}",
                searched_locations.join("\n  - ")
            ),
        ))
    }

    /// Lists local models found in the models directory and custom path.
    pub fn scan_local_models(&self, api_host: Option<&str>) -> Vec<LlmModelSummary> {
        let mut models = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        let api_host_clean = api_host
            .map(str::trim)
            .filter(|h| !h.is_empty() && !h.starts_with("http://") && !h.starts_with("https://"));

        // Helper to add a GGUF file
        let mut add_model_entry = |file_path: &Path| {
            if let Some(file_name) = file_path.file_name().and_then(|s| s.to_str()) {
                if sona_core::llm::local_models::is_non_llm_model_file(file_name) {
                    return;
                }
                if let Some(preset) = sona_core::llm::local_models::find_local_llm_model(file_name)
                {
                    if seen_names.insert(preset.model.clone()) {
                        models.push(preset.to_model_summary());
                    }
                    return;
                }
            }
            if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
                if sona_core::llm::local_models::is_non_llm_model_file(stem) {
                    return;
                }
                if let Some(preset) = sona_core::llm::local_models::find_local_llm_model(stem) {
                    if seen_names.insert(preset.model.clone()) {
                        models.push(preset.to_model_summary());
                    }
                    return;
                }
                let name = stem.to_string();
                if seen_names.insert(name.clone()) {
                    let mut summary = LlmModelSummary {
                        model: name.clone(),
                        context_window: Some(262_144),
                        max_output_tokens: Some(4096),
                        input_modalities: vec![LlmModality::Text],
                        output_modalities: vec![LlmModality::Text],
                        ..Default::default()
                    };
                    let caps = sona_core::llm::capabilities::LlmModelCapabilities::resolve(
                        sona_core::llm::tasks::LlmProviderStrategy::Local,
                        &name,
                        "",
                        Some(&summary),
                    );
                    summary.supports_reasoning = Some(caps.reasoning);
                    summary.reasoning_mode = Some(caps.reasoning_mode);
                    summary.supported_thinking_levels = caps.supported_thinking_levels;
                    summary.supports_temperature = Some(caps.supports_temperature);
                    summary.token_limit_key = Some(caps.token_limit_key.as_str().to_string());
                    models.push(summary);
                }
            }
        };

        // 1. Scan api_host if directory
        if let Some(host) = api_host_clean {
            let host_path = Path::new(host);
            if host_path.is_file() && is_gguf_file(host_path) {
                add_model_entry(host_path);
            } else if host_path.is_dir() {
                scan_dir_for_gguf(host_path, 2, &mut add_model_entry);
            }
        }

        // 2. Scan models_dir
        if let Some(dir) = &self.models_dir
            && dir.is_dir()
        {
            scan_dir_for_gguf(dir, 3, &mut add_model_entry);
        }

        // 3. For presets from sona_core::llm::local_models, only add if actually installed!
        for preset in sona_core::llm::local_models::local_llm_models() {
            let is_installed = if let Some(dir) = &self.models_dir
                && dir.is_dir()
            {
                preset.is_installed_in_dir(dir) || find_model_in_dir(dir, &preset.model).is_some()
            } else {
                false
            } || if let Some(host) = api_host_clean {
                let host_path = Path::new(host);
                if host_path.is_file() {
                    host_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| {
                            s.eq_ignore_ascii_case(&preset.id)
                                || s.eq_ignore_ascii_case(&preset.filename)
                        })
                } else if host_path.is_dir() {
                    preset.is_installed_in_dir(host_path)
                        || find_model_in_dir(host_path, &preset.model).is_some()
                } else {
                    false
                }
            } else {
                false
            };

            if is_installed && seen_names.insert(preset.model.clone()) {
                models.push(preset.to_model_summary());
            }
        }

        models
    }
}

fn is_gguf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
}

fn scan_dir_for_gguf<F>(dir: &Path, max_depth: usize, callback: &mut F)
where
    F: FnMut(&Path),
{
    if max_depth == 0 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_gguf_file(&path) {
            callback(&path);
        } else if path.is_dir() {
            scan_dir_for_gguf(&path, max_depth - 1, callback);
        }
    }
}

fn find_model_in_dir(dir: &Path, target: &str) -> Option<PathBuf> {
    // 1. Check if target or its bare name matches any preset from local_models registry
    let bare_name = target.split('/').next_back().unwrap_or(target);
    let matched_preset = sona_core::llm::local_models::find_local_llm_model(target)
        .or_else(|| sona_core::llm::local_models::find_local_llm_model(bare_name));

    if let Some(preset) = matched_preset {
        let preset_candidates = [
            dir.join(&preset.filename),
            dir.join(&preset.id).join(&preset.filename),
            dir.join(format!("{}.gguf", preset.id)),
            dir.join(format!("{}-gguf", preset.id))
                .join(&preset.filename),
        ];
        for c in preset_candidates {
            if c.is_file() {
                return Some(c);
            }
        }
    }

    // 2. Exact file or with .gguf extension
    let candidate = dir.join(target);
    if candidate.is_file() {
        return Some(candidate);
    }
    let with_ext = dir.join(format!("{target}.gguf"));
    if with_ext.is_file() {
        return Some(with_ext);
    }

    let candidate_bare = dir.join(bare_name);
    if candidate_bare.is_file() {
        return Some(candidate_bare);
    }
    let candidate_bare_ext = dir.join(format!("{bare_name}.gguf"));
    if candidate_bare_ext.is_file() {
        return Some(candidate_bare_ext);
    }

    // 3. Check known standard files for Qwen3.5-4B fallback
    let is_qwen3_5_4b = target.eq_ignore_ascii_case("Qwen/Qwen3.5-4B")
        || target.eq_ignore_ascii_case("qwen3.5-4b")
        || target.eq_ignore_ascii_case("Qwen3.5-4B")
        || target.to_ascii_lowercase().contains("qwen3.5-4b");

    if is_qwen3_5_4b {
        let standard_candidates = [
            dir.join(DEFAULT_LOCAL_LLM_FILENAME),
            dir.join("Qwen3.5-4B.gguf"),
            dir.join("qwen3.5-4b").join(DEFAULT_LOCAL_LLM_FILENAME),
            dir.join("qwen3.5-4b").join("Qwen3.5-4B.gguf"),
            dir.join("qwen3.5-4b-gguf").join(DEFAULT_LOCAL_LLM_FILENAME),
            dir.join("qwen3.5-4b-gguf").join("Qwen3.5-4B.gguf"),
        ];
        for c in standard_candidates {
            if c.is_file() {
                return Some(c);
            }
        }
    }

    let is_qwen3_1_7b = target.eq_ignore_ascii_case("Qwen/Qwen3-1.7B-Instruct")
        || target.eq_ignore_ascii_case("Qwen/Qwen3-1.7B")
        || target.eq_ignore_ascii_case("qwen3-1.7b-instruct")
        || target.eq_ignore_ascii_case("qwen3-1.7b")
        || target.eq_ignore_ascii_case("Qwen3-1.7B-Instruct")
        || target.eq_ignore_ascii_case("Qwen3-1.7B")
        || target.to_ascii_lowercase().contains("qwen3-1.7b");

    if is_qwen3_1_7b {
        let candidates = [
            dir.join("Qwen3-1.7B-Q4_K_M.gguf"),
            dir.join("Qwen3-1.7B-Instruct-Q4_K_M.gguf"),
            dir.join("Qwen3-1.7B-Instruct.gguf"),
            dir.join("Qwen3-1.7B.gguf"),
            dir.join("qwen3-1.7b").join("Qwen3-1.7B-Q4_K_M.gguf"),
            dir.join("qwen3-1.7b")
                .join("Qwen3-1.7B-Instruct-Q4_K_M.gguf"),
            dir.join("qwen3-1.7b").join("Qwen3-1.7B-Instruct.gguf"),
            dir.join("qwen3-1.7b").join("Qwen3-1.7B.gguf"),
            dir.join("qwen3-1.7b-gguf").join("Qwen3-1.7B-Q4_K_M.gguf"),
            dir.join("qwen3-1.7b-gguf")
                .join("Qwen3-1.7B-Instruct-Q4_K_M.gguf"),
            dir.join("qwen3-1.7b-gguf").join("Qwen3-1.7B.gguf"),
        ];
        for c in candidates {
            if c.is_file() {
                return Some(c);
            }
        }
    }

    // 4. Subdirectory matching bare name or preset id
    let mut subdirs = vec![dir.join(bare_name.to_lowercase())];
    if let Some(preset) = matched_preset {
        subdirs.push(dir.join(preset.id.to_lowercase()));
    }
    for subdir in subdirs {
        if subdir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&subdir)
        {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && is_gguf_file(&p) {
                    return Some(p);
                }
            }
        }
    }

    // 5. Search top-level files matching target, bare name, or preset metadata
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && is_gguf_file(&p) {
                if let Some(file_name) = p.file_name().and_then(|s| s.to_str())
                    && let Some(preset) = matched_preset
                    && file_name.eq_ignore_ascii_case(&preset.filename)
                {
                    return Some(p);
                }
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if let Some(preset) = matched_preset
                        && (stem.eq_ignore_ascii_case(&preset.id)
                            || stem.to_ascii_lowercase().contains(&preset.id))
                    {
                        return Some(p);
                    }
                    if stem.eq_ignore_ascii_case(bare_name)
                        || (is_qwen3_5_4b && stem.to_ascii_lowercase().contains("qwen3.5-4b"))
                        || (is_qwen3_1_7b && stem.to_ascii_lowercase().contains("qwen3-1.7b"))
                    {
                        return Some(p);
                    }
                }
            }
        }
    }

    None
}

fn load_cached_model(
    backend: &'static LlamaBackend,
    model_path: &Path,
    n_gpu_layers: u32,
) -> Result<Arc<LlamaModel>, LlmPortError> {
    let canonical = model_path.canonicalize().map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::Unavailable,
            format!(
                "Failed to canonicalize local model path '{}': {error}",
                model_path.display()
            ),
        )
    })?;

    let cache_key = (canonical, n_gpu_layers);
    let mut cache = LLM_MODEL_CACHE.lock().map_err(|_| {
        LlmPortError::new(
            LlmPortErrorKind::Unavailable,
            "llama.cpp local model cache lock poisoned",
        )
    })?;

    if let Some(model) = cache.get(&cache_key) {
        return Ok(Arc::clone(model));
    }

    // Evict unused models
    cache.retain(|k, model| k == &cache_key || Arc::strong_count(model) > 1);

    let params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
    let model = Arc::new(
        LlamaModel::load_from_file(backend, &cache_key.0, &params).map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!(
                    "Failed to load llama.cpp model from '{}': {error}",
                    cache_key.0.display()
                ),
            )
        })?,
    );

    cache.insert(cache_key, Arc::clone(&model));
    Ok(model)
}

pub(crate) fn prepare_chat_messages(
    system_prompt: Option<&str>,
    input: &str,
    model_name: &str,
    reasoning_enabled: bool,
    thinking_level: &sona_core::llm::runtime::ThinkingLevel,
) -> (Option<String>, String) {
    use sona_core::llm::runtime::ThinkingLevel;

    let lower_name = model_name.to_lowercase();
    let is_qwen = lower_name.contains("qwen");

    if !reasoning_enabled {
        let no_think_instruction = "Answer directly and concisely. Do not output any thinking process, internal monologue, or <think> tags.";
        let sys = match system_prompt {
            Some(s) if !s.trim().is_empty() => {
                format!("{s}\n\n[Instruction: {no_think_instruction}]")
            }
            _ => format!("[Instruction: {no_think_instruction}]"),
        };
        let user = if is_qwen {
            format!("/no_think\n{input}")
        } else {
            input.to_string()
        };
        (Some(sys), user)
    } else {
        let dynamic_instruction = match thinking_level {
            ThinkingLevel::Minimal => {
                "Thinking effort: minimal. Keep your internal thought process extremely brief and concise before answering."
                    .to_string()
            }
            ThinkingLevel::Low => {
                "Thinking effort: low. Keep your internal thought process concise and focus on key steps."
                    .to_string()
            }
            ThinkingLevel::Medium | ThinkingLevel::Auto => {
                "Thinking effort: medium. Think step by step before answering."
                    .to_string()
            }
            ThinkingLevel::High => {
                "Thinking effort: high. Think thoroughly, exploring key considerations and details carefully before answering."
                    .to_string()
            }
            ThinkingLevel::Xhigh | ThinkingLevel::Max => {
                "Thinking effort: maximum. Think deeply, rigorously, and exhaustively, evaluating all possibilities and verifying reasoning before answering."
                    .to_string()
            }
            ThinkingLevel::Budget(budget) => {
                format!("Thinking budget: approximately {budget} tokens. Plan and bound your internal thinking process within this token budget before answering.")
            }
            ThinkingLevel::None => String::new(),
        };

        let sys = if !dynamic_instruction.is_empty() {
            match system_prompt {
                Some(s) if !s.trim().is_empty() => {
                    format!("{s}\n\n[Reasoning Guidance: {dynamic_instruction}]")
                }
                _ => format!("[Reasoning Guidance: {dynamic_instruction}]"),
            }
        } else {
            system_prompt.unwrap_or_default().to_string()
        };

        let user = if is_qwen {
            format!("/think\n{input}")
        } else {
            input.to_string()
        };
        (
            if sys.trim().is_empty() {
                None
            } else {
                Some(sys)
            },
            user,
        )
    }
}

pub(crate) fn format_chatml_prompt(system_prompt: Option<&str>, user_input: &str) -> String {
    let mut prompt = String::new();
    if let Some(sys) = system_prompt.filter(|s| !s.trim().is_empty()) {
        prompt.push_str("<|im_start|>system\n");
        prompt.push_str(sys);
        if !sys.ends_with('\n') {
            prompt.push('\n');
        }
        prompt.push_str("<|im_end|>\n");
    }
    prompt.push_str("<|im_start|>user\n");
    prompt.push_str(user_input);
    if !user_input.ends_with('\n') {
        prompt.push('\n');
    }
    prompt.push_str("<|im_end|>\n<|im_start|>assistant\n");
    prompt
}

fn format_prompt(
    model: &LlamaModel,
    system_prompt: Option<&str>,
    input: &str,
    model_name: &str,
    reasoning_enabled: bool,
    thinking_level: &sona_core::llm::runtime::ThinkingLevel,
) -> Result<String, LlmPortError> {
    let (effective_sys, effective_input) = prepare_chat_messages(
        system_prompt,
        input,
        model_name,
        reasoning_enabled,
        thinking_level,
    );

    if let Ok(template) = model.chat_template(None) {
        let mut chat_messages = Vec::new();
        if let Some(sys) = &effective_sys
            && let Ok(msg) = LlamaChatMessage::new("system".to_string(), sys.to_string())
        {
            chat_messages.push(msg);
        }
        if let Ok(msg) = LlamaChatMessage::new("user".to_string(), effective_input.clone()) {
            chat_messages.push(msg);
        }

        if !chat_messages.is_empty()
            && let Ok(prompt) = model.apply_chat_template(&template, &chat_messages, true)
        {
            return Ok(prompt);
        }
    }

    // Fallback: ChatML format
    Ok(format_chatml_prompt(
        effective_sys.as_deref(),
        &effective_input,
    ))
}

struct GenerationContext {
    model: Arc<LlamaModel>,
    prompt_tokens: Vec<llama_cpp_2::token::LlamaToken>,
    max_output_tokens: usize,
    temperature: f32,
    reasoning_enabled: bool,
    delta_sender: Option<tokio::sync::mpsc::UnboundedSender<String>>,
}

fn run_llama_generation(
    backend: &'static LlamaBackend,
    gen_ctx: GenerationContext,
) -> Result<StandardLlmResponse, LlmPortError> {
    let model = gen_ctx.model;
    let prompt_tokens = gen_ctx.prompt_tokens;
    let max_output_tokens = gen_ctx.max_output_tokens;
    let temperature = gen_ctx.temperature;
    let delta_sender = gen_ctx.delta_sender;

    let n_ctx_train = model.n_ctx_train();
    let max_ctx = if n_ctx_train > 0 {
        n_ctx_train.clamp(2048, 32_768)
    } else {
        32_768
    };
    let prompt_len = prompt_tokens.len() as u32;
    let needed = prompt_len.saturating_add(max_output_tokens as u32);
    let min_ctx = prompt_len.saturating_add(1).max(2048).min(max_ctx);
    let resolved_ctx = needed.clamp(min_ctx, max_ctx);
    let context_size = NonZeroU32::new(resolved_ctx).unwrap_or(NonZeroU32::new(4096).unwrap());

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8) as i32;

    let context_params = LlamaContextParams::default()
        .with_n_ctx(Some(context_size))
        .with_n_batch(N_BATCH as u32)
        .with_n_threads(num_threads)
        .with_n_threads_batch(num_threads);

    let mut context = model
        .new_context(backend, context_params)
        .map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!("Failed to create llama.cpp context: {error}"),
            )
        })?;

    // Ingest prompt tokens in batches
    let mut batch = LlamaBatch::new(N_BATCH, 1);
    let mut current_pos = 0;

    for chunk in prompt_tokens.chunks(N_BATCH) {
        batch.clear();
        for (offset, &token) in chunk.iter().enumerate() {
            let pos = (current_pos + offset) as i32;
            let is_last = (current_pos + offset) == prompt_tokens.len() - 1;
            batch.add(token, pos, &[0], is_last).map_err(|error| {
                LlmPortError::new(
                    LlmPortErrorKind::Unavailable,
                    format!("Failed to add token to batch: {error}"),
                )
            })?;
        }
        context.decode(&mut batch).map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!("llama.cpp prompt evaluation failed: {error}"),
            )
        })?;
        current_pos += chunk.len();
    }

    // Sampler setup
    let mut sampler = if temperature <= 0.01 {
        LlamaSampler::greedy()
    } else {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(12345);
        LlamaSampler::chain_simple([LlamaSampler::temp(temperature), LlamaSampler::dist(seed)])
    };

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut generated_text = String::new();
    let mut generated_tokens = 0usize;

    let available = context.n_ctx().saturating_sub(current_pos as u32) as usize;
    let generation_limit = max_output_tokens.min(available);

    for _ in 0..generation_limit {
        let token = sampler.sample(&context, -1);
        if model.is_eog_token(token) {
            break;
        }
        sampler.accept(token);

        let piece = model
            .token_to_piece(token, &mut decoder, false, None)
            .map_err(|error| {
                LlmPortError::new(
                    LlmPortErrorKind::Protocol,
                    format!("Failed to decode output token: {error}"),
                )
            })?;

        generated_text.push_str(&piece);
        if let Some(tx) = &delta_sender
            && tx.send(piece).is_err()
        {
            // Stream receiver has disconnected or cancelled
            break;
        }
        generated_tokens += 1;

        batch.clear();
        batch
            .add(token, current_pos as i32, &[0], true)
            .map_err(|error| {
                LlmPortError::new(
                    LlmPortErrorKind::Unavailable,
                    format!("Failed to add generated token to batch: {error}"),
                )
            })?;
        context.decode(&mut batch).map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!("llama.cpp token generation step failed: {error}"),
            )
        })?;
        current_pos += 1;
    }

    let (text, thought) =
        sona_core::llm::provider_protocol::strip_and_extract_inline_thoughts(&generated_text);

    let reasoning_tokens = if let Some(th) = &thought {
        model
            .str_to_token(th, AddBos::Never)
            .map(|t| t.len() as u64)
            .unwrap_or(0)
    } else {
        0
    };

    Ok(StandardLlmResponse {
        text,
        thought: if gen_ctx.reasoning_enabled {
            thought
        } else {
            None
        },
        usage: Some(TokenUsage {
            prompt_tokens: prompt_tokens.len() as u64,
            completion_tokens: generated_tokens as u64,
            total_tokens: (prompt_tokens.len() + generated_tokens) as u64,
            reasoning_tokens,
            ..Default::default()
        }),
    })
}

impl LlamaCppLlmEngine {
    async fn execute_completion_internal(
        &self,
        request: LlmCompletionRequest,
        delta_sender: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let backend = get_llama_backend().map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!("Failed to initialize llama.cpp backend: {error}"),
            )
        })?;

        let model_path = self.resolve_model_path(
            &request.config.model,
            Some(request.config.base_url.as_str()),
        )?;

        let n_gpu_layers = if gpu_backend_available() {
            GPU_OFFLOAD_ALL_LAYERS
        } else {
            0
        };

        let model = load_cached_model(backend, &model_path, n_gpu_layers)?;
        let reasoning_enabled = request.effective_reasoning_enabled();
        let thinking_level = request.effective_thinking_level();
        let prompt = format_prompt(
            &model,
            request.system_prompt.as_deref(),
            &request.input,
            &request.config.model,
            reasoning_enabled,
            &thinking_level,
        )?;

        let prompt_tokens = model
            .str_to_token(&prompt, AddBos::Never)
            .map_err(|error| {
                LlmPortError::new(
                    LlmPortErrorKind::Protocol,
                    format!("Failed to tokenize prompt: {error}"),
                )
            })?;

        if prompt_tokens.is_empty() {
            return Err(LlmPortError::new(
                LlmPortErrorKind::InvalidRequest,
                "Prompt resulted in empty token sequence",
            ));
        }
        let n_ctx_train = model.n_ctx_train() as usize;
        let max_supported_tokens = if n_ctx_train > 0 {
            n_ctx_train.min(32_768)
        } else {
            32_768
        };
        if prompt_tokens.len() >= max_supported_tokens {
            return Err(LlmPortError::new(
                LlmPortErrorKind::InvalidRequest,
                format!(
                    "Prompt token length ({}) exceeds maximum local context size ({})",
                    prompt_tokens.len(),
                    max_supported_tokens
                ),
            ));
        }

        let mut max_output_tokens = request.options.max_output_tokens.unwrap_or(4096) as usize;
        if reasoning_enabled {
            use sona_core::llm::runtime::ThinkingLevel;
            if let ThinkingLevel::Budget(budget) = thinking_level {
                max_output_tokens = max_output_tokens.max(budget as usize + 2048);
            } else if matches!(
                thinking_level,
                ThinkingLevel::High | ThinkingLevel::Xhigh | ThinkingLevel::Max
            ) {
                max_output_tokens = max_output_tokens.max(8192);
            }
        }
        let temperature = request.effective_temperature().unwrap_or(0.7);

        let gen_ctx = GenerationContext {
            model,
            prompt_tokens,
            max_output_tokens,
            temperature,
            reasoning_enabled,
            delta_sender,
        };

        tokio::task::spawn_blocking(move || run_llama_generation(backend, gen_ctx))
            .await
            .map_err(|error| {
                LlmPortError::new(
                    LlmPortErrorKind::Unavailable,
                    format!("Local LLM generation task failed: {error}"),
                )
            })?
    }
}

#[async_trait]
impl LlmCompletionPort for LlamaCppLlmEngine {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        self.execute_completion_internal(request, None).await
    }
}

#[async_trait]
impl LlmStreamingPort for LlamaCppLlmEngine {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let engine = self.clone();
        let completion_handle =
            tokio::spawn(
                async move { engine.execute_completion_internal(request, Some(tx)).await },
            );
        let mut dual = sona_core::llm::streaming_protocol::DualStreamAccumulator::new(emit_delta);
        let mut demuxer = sona_core::llm::demuxer::ThoughtStreamDemuxer::new();

        while let Some(delta) = rx.recv().await {
            for chunk in demuxer.process(&delta) {
                match chunk.kind {
                    sona_core::llm::runtime::LlmStreamDeltaKind::Thought => {
                        dual.push_thought(&chunk.text)?;
                    }
                    sona_core::llm::runtime::LlmStreamDeltaKind::Content => {
                        dual.push_content(&chunk.text)?;
                    }
                }
            }
        }

        for chunk in demuxer.flush() {
            match chunk.kind {
                sona_core::llm::runtime::LlmStreamDeltaKind::Thought => {
                    dual.push_thought(&chunk.text)?;
                }
                sona_core::llm::runtime::LlmStreamDeltaKind::Content => {
                    dual.push_content(&chunk.text)?;
                }
            }
        }

        let mut response = completion_handle.await.map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!("Streaming background task join error: {error}"),
            )
        })??;

        if !dual.content_text().is_empty() {
            response.text = dual.content_text().to_string();
        }
        if !dual.thought_text().is_empty() {
            response.thought = Some(dual.thought_text().to_string());
        }

        Ok(response)
    }
}

#[async_trait]
impl LlmModelMetadataPort for LlamaCppLlmEngine {
    async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        let model_name = config.model.trim();
        let target = if model_name.is_empty() {
            DEFAULT_LOCAL_LLM_MODEL
        } else {
            model_name
        };
        let mut summary =
            if let Some(preset) = sona_core::llm::local_models::find_local_llm_model(target) {
                preset.to_model_summary()
            } else {
                LlmModelSummary {
                    model: target.to_string(),
                    context_window: Some(131_072),
                    max_output_tokens: Some(4096),
                    input_modalities: vec![LlmModality::Text],
                    output_modalities: vec![LlmModality::Text],
                    ..Default::default()
                }
            };

        let caps = sona_core::llm::capabilities::LlmModelCapabilities::resolve(
            config.strategy,
            &config.model,
            &config.base_url,
            Some(&summary),
        );
        summary.supports_reasoning = Some(caps.reasoning);
        summary.reasoning_mode = Some(caps.reasoning_mode);
        summary.supported_thinking_levels = caps.supported_thinking_levels;
        summary.supports_temperature = Some(caps.supports_temperature);
        summary.token_limit_key = Some(caps.token_limit_key.as_str().to_string());

        Ok(Some(summary))
    }
}
#[async_trait]
impl LlmModelDiscoveryPort for LlamaCppLlmEngine {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
        let api_host = if request.base_url.is_empty() {
            None
        } else {
            Some(request.base_url.as_str())
        };
        Ok(self.scan_local_models(api_host))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_model_fallback_error_message() {
        let engine = LlamaCppLlmEngine::new();
        let err = engine
            .resolve_model_path("non_existent_model.gguf", None)
            .unwrap_err();
        assert!(err.message.contains("Local model"));
        assert!(err.message.contains(DEFAULT_LOCAL_LLM_FILENAME));
    }

    #[test]
    fn scans_local_models_empty_when_not_downloaded() {
        let engine = LlamaCppLlmEngine::new();
        let list = engine.scan_local_models(None);
        assert!(
            list.is_empty(),
            "Un-downloaded models must not appear in scan_local_models"
        );
    }

    #[test]
    fn scans_local_models_includes_installed_preset_and_excludes_asr() {
        let temp_dir =
            std::env::temp_dir().join(format!("models_scan_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Installed LLM preset file
        let qwen_file = temp_dir.join(DEFAULT_LOCAL_LLM_FILENAME);
        std::fs::write(&qwen_file, b"dummy").unwrap();

        // ASR GGUF file and mmproj file
        let asr_file = temp_dir.join("Qwen3-ASR-0.6B-Q8_0.gguf");
        std::fs::write(&asr_file, b"dummy-asr").unwrap();
        let mmproj_file = temp_dir.join("mmproj-Qwen3-ASR-0.6B-Q8_0.gguf");
        std::fs::write(&mmproj_file, b"dummy-mmproj").unwrap();

        let engine = LlamaCppLlmEngine::with_models_dir(Some(temp_dir.clone()));
        let list = engine.scan_local_models(None);

        // Should include installed Qwen LLM
        assert!(list.iter().any(|m| m.model == DEFAULT_LOCAL_LLM_MODEL));
        // Should NOT include uninstalled gemma
        assert!(!list.iter().any(|m| m.model == "google/gemma-4-e2b"));
        // Should NOT include ASR or mmproj models
        assert!(
            !list
                .iter()
                .any(|m| m.model.contains("ASR") || m.model.contains("asr"))
        );
        assert!(!list.iter().any(|m| m.model.contains("mmproj")));

        let _ = std::fs::remove_dir_all(temp_dir);
    }
    fn test_config(model: &str) -> LlmConfig {
        LlmConfig {
            provider: sona_core::domain::LlmProvider::Builtin(
                sona_core::domain::BuiltinLlmProvider::Local,
            ),
            strategy: sona_core::llm::tasks::LlmProviderStrategy::Local,
            base_url: String::new(),
            api_key: String::new(),
            model: model.to_string(),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
            timeout_seconds: None,
        }
    }

    #[tokio::test]
    async fn describe_model_returns_correct_metadata_for_presets() {
        let engine = LlamaCppLlmEngine::new();
        let qwen_summary = engine
            .describe_model(&test_config("Qwen/Qwen3.5-4B"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(qwen_summary.context_window, Some(262_144));
        assert_eq!(qwen_summary.supports_reasoning, Some(true));
        assert!(matches!(
            qwen_summary.reasoning_mode,
            Some(sona_core::llm::runtime::ReasoningMode::Effort { .. })
        ));
        assert_eq!(qwen_summary.supported_thinking_levels.len(), 6);

        let gemma_summary = engine
            .describe_model(&test_config("google/gemma-4-e2b"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gemma_summary.context_window, Some(131_072));
        assert_eq!(gemma_summary.supports_reasoning, Some(true));

        let qwen3_summary = engine
            .describe_model(&test_config("Qwen/Qwen3-1.7B-Instruct"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(qwen3_summary.context_window, Some(32_768));
        assert_eq!(qwen3_summary.supports_reasoning, Some(true));
    }
    #[test]
    fn resolves_gemma_4_e2b_preset_in_models_dir() {
        let temp_dir = std::env::temp_dir().join(format!("models_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let gemma_file = temp_dir.join("gemma-4-E2B-it-Q4_K_M.gguf");
        std::fs::write(&gemma_file, b"dummy").unwrap();

        let engine = LlamaCppLlmEngine::with_models_dir(Some(temp_dir.clone()));

        // Resolving by canonical model ID
        let resolved_by_model = engine
            .resolve_model_path("google/gemma-4-e2b", None)
            .expect("should resolve gemma by model name");
        assert_eq!(resolved_by_model, gemma_file);

        // Resolving by preset id
        let resolved_by_id = engine
            .resolve_model_path("gemma-4-e2b", None)
            .expect("should resolve gemma by preset id");
        assert_eq!(resolved_by_id, gemma_file);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn resolves_qwen3_1_7b_preset_in_models_dir() {
        let temp_dir = std::env::temp_dir().join(format!("models_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let qwen_file = temp_dir.join("Qwen3-1.7B-Q4_K_M.gguf");
        std::fs::write(&qwen_file, b"dummy").unwrap();

        let engine = LlamaCppLlmEngine::with_models_dir(Some(temp_dir.clone()));

        // Resolving by canonical model ID
        let resolved_by_model = engine
            .resolve_model_path("Qwen/Qwen3-1.7B-Instruct", None)
            .expect("should resolve qwen3-1.7b by model name");
        assert_eq!(resolved_by_model, qwen_file);

        // Resolving by preset id
        let resolved_by_id = engine
            .resolve_model_path("qwen3-1.7b", None)
            .expect("should resolve qwen3-1.7b by preset id");
        assert_eq!(resolved_by_id, qwen_file);

        let _ = std::fs::remove_dir_all(temp_dir);
    }
    #[test]
    fn resolves_existing_file_directly() {
        let temp_path =
            std::env::temp_dir().join(format!("test_model_{}.gguf", uuid::Uuid::new_v4()));
        std::fs::write(&temp_path, b"test").unwrap();
        let engine = LlamaCppLlmEngine::new();
        let resolved = engine
            .resolve_model_path(temp_path.to_str().unwrap(), None)
            .unwrap();
        assert_eq!(resolved, temp_path);
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn prepares_chat_messages_for_qwen_with_reasoning_disabled() {
        use sona_core::llm::runtime::ThinkingLevel;

        let (sys, user) = prepare_chat_messages(
            Some("You are a helpful assistant."),
            "Explain quantum computing",
            "Qwen/Qwen3.5-4B",
            false,
            &ThinkingLevel::None,
        );
        assert!(user.starts_with("/no_think\n"));
        assert!(user.contains("Explain quantum computing"));
        let sys_str = sys.unwrap();
        assert!(sys_str.contains("You are a helpful assistant."));
        assert!(sys_str.contains("Answer directly and concisely"));
    }

    #[test]
    fn prepares_chat_messages_for_qwen_with_reasoning_enabled_and_high_effort() {
        use sona_core::llm::runtime::ThinkingLevel;

        let (sys, user) = prepare_chat_messages(
            None,
            "Solve this math problem",
            "Qwen/Qwen3.5-4B",
            true,
            &ThinkingLevel::High,
        );
        assert!(user.starts_with("/think\n"));
        assert!(user.contains("Solve this math problem"));
        let sys_str = sys.unwrap();
        assert!(sys_str.contains("Thinking effort: high"));
    }

    #[test]
    fn prepares_chat_messages_with_budget() {
        use sona_core::llm::runtime::ThinkingLevel;

        let (sys, _user) = prepare_chat_messages(
            None,
            "Write an essay",
            "google/gemma-4-e2b",
            true,
            &ThinkingLevel::Budget(2048),
        );
        let sys_str = sys.unwrap();
        assert!(sys_str.contains("2048 tokens"));
    }

    #[test]
    fn format_chatml_prompt_constructs_valid_chatml() {
        let prompt = format_chatml_prompt(Some("You are a helper."), "Hello world");
        assert!(prompt.contains("<|im_start|>system\nYou are a helper.\n<|im_end|>"));
        assert!(prompt.contains("<|im_start|>user\nHello world\n<|im_end|>"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }
}
