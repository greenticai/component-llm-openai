use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = ".greentic.wasi")]
static WASI_TARGET_MARKER: [u8; 13] = *b"wasm32-wasip2";

#[cfg(target_arch = "wasm32")]
mod component {
    use greentic_interfaces_guest::component::node::{
        self, ExecCtx, InvokeResult, LifecycleStatus, StreamEvent,
    };

    use super::{
        GuestHost, TenantContext, describe_payload, handle_invocation, handle_invocation_stream,
    };

    pub(super) struct Component;

    impl node::Guest for Component {
        fn get_manifest() -> String {
            describe_payload()
        }

        fn on_start(_ctx: ExecCtx) -> Result<LifecycleStatus, String> {
            Ok(LifecycleStatus::Ok)
        }

        fn on_stop(_ctx: ExecCtx, _reason: String) -> Result<LifecycleStatus, String> {
            Ok(LifecycleStatus::Ok)
        }

        fn invoke(ctx: ExecCtx, op: String, input: String) -> InvokeResult {
            let tenant = TenantContext::from(&ctx);
            match handle_invocation(&op, &input, &GuestHost, Some(&tenant)) {
                Ok(output) => InvokeResult::Ok(output),
                Err(err) => InvokeResult::Err(err.into_node_error()),
            }
        }

        fn invoke_stream(ctx: ExecCtx, op: String, input: String) -> Vec<StreamEvent> {
            let tenant = TenantContext::from(&ctx);
            handle_invocation_stream(&op, &input, &GuestHost, Some(&tenant))
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod exports {
    use super::component::Component;
    use greentic_interfaces_guest::component::node;

    #[unsafe(export_name = "greentic:component/node@0.4.0#get-manifest")]
    unsafe extern "C" fn export_get_manifest() -> *mut u8 {
        unsafe { node::_export_get_manifest_cabi::<Component>() }
    }

    #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#get-manifest")]
    unsafe extern "C" fn post_return_get_manifest(arg0: *mut u8) {
        unsafe { node::__post_return_get_manifest::<Component>(arg0) };
    }

    #[unsafe(export_name = "greentic:component/node@0.4.0#on-start")]
    unsafe extern "C" fn export_on_start(arg0: *mut u8) -> *mut u8 {
        unsafe { node::_export_on_start_cabi::<Component>(arg0) }
    }

    #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#on-start")]
    unsafe extern "C" fn post_return_on_start(arg0: *mut u8) {
        unsafe { node::__post_return_on_start::<Component>(arg0) };
    }

    #[unsafe(export_name = "greentic:component/node@0.4.0#on-stop")]
    unsafe extern "C" fn export_on_stop(arg0: *mut u8) -> *mut u8 {
        unsafe { node::_export_on_stop_cabi::<Component>(arg0) }
    }

    #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#on-stop")]
    unsafe extern "C" fn post_return_on_stop(arg0: *mut u8) {
        unsafe { node::__post_return_on_stop::<Component>(arg0) };
    }

    #[unsafe(export_name = "greentic:component/node@0.4.0#invoke")]
    unsafe extern "C" fn export_invoke(arg0: *mut u8) -> *mut u8 {
        unsafe { node::_export_invoke_cabi::<Component>(arg0) }
    }

    #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#invoke")]
    unsafe extern "C" fn post_return_invoke(arg0: *mut u8) {
        unsafe { node::__post_return_invoke::<Component>(arg0) };
    }

    #[unsafe(export_name = "greentic:component/node@0.4.0#invoke-stream")]
    unsafe extern "C" fn export_invoke_stream(arg0: *mut u8) -> *mut u8 {
        unsafe { node::_export_invoke_stream_cabi::<Component>(arg0) }
    }

    #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#invoke-stream")]
    unsafe extern "C" fn post_return_invoke_stream(arg0: *mut u8) {
        unsafe { node::__post_return_invoke_stream::<Component>(arg0) };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    #[default]
    Openai,
    Ollama,
    Openrouter,
    Together,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOpenaiConfig {
    /// Provider to talk to. Defaults to "openai".
    #[serde(default)]
    pub provider: LlmProvider,
    /// Optional override of the base URL. Required for Custom.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Secret key name for auth (looked up via greentic-secrets).
    #[serde(default)]
    pub api_key_secret: Option<String>,
    /// Optional default model for this component instance.
    #[serde(default)]
    pub default_model: Option<String>,
    /// Optional timeout (ms) if the host supports it.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl Default for LlmOpenaiConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Openai,
            base_url: None,
            api_key_secret: None,
            default_model: None,
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOpenaiRequest {
    /// Optional override of the model for this call.
    #[serde(default)]
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Escape hatch for provider-specific things if needed.
    #[serde(default)]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmOpenaiResponse {
    /// The main textual output from the assistant.
    pub completion: String,
    /// Optional full conversation including the assistant reply.
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    /// Raw provider JSON response for debugging / advanced usage.
    #[serde(default)]
    pub raw_provider_response: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InvocationPayload {
    #[serde(default)]
    config: Option<LlmOpenaiConfig>,
    input: LlmOpenaiRequest,
}

#[derive(Debug, Clone)]
struct ResolvedCall {
    _provider: LlmProvider,
    base_url: String,
    model: String,
    api_key: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug)]
pub struct ComponentError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub backoff_ms: Option<u64>,
    pub details: Option<Value>,
}

impl ComponentError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            backoff_ms: None,
            details: None,
        }
    }

    fn retryable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: true,
            backoff_ms: None,
            details: None,
        }
    }

    fn to_error_json(&self) -> Value {
        json!({
            "error": {
                "code": self.code,
                "message": self.message,
                "retryable": self.retryable,
                "backoff_ms": self.backoff_ms,
                "details": self.details,
            }
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn into_node_error(self) -> greentic_interfaces_guest::component::node::NodeError {
        greentic_interfaces_guest::component::node::NodeError {
            code: self.code,
            message: self.message,
            retryable: self.retryable,
            backoff_ms: self.backoff_ms,
            details: self.details.map(|val| val.to_string()),
        }
    }
}

fn default_base_url(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::Openai => "https://api.openai.com/v1",
        LlmProvider::Ollama => "http://localhost:11434/v1",
        LlmProvider::Openrouter => "https://openrouter.ai/api/v1",
        LlmProvider::Together => "https://api.together.xyz/v1",
        LlmProvider::Custom => "",
    }
}

fn default_model(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::Openai => "gpt-4.1-mini",
        LlmProvider::Ollama => "llama3:8b",
        LlmProvider::Openrouter => "openrouter/auto",
        LlmProvider::Together => "togethercomputer/llama-3-8b-instruct",
        LlmProvider::Custom => "gpt-4.1-mini",
    }
}

pub trait Host {
    fn fetch_http(
        &self,
        request: HttpRequest,
        tenant: Option<&TenantContext>,
    ) -> Result<HttpResponse, ComponentError>;
    fn get_secret(&self, name: &str) -> Result<Option<String>, ComponentError>;
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Map<String, Value>,
    pub body: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Map<String, Value>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TenantContext {
    pub tenant: Option<String>,
    pub team: Option<String>,
    pub user: Option<String>,
    pub trace_id: Option<String>,
    pub correlation_id: Option<String>,
    pub deadline_ms: Option<u64>,
    pub attempt: Option<u32>,
    pub idempotency_key: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl From<&greentic_interfaces_guest::component::node::ExecCtx> for TenantContext {
    fn from(ctx: &greentic_interfaces_guest::component::node::ExecCtx) -> Self {
        Self {
            tenant: Some(ctx.tenant.tenant.clone()),
            team: ctx.tenant.team.clone(),
            user: ctx.tenant.user.clone(),
            trace_id: ctx.tenant.trace_id.clone(),
            correlation_id: ctx.tenant.correlation_id.clone(),
            deadline_ms: ctx.tenant.deadline_unix_ms,
            attempt: Some(ctx.tenant.attempt),
            idempotency_key: ctx.tenant.idempotency_key.clone(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl TenantContext {
    fn into_http_tenant_ctx(
        &self,
    ) -> greentic_interfaces_guest::bindings::greentic_http_1_0_0_client::greentic::http::http_client::TenantCtx{
        use greentic_interfaces_guest::bindings::greentic_http_1_0_0_client::greentic::http::http_client::TenantCtx;

        TenantCtx {
            env: String::new(),
            tenant: self.tenant.clone().unwrap_or_default(),
            tenant_id: String::new(),
            team: self.team.clone(),
            team_id: None,
            user: self.user.clone(),
            user_id: None,
            trace_id: self.trace_id.clone(),
            correlation_id: self.correlation_id.clone(),
            attributes: Vec::new(),
            session_id: None,
            flow_id: None,
            node_id: None,
            provider_id: None,
            deadline_ms: self.deadline_ms.map(|v| v as i64),
            attempt: self.attempt.unwrap_or_default(),
            idempotency_key: self.idempotency_key.clone(),
            impersonation: None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct GuestHost;

#[cfg(target_arch = "wasm32")]
impl Host for GuestHost {
    fn fetch_http(
        &self,
        request: HttpRequest,
        tenant: Option<&TenantContext>,
    ) -> Result<HttpResponse, ComponentError> {
        use greentic_interfaces_guest::bindings::greentic_http_1_0_0_client::greentic::http::http_client::{
            self, Request,
        };

        let mut headers_vec = Vec::with_capacity(request.headers.len());
        for (k, v) in request.headers {
            let Some(value) = v.as_str() else {
                return Err(ComponentError::new(
                    "invalid-header",
                    format!("header `{k}` is not a string"),
                ));
            };
            headers_vec.push((k, value.to_owned()));
        }

        let req = Request {
            method: request.method,
            url: request.url,
            headers: headers_vec,
            body: Some(request.body.into_bytes()),
        };

        let tenant_ctx = tenant.map(TenantContext::into_http_tenant_ctx);

        match http_client::send(&req, tenant_ctx.as_ref()) {
            Ok(res) => {
                let mut headers = Map::new();
                for (k, v) in res.headers {
                    headers.insert(k, Value::String(v));
                }

                Ok(HttpResponse {
                    status: res.status,
                    headers,
                    body: res
                        .body
                        .map(|bytes| String::from_utf8(bytes).unwrap_or_default()),
                })
            }
            Err(err) => Err(ComponentError::new(
                "http-request-failed",
                format!("host http send failed: {} ({})", err.message, err.code),
            )),
        }
    }

    fn get_secret(&self, name: &str) -> Result<Option<String>, ComponentError> {
        use greentic_interfaces_guest::bindings::greentic_secrets_1_0_0_store::greentic::secrets::secret_store;

        if name.is_empty() {
            return Ok(None);
        }

        match secret_store::read(name) {
            Ok(bytes) => Ok(Some(String::from_utf8(bytes).unwrap_or_default())),
            Err(err) => Err(ComponentError::new(
                "secret-resolution-failed",
                format!(
                    "failed to read secret `{name}`: {} ({})",
                    err.message, err.code
                ),
            )),
        }
    }
}

// Export shim for greentic:secrets/secrets@0.1.0 by delegating to the host secret store.
#[cfg(target_arch = "wasm32")]
mod secrets_shim {
    use greentic_interfaces_guest::bindings::greentic_secrets_0_1_0_host::exports::greentic::secrets::secrets::{
        Guest, __post_return_get, _export_get_cabi,
    };

    pub struct SecretsShim;

    impl Guest for SecretsShim {
        fn get(uri: String) -> Vec<u8> {
            use greentic_interfaces_guest::bindings::greentic_secrets_1_0_0_store::greentic::secrets::secret_store;

            match secret_store::read(&uri) {
                Ok(bytes) => bytes,
                Err(_) => Vec::new(),
            }
        }
    }

    #[unsafe(export_name = "greentic:secrets/secrets@0.1.0#get")]
    unsafe extern "C" fn export_get(arg0: *mut u8, arg1: usize) -> *mut u8 {
        unsafe { _export_get_cabi::<SecretsShim>(arg0, arg1) }
    }

    #[unsafe(export_name = "cabi_post_greentic:secrets/secrets@0.1.0#get")]
    unsafe extern "C" fn post_return_get(arg0: *mut u8) {
        unsafe { __post_return_get::<SecretsShim>(arg0) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct GuestHost;

#[cfg(not(target_arch = "wasm32"))]
impl Host for GuestHost {
    fn fetch_http(
        &self,
        _request: HttpRequest,
        _tenant: Option<&TenantContext>,
    ) -> Result<HttpResponse, ComponentError> {
        Err(ComponentError::new(
            "unsupported-target",
            "HTTP dispatch is only available in wasm32 builds",
        ))
    }

    fn get_secret(&self, _name: &str) -> Result<Option<String>, ComponentError> {
        Ok(None)
    }
}

pub fn describe_payload() -> String {
    json!({
        "component": {
            "name": "component-llm-openai",
            "org": "ai.greentic",
            "version": "0.1.0",
            "world": "greentic:component/component@0.4.0",
            "schemas": {
                "component": "schemas/component.schema.json",
                "input": "schemas/io/input.schema.json",
                "output": "schemas/io/output.schema.json"
            }
        }
    })
    .to_string()
}

pub fn handle_message(operation: &str, input: &str) -> String {
    match handle_invocation(operation, input, &GuestHost, None) {
        Ok(output) => output,
        Err(err) => err.to_error_json().to_string(),
    }
}

fn handle_invocation<H: Host>(
    _operation: &str,
    input: &str,
    host: &H,
    tenant: Option<&TenantContext>,
) -> Result<String, ComponentError> {
    let payload: InvocationPayload = serde_json::from_str(input).map_err(|err| {
        ComponentError::new(
            "invalid-input",
            format!("failed to parse input JSON: {err}"),
        )
    })?;
    let config = payload.config.unwrap_or_default();
    let response = call_model(host, &config, payload.input, tenant)?;
    serde_json::to_string(&response)
        .map_err(|err| ComponentError::new("serialization-failed", err.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn handle_invocation_stream<H: Host>(
    _operation: &str,
    input: &str,
    host: &H,
    tenant: Option<&TenantContext>,
) -> Vec<greentic_interfaces_guest::component::node::StreamEvent> {
    use greentic_interfaces_guest::component::node::StreamEvent;

    let payload: InvocationPayload = match serde_json::from_str(input) {
        Ok(val) => val,
        Err(err) => {
            return vec![StreamEvent::Error(format!(
                "invalid-input: failed to parse input JSON: {err}"
            ))];
        }
    };
    let config = payload.config.unwrap_or_default();
    match call_model_stream(host, &config, payload.input, tenant) {
        Ok((deltas, response)) => {
            let mut events = Vec::with_capacity(deltas.len() + 3);
            events.push(StreamEvent::Progress(0));
            for delta in deltas {
                events.push(StreamEvent::Data(json!({ "delta": delta }).to_string()));
            }
            match serde_json::to_string(&response) {
                Ok(body) => events.push(StreamEvent::Data(body)),
                Err(err) => events.push(StreamEvent::Error(format!("serialization-failed: {err}"))),
            }
            events.push(StreamEvent::Done);
            events
        }
        Err(err) => vec![StreamEvent::Error(err.message)],
    }
}

fn call_model<H: Host>(
    host: &H,
    config: &LlmOpenaiConfig,
    request: LlmOpenaiRequest,
    tenant: Option<&TenantContext>,
) -> Result<LlmOpenaiResponse, ComponentError> {
    call_model_inner(host, config, request, tenant, false).map(|(_, resp)| resp)
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn call_model_stream<H: Host>(
    host: &H,
    config: &LlmOpenaiConfig,
    request: LlmOpenaiRequest,
    tenant: Option<&TenantContext>,
) -> Result<(Vec<String>, LlmOpenaiResponse), ComponentError> {
    call_model_inner(host, config, request, tenant, true)
}

fn call_model_inner<H: Host>(
    host: &H,
    config: &LlmOpenaiConfig,
    request: LlmOpenaiRequest,
    tenant: Option<&TenantContext>,
    stream: bool,
) -> Result<(Vec<String>, LlmOpenaiResponse), ComponentError> {
    if request.messages.is_empty() {
        return Err(ComponentError::new(
            "missing-messages",
            "at least one message is required",
        ));
    }

    let call = resolve_call(host, config, &request)?;
    let url = format!(
        "{}{}",
        call.base_url.trim_end_matches('/'),
        CHAT_COMPLETIONS_PATH
    );

    let mut body_map = build_request_body(&request, &call.model)?;
    if stream {
        body_map.insert("stream".to_string(), Value::Bool(true));
    }
    if let Some(extra) = request.extra {
        merge_extra_fields(&mut body_map, extra)?;
    }
    let body = Value::Object(body_map);

    let headers = build_headers(call.api_key.as_deref());
    let http_req = HttpRequest {
        method: "POST".to_string(),
        url,
        headers,
        body: body.to_string(),
        timeout_ms: call.timeout_ms,
    };

    let http_resp = host.fetch_http(http_req, tenant)?;
    if http_resp.status >= 400 {
        let body = http_resp.body.unwrap_or_default();
        return Err(ComponentError::retryable(
            "http-error",
            format!("provider returned status {}: {}", http_resp.status, body),
        ));
    }

    let body_str = http_resp
        .body
        .as_deref()
        .ok_or_else(|| ComponentError::new("empty-response", "provider response was empty"))?;

    if stream {
        parse_streaming_response(body_str, &request.messages)
    } else {
        parse_response(body_str, &request.messages).map(|resp| (Vec::new(), resp))
    }
}

fn resolve_call<H: Host>(
    host: &H,
    config: &LlmOpenaiConfig,
    request: &LlmOpenaiRequest,
) -> Result<ResolvedCall, ComponentError> {
    let provider = config.provider.clone();
    let base_url = config
        .base_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_base_url(&provider));

    if matches!(provider, LlmProvider::Custom) && base_url.is_empty() {
        return Err(ComponentError::new(
            "missing-base-url",
            "custom provider requires base_url",
        ));
    }

    let api_key = match &config.api_key_secret {
        Some(secret_name) => host.get_secret(secret_name)?.filter(|s| !s.is_empty()),
        None => None,
    };

    if api_key.is_none() && key_required(&provider) {
        return Err(ComponentError::new(
            "missing-api-key",
            format!("provider {provider:?} requires an API key"),
        ));
    }

    let model = request
        .model
        .as_deref()
        .or(config.default_model.as_deref())
        .unwrap_or_else(|| default_model(&provider))
        .to_string();

    Ok(ResolvedCall {
        _provider: provider,
        base_url: base_url.to_string(),
        model,
        api_key,
        timeout_ms: config.timeout_ms,
    })
}

fn build_request_body(
    request: &LlmOpenaiRequest,
    model: &str,
) -> Result<Map<String, Value>, ComponentError> {
    let mut body = Map::new();
    body.insert("model".to_string(), Value::String(model.to_string()));
    body.insert(
        "messages".to_string(),
        Value::Array(
            request
                .messages
                .iter()
                .map(|m| json!({ "role": m.role, "content": m.content }))
                .collect(),
        ),
    );

    if let Some(temp) = request.temperature
        && let Some(num) = serde_json::Number::from_f64(temp as f64)
    {
        body.insert("temperature".to_string(), Value::Number(num));
    }
    if let Some(top_p) = request.top_p
        && let Some(num) = serde_json::Number::from_f64(top_p as f64)
    {
        body.insert("top_p".to_string(), Value::Number(num));
    }
    if let Some(max_tokens) = request.max_tokens {
        body.insert(
            "max_tokens".to_string(),
            Value::Number(serde_json::Number::from(max_tokens)),
        );
    }

    Ok(body)
}

fn merge_extra_fields(base: &mut Map<String, Value>, extra: Value) -> Result<(), ComponentError> {
    let Value::Object(extra_obj) = extra else {
        return Err(ComponentError::new(
            "invalid-extra",
            "extra must be an object",
        ));
    };

    for (key, value) in extra_obj {
        // Do not overwrite required keys.
        if base.contains_key(&key) {
            continue;
        }
        base.insert(key, value);
    }
    Ok(())
}

fn build_headers(api_key: Option<&str>) -> Map<String, Value> {
    let mut headers = Map::new();
    headers.insert(
        "Content-Type".to_string(),
        Value::String("application/json".to_string()),
    );
    if let Some(key) = api_key {
        headers.insert(
            "Authorization".to_string(),
            Value::String(format!("Bearer {key}")),
        );
    }
    headers
}

fn parse_response(
    body: &str,
    prior_messages: &[ChatMessage],
) -> Result<LlmOpenaiResponse, ComponentError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| ComponentError::new("invalid-response-json", err.to_string()))?;

    let message_value = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| {
            ComponentError::new("missing-message", "no choices[0].message in response")
        })?;

    let content = message_value
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| ComponentError::new("missing-content", "assistant content was empty"))?
        .to_string();

    let role = message_value
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("assistant")
        .to_string();

    let mut messages = prior_messages.to_vec();
    messages.push(ChatMessage {
        role,
        content: content.clone(),
    });

    Ok(LlmOpenaiResponse {
        completion: content,
        messages,
        raw_provider_response: Some(value),
    })
}

fn parse_streaming_response(
    body: &str,
    prior_messages: &[ChatMessage],
) -> Result<(Vec<String>, LlmOpenaiResponse), ComponentError> {
    let has_data_prefix = body
        .lines()
        .any(|line| line.trim_start().starts_with("data:"));
    if !has_data_prefix {
        return parse_response(body, prior_messages).map(|resp| (Vec::new(), resp));
    }

    let mut deltas = Vec::new();
    let mut last_json: Option<Value> = None;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(payload) = trimmed.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        let value: Value = serde_json::from_str(payload)
            .map_err(|err| ComponentError::new("invalid-stream-chunk", err.to_string()))?;
        last_json = Some(value.clone());
        if let Some(content) = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("delta").or_else(|| choice.get("message")))
            .and_then(|msg| msg.get("content"))
            .and_then(Value::as_str)
            && !content.is_empty()
        {
            deltas.push(content.to_string());
        }
    }

    if deltas.is_empty() {
        return Err(ComponentError::new(
            "stream-empty",
            "no streaming chunks received",
        ));
    }

    let completion: String = deltas.iter().cloned().collect();
    let role = last_json
        .as_ref()
        .and_then(|value| {
            value
                .get("choices")
                .and_then(|choices| choices.get(0))
                .and_then(|choice| choice.get("delta").or_else(|| choice.get("message")))
                .and_then(|msg| msg.get("role"))
                .and_then(Value::as_str)
        })
        .unwrap_or("assistant")
        .to_string();

    let mut messages = prior_messages.to_vec();
    messages.push(ChatMessage {
        role,
        content: completion.clone(),
    });

    Ok((
        deltas,
        LlmOpenaiResponse {
            completion,
            messages,
            raw_provider_response: last_json,
        },
    ))
}

fn key_required(provider: &LlmProvider) -> bool {
    matches!(
        provider,
        LlmProvider::Openai | LlmProvider::Openrouter | LlmProvider::Together
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    #[derive(Default)]
    struct MockHost {
        secrets: HashMap<String, Option<String>>,
        responses: VecDeque<HttpResponse>,
    }

    impl MockHost {
        fn with_secret(mut self, name: &str, value: Option<&str>) -> Self {
            self.secrets
                .insert(name.to_string(), value.map(|v| v.to_string()));
            self
        }
    }

    impl Host for MockHost {
        fn fetch_http(
            &self,
            request: HttpRequest,
            _tenant: Option<&TenantContext>,
        ) -> Result<HttpResponse, ComponentError> {
            let _ = request;
            let mut responses = self.responses.clone();
            responses
                .pop_front()
                .ok_or_else(|| ComponentError::new("no-mock-response", "no HTTP response queued"))
        }

        fn get_secret(&self, name: &str) -> Result<Option<String>, ComponentError> {
            Ok(self.secrets.get(name).cloned().flatten())
        }
    }

    fn sample_request() -> LlmOpenaiRequest {
        LlmOpenaiRequest {
            model: None,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            temperature: None,
            top_p: None,
            max_tokens: None,
            extra: None,
        }
    }

    #[test]
    fn default_provider_and_base_url() {
        let config = LlmOpenaiConfig {
            api_key_secret: Some("OPENAI_API_KEY".into()),
            ..Default::default()
        };
        let host = MockHost::default().with_secret("OPENAI_API_KEY", Some("token"));
        let resolved = resolve_call(&host, &config, &sample_request()).expect("resolve");
        assert_eq!(resolved._provider, LlmProvider::Openai);
        assert_eq!(resolved.base_url, "https://api.openai.com/v1");
        assert_eq!(resolved.model, "gpt-4.1-mini");
    }

    #[test]
    fn ollama_defaults() {
        let config = LlmOpenaiConfig {
            provider: LlmProvider::Ollama,
            ..Default::default()
        };
        let host = MockHost::default();
        let resolved = resolve_call(&host, &config, &sample_request()).expect("resolve");
        assert_eq!(resolved.base_url, "http://localhost:11434/v1");
        assert_eq!(resolved.model, "llama3:8b");
    }

    #[test]
    fn custom_requires_base_url() {
        let config = LlmOpenaiConfig {
            provider: LlmProvider::Custom,
            base_url: None,
            ..Default::default()
        };
        let host = MockHost::default();
        let err = resolve_call(&host, &config, &sample_request()).unwrap_err();
        assert_eq!(err.code, "missing-base-url");
    }

    #[test]
    fn model_resolution_priority() {
        let req = LlmOpenaiRequest {
            model: Some("req-model".into()),
            ..sample_request()
        };
        let config = LlmOpenaiConfig {
            default_model: Some("config-model".into()),
            api_key_secret: Some("OPENAI_API_KEY".into()),
            ..Default::default()
        };
        let host = MockHost::default().with_secret("OPENAI_API_KEY", Some("token"));
        let resolved = resolve_call(&host, &config, &req).expect("resolve");
        assert_eq!(resolved.model, "req-model");
    }

    #[test]
    fn config_model_used_when_request_missing() {
        let req = sample_request();
        let config = LlmOpenaiConfig {
            default_model: Some("config-model".into()),
            api_key_secret: Some("OPENAI_API_KEY".into()),
            ..Default::default()
        };
        let host = MockHost::default().with_secret("OPENAI_API_KEY", Some("token"));
        let resolved = resolve_call(&host, &config, &req).expect("resolve");
        assert_eq!(resolved.model, "config-model");
    }

    #[test]
    fn provider_default_model_used_when_no_overrides() {
        let host = MockHost::default().with_secret("OPENAI_API_KEY", Some("token"));
        let resolved = resolve_call(
            &host,
            &LlmOpenaiConfig {
                api_key_secret: Some("OPENAI_API_KEY".into()),
                ..Default::default()
            },
            &sample_request(),
        )
        .unwrap();
        assert_eq!(resolved.model, "gpt-4.1-mini");
    }

    #[test]
    fn auth_required_for_openai() {
        let config = LlmOpenaiConfig::default();
        let host = MockHost::default();
        let err = resolve_call(&host, &config, &sample_request()).unwrap_err();
        assert_eq!(err.code, "missing-api-key");
    }

    #[test]
    fn auth_optional_for_ollama() {
        let config = LlmOpenaiConfig {
            provider: LlmProvider::Ollama,
            ..Default::default()
        };
        let host = MockHost::default();
        let resolved = resolve_call(&host, &config, &sample_request()).unwrap();
        assert!(resolved.api_key.is_none());
    }

    #[test]
    fn parse_basic_response() {
        let body = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "hi there"
                }
            }]
        })
        .to_string();
        let response = parse_response(&body, &sample_request().messages).expect("parsed");
        assert_eq!(response.completion, "hi there");
        assert_eq!(response.messages.last().unwrap().content, "hi there");
        assert!(response.raw_provider_response.is_some());
    }

    #[test]
    fn parse_streaming_sse() {
        let sse = "\
data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\
\n\
data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\
\n\
data: [DONE]\n";
        let (deltas, response) =
            parse_streaming_response(sse, &sample_request().messages).expect("stream parsed");
        assert_eq!(deltas, vec!["hello ".to_string(), "world".to_string()]);
        assert_eq!(response.completion, "hello world");
        assert_eq!(response.messages.last().unwrap().content, "hello world");
    }
}
