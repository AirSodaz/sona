fn normalize_incremental_json_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed == "```" || trimmed == "```json" {
        return None;
    }

    let trimmed = trimmed.trim_end_matches(',').trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }

    None
}

fn parse_json_array_or_ndjson<T: DeserializeOwned>(
    response_text: &str,
    task_type: LlmTaskType,
    chunk_number: usize,
) -> Result<Vec<T>, String> {
    let cleaned = clean_json_response(response_text);
    if cleaned.starts_with('[') {
        return serde_json::from_str::<Vec<T>>(&cleaned).map_err(|error| {
            chunk_error(
                task_type,
                chunk_number,
                format!("invalid JSON response: {error}"),
            )
        });
    }

    let mut items = Vec::new();
    for line in cleaned.lines() {
        if let Some(normalized) = normalize_incremental_json_line(line) {
            let parsed = serde_json::from_str::<T>(&normalized).map_err(|error| {
                chunk_error(
                    task_type,
                    chunk_number,
                    format!("invalid JSON response: {error}"),
                )
            })?;
            items.push(parsed);
        }
    }

    if items.is_empty() {
        return Err(chunk_error(
            task_type,
            chunk_number,
            "invalid JSON response: expected NDJSON lines or a JSON array",
        ));
    }

    Ok(items)
}

async fn stream_rig_completion_model<M, EmitFn>(
    model: M,
    input: &str,
    temperature: f32,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    M: CompletionModel,
    M::StreamingResponse: Clone + Unpin + GetTokenUsage + 'static,
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let mut stream = model
        .completion_request(input)
        .temperature_opt(Some(temperature as f64))
        .stream()
        .await
        .map_err(|error| error.to_string())?;

    while let Some(item) = stream.next().await {
        match item.map_err(|error| error.to_string())? {
            StreamedAssistantContent::Text(text) => accumulator.push(&text.text)?,
            StreamedAssistantContent::Final(_) => {}
            _ => {}
        }
    }

    let text = if accumulator.text.is_empty() {
        extract_text_response(&stream.choice)?
    } else {
        accumulator.text()
    };

    Ok(StandardLlmResponse {
        text,
        usage: stream
            .response
            .as_ref()
            .and_then(|response| token_usage_from_rig_usage(response.token_usage())),
    })
}

fn build_openai_stream_url(config: &LlmConfig) -> String {
    match config.provider {
        LlmProvider::AzureOpenAi => {
            let version = config.api_version.as_deref().unwrap_or("2024-10-21");
            format!(
                "{}/openai/deployments/{}/chat/completions?api-version={}",
                config.base_url.trim_end_matches('/'),
                config.model.trim(),
                version
            )
        }
        LlmProvider::Perplexity => join_url(
            &config.base_url,
            config.api_path.as_deref().unwrap_or("/chat/completions"),
        ),
        _ => join_url(
            &config.base_url,
            config.api_path.as_deref().unwrap_or("/v1/chat/completions"),
        ),
    }
}

async fn stream_openai_chat_completion<EmitFn>(
    config: &LlmConfig,
    input: &str,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let client = Client::new();
    let url = build_openai_stream_url(config);
    let payload = if config.provider == LlmProvider::AzureOpenAi {
        json!({
            "messages": [
                {
                    "role": "user",
                    "content": input,
                }
            ],
            "temperature": config.temperature.unwrap_or(0.7),
            "stream": true,
        })
    } else {
        json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": input,
                }
            ],
            "temperature": config.temperature.unwrap_or(0.7),
            "stream": true,
        })
    };

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    if config.provider == LlmProvider::AzureOpenAi {
        request = request.header("api-key", config.api_key.clone());
    } else if !config.api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if !content_type.contains("text/event-stream") {
        let body = response.text().await.map_err(|error| error.to_string())?;
        let json = serde_json::from_str::<Value>(&body).map_err(|error| error.to_string())?;
        return Ok(StandardLlmResponse {
            text: extract_text_from_json_response(&json)?,
            usage: extract_usage_from_json_response(&json),
        });
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if data.trim() == "[DONE]" {
                continue;
            }

            let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
            if let Some(delta) = event
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
            {
                accumulator.push(delta)?;
            }
        }
    }

    for data in sse.flush() {
        if data.trim() == "[DONE]" {
            continue;
        }

        let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
        if let Some(delta) = event
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            accumulator.push(delta)?;
        }
    }

    if accumulator.text.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: None,
    })
}

async fn stream_openai_responses_completion<EmitFn>(
    config: &LlmConfig,
    input: &str,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let client = Client::new();
    let url = join_url(
        &config.base_url,
        config.api_path.as_deref().unwrap_or("/v1/responses"),
    );
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&json!({
            "model": config.model,
            "input": input,
            "temperature": config.temperature.unwrap_or(0.7),
            "stream": true,
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if !content_type.contains("text/event-stream") {
        let body = response.text().await.map_err(|error| error.to_string())?;
        let json = serde_json::from_str::<Value>(&body).map_err(|error| error.to_string())?;
        return Ok(StandardLlmResponse {
            text: extract_text_from_json_response(&json)?,
            usage: extract_usage_from_json_response(&json),
        });
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if data.trim() == "[DONE]" {
                continue;
            }

            let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
            if let Some(event_type) = event.get("type").and_then(Value::as_str) {
                if event_type.contains("output_text.delta")
                    || event_type.contains("refusal.delta")
                {
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        accumulator.push(delta)?;
                    }
                }
            }
        }
    }

    for data in sse.flush() {
        if data.trim() == "[DONE]" {
            continue;
        }

        let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
        if let Some(event_type) = event.get("type").and_then(Value::as_str) {
            if event_type.contains("output_text.delta") || event_type.contains("refusal.delta") {
                if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                    accumulator.push(delta)?;
                }
            }
        }
    }

    if accumulator.text.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: None,
    })
}

async fn try_stream_text<EmitFn>(
    request: &LlmGenerateRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<Option<StandardLlmResponse>, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let input = build_standard_input(&StandardLlmRequest {
        messages: vec![StandardMessage {
            role: MessageRole::User,
            content: request.input.clone(),
        }],
        temperature: request.config.temperature.unwrap_or(0.7),
        max_tokens: None,
    });

    let response = match request.config.provider {
        LlmProvider::Anthropic => {
            let client = anthropic::Client::builder()
                .api_key(&request.config.api_key)
                .base_url(&request.config.base_url)
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProvider::Gemini => {
            let client = gemini::Client::builder()
                .api_key(&request.config.api_key)
                .base_url(clean_gemini_base_url(&request.config.base_url))
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProvider::Ollama => {
            let client = ollama::Client::builder()
                .api_key(Nothing)
                .base_url(request.config.base_url.trim_end_matches("/v1"))
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProvider::OpenAiResponses => {
            Some(stream_openai_responses_completion(&request.config, &input, accumulator).await?)
        }
        LlmProvider::GoogleTranslate | LlmProvider::GoogleTranslateFree => None,
        _ => Some(stream_openai_chat_completion(&request.config, &input, accumulator).await?),
    };

    Ok(response)
}

async fn generate_with_optional_streaming<EmitFn>(
    request: LlmGenerateRequest,
    emit_delta: &mut EmitFn,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let mut accumulator = StreamTextAccumulator::new(emit_delta);
    let stream_result = try_stream_text(&request, &mut accumulator).await;
    let emitted_any = accumulator.emitted_any;
    drop(accumulator);

    match stream_result {
        Ok(Some(response)) => Ok(response),
        Ok(None) => generate_with_rig(request).await,
        Err(error) if !emitted_any => {
            warn!(
                "[LLM] streaming unavailable or failed before first token, falling back to buffered generate: provider={:?} error={}",
                request.config.provider, error
            );
            generate_with_rig(request).await
        }
        Err(error) => Err(error),
    }
}

async fn post_json_request(
    url: &str,
    headers: Vec<(&str, String)>,
    body: Value,
) -> Result<Value, String> {
    let client = Client::new();
    let mut request = client.post(url).header("Content-Type", "application/json");

    for (key, value) in headers {
        if !value.is_empty() {
            request = request.header(key, value);
        }
    }

    let response = request
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;

    if !status.is_success() {
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    serde_json::from_str(&text).map_err(|error| error.to_string())
}

async fn generate_with_openai_chat_api(
    url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    extra_headers: Vec<(&str, String)>,
) -> Result<StandardLlmResponse, String> {
    let mut headers = vec![];
    if !api_key.is_empty() {
        headers.push(("Authorization", format!("Bearer {}", api_key)));
    }
    headers.extend(extra_headers);

    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": input,
            }
        ],
        "temperature": temperature.unwrap_or(0.7),
    });

    let response = post_json_request(url, headers, payload).await?;
    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

async fn generate_with_openai_responses_api(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    api_path: Option<&str>,
) -> Result<StandardLlmResponse, String> {
    let url = join_url(base_url, api_path.unwrap_or("/v1/responses"));
    let payload = json!({
        "model": model,
        "input": input,
        "temperature": temperature.unwrap_or(0.7),
    });

    let response = post_json_request(
        &url,
        vec![("Authorization", format!("Bearer {}", api_key))],
        payload,
    )
    .await?;

    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

async fn generate_with_azure_openai(
    base_url: &str,
    api_key: &str,
    deployment: &str,
    input: &str,
    temperature: Option<f32>,
    api_version: Option<&str>,
) -> Result<StandardLlmResponse, String> {
    let version = api_version.unwrap_or("2024-10-21");
    let deployment = deployment.trim();
    let endpoint = format!(
        "{}/openai/deployments/{}/chat/completions?api-version={}",
        base_url.trim_end_matches('/'),
        deployment,
        version
    );

    let payload = json!({
        "messages": [
            {
                "role": "user",
                "content": input,
            }
        ],
        "temperature": temperature.unwrap_or(0.7),
    });

    let response =
        post_json_request(&endpoint, vec![("api-key", api_key.to_string())], payload).await?;

    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

async fn generate_with_openai_custom_path(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    api_path: Option<&str>,
) -> Result<StandardLlmResponse, String> {
    let url = join_url(base_url, api_path.unwrap_or("/v1/chat/completions"));
    generate_with_openai_chat_api(&url, api_key, model, input, temperature, vec![]).await
}

async fn generate_with_perplexity(
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<StandardLlmResponse, String> {
    generate_with_openai_chat_api(
        "https://api.perplexity.ai/chat/completions",
        api_key,
        model,
        input,
        temperature,
        vec![],
    )
    .await
}

async fn generate_with_rig(request: LlmGenerateRequest) -> Result<StandardLlmResponse, String> {
    let adapter = AdapterFactory::create(request.config.provider);
    let std_req = StandardLlmRequest {
        messages: vec![StandardMessage {
            role: MessageRole::User,
            content: request.input,
        }],
        temperature: request.config.temperature.unwrap_or(0.7),
        max_tokens: None,
    };

    let client = Client::new();
    let response = adapter
        .generate(&client, &std_req, &request.config)
        .await?;

    Ok(response)
}
