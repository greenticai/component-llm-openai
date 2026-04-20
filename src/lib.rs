use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

mod i18n;
mod i18n_bundle;
mod qa;

const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";
const COMPONENT_NAME: &str = "component-llm-openai";
const COMPONENT_ID: &str = "ai.greentic.component-llm-openai";
const COMPONENT_VERSION: &str = "0.1.0";
const COMPONENT_WORLD: &str = "greentic:component/component@0.6.0";
const DEFAULT_OPERATION: &str = "handle_message";

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = ".greentic.wasi")]
static WASI_TARGET_MARKER: [u8; 13] = *b"wasm32-wasip2";

#[cfg(target_arch = "wasm32")]
mod component {
    use greentic_interfaces_guest::component_v0_6::node;
    use serde_json::Value;

    use super::{
        DEFAULT_OPERATION, GuestHost, TenantContext, apply_answers_value_from_bytes,
        component_descriptor, encode_cbor, handle_invocation, parse_payload,
    };

    pub(super) struct Component;

    impl node::Guest for Component {
        fn describe() -> node::ComponentDescriptor {
            component_descriptor()
        }

        fn invoke(
            op: String,
            envelope: node::InvocationEnvelope,
        ) -> Result<node::InvocationResult, node::NodeError> {
            let tenant = TenantContext::from(&envelope.ctx);
            let operation = if op.is_empty() {
                DEFAULT_OPERATION
            } else {
                &op
            };
            let output_value = match operation {
                "qa-spec" => super::qa_spec_value(&parse_payload(&envelope.payload_cbor))?,
                "apply-answers" | "setup.apply_answers" => {
                    apply_answers_value_from_bytes(&envelope.payload_cbor)?
                }
                "i18n-keys" => Value::Array(
                    super::qa::i18n_keys()
                        .into_iter()
                        .map(Value::String)
                        .collect(),
                ),
                _ => {
                    let input = super::parse_payload_to_json_string(&envelope.payload_cbor)
                        .map_err(|err| err.into_node_error())?;
                    let output = handle_invocation(operation, &input, &GuestHost, Some(&tenant))
                        .map_err(|err| err.into_node_error())?;
                    serde_json::from_str::<Value>(&output).map_err(|err| {
                        super::ComponentError::new(
                            "serialization-failed",
                            format!("failed to parse component output as JSON: {err}"),
                        )
                        .into_node_error()
                    })?
                }
            };
            Ok(node::InvocationResult {
                ok: true,
                output_cbor: encode_cbor(&output_value).map_err(|err| err.into_node_error())?,
                output_metadata_cbor: None,
            })
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod qa_exports {
    use crate::qa::WizardMode;
    use serde_json::Value;

    wit_bindgen::generate!({
        inline: r#"
            package greentic:component@0.6.0;

            interface component-qa {
                enum qa-mode {
                    default,
                    setup,
                    update,
                    remove
                }

                qa-spec: func(mode: qa-mode) -> list<u8>;
                apply-answers: func(mode: qa-mode, current-config: list<u8>, answers: list<u8>) -> list<u8>;
            }

            interface component-i18n {
                i18n-keys: func() -> list<string>;
            }

            world wizard-support {
                export component-qa;
                export component-i18n;
            }
        "#,
        world: "wizard-support",
    });

    pub struct WizardSupport;

    impl exports::greentic::component::component_qa::Guest for WizardSupport {
        fn qa_spec(mode: exports::greentic::component::component_qa::QaMode) -> Vec<u8> {
            let mode = match mode {
                exports::greentic::component::component_qa::QaMode::Default => WizardMode::Default,
                exports::greentic::component::component_qa::QaMode::Setup => WizardMode::Setup,
                exports::greentic::component::component_qa::QaMode::Update => WizardMode::Update,
                exports::greentic::component::component_qa::QaMode::Remove => WizardMode::Remove,
            };
            crate::encode_cbor(&crate::qa::qa_spec(mode)).expect("encode qa spec")
        }

        fn apply_answers(
            mode: exports::greentic::component::component_qa::QaMode,
            current_config: Vec<u8>,
            answers: Vec<u8>,
        ) -> Vec<u8> {
            let mode = match mode {
                exports::greentic::component::component_qa::QaMode::Default => WizardMode::Default,
                exports::greentic::component::component_qa::QaMode::Setup => WizardMode::Setup,
                exports::greentic::component::component_qa::QaMode::Update => WizardMode::Update,
                exports::greentic::component::component_qa::QaMode::Remove => WizardMode::Remove,
            };
            let current = crate::parse_payload(&current_config);
            let answers = crate::parse_payload(&answers);
            let value = crate::qa::apply_answers(mode, Some(&current), Some(&answers))
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            crate::encode_cbor(&value).expect("encode wizard answers result")
        }
    }

    impl exports::greentic::component::component_i18n::Guest for WizardSupport {
        fn i18n_keys() -> Vec<String> {
            crate::qa::i18n_keys()
        }
    }

    export!(WizardSupport with_types_in self);
}

#[cfg(target_arch = "wasm32")]
greentic_interfaces_guest::export_component_v060!(component::Component);

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

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
struct SetupApplyPayload {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    current_config_cbor: Option<Vec<u8>>,
    #[serde(default)]
    answers_cbor: Option<Vec<u8>>,
    #[serde(default)]
    current_config: Option<Value>,
    #[serde(default)]
    answers: Option<Value>,
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
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            backoff_ms: None,
            details: None,
        }
    }

    pub fn retryable(code: impl Into<String>, message: impl Into<String>) -> Self {
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
    fn into_node_error(self) -> greentic_interfaces_guest::component_v0_6::node::NodeError {
        let details = self.details.and_then(|val| {
            greentic_types::cbor::canonical::to_canonical_cbor_allow_floats(&val).ok()
        });
        greentic_interfaces_guest::component_v0_6::node::NodeError {
            code: self.code,
            message: self.message,
            retryable: self.retryable,
            backoff_ms: self.backoff_ms,
            details,
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
        // Compatibility fallback only. The wizard will ask for a custom model
        // instead of relying on this.
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
impl From<&greentic_interfaces_guest::component_v0_6::node::TenantCtx> for TenantContext {
    fn from(ctx: &greentic_interfaces_guest::component_v0_6::node::TenantCtx) -> Self {
        Self {
            tenant: (!ctx.tenant_id.is_empty()).then(|| ctx.tenant_id.clone()),
            team: ctx.team_id.clone(),
            user: ctx.user_id.clone(),
            trace_id: (!ctx.trace_id.is_empty()).then(|| ctx.trace_id.clone()),
            correlation_id: (!ctx.correlation_id.is_empty()).then(|| ctx.correlation_id.clone()),
            deadline_ms: (ctx.deadline_ms != u64::MAX).then_some(ctx.deadline_ms),
            attempt: Some(ctx.attempt),
            idempotency_key: ctx.idempotency_key.clone(),
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
            i18n_id: Some(String::new()),
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
        use greentic_interfaces_guest::secrets_store;

        if name.is_empty() {
            return Ok(None);
        }

        match secrets_store::get(name) {
            Ok(Some(bytes)) => Ok(Some(String::from_utf8(bytes).unwrap_or_default())),
            Ok(None) => Ok(None),
            Err(err) => Err(ComponentError::new(
                "secret-resolution-failed",
                format!("failed to read secret `{name}`: {}", err.message()),
            )),
        }
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
            "name": COMPONENT_NAME,
            "org": "ai.greentic",
            "id": COMPONENT_ID,
            "version": COMPONENT_VERSION,
            "world": COMPONENT_WORLD,
            "config_schema": component_config_schema_value(),
            "i18n": i18n_catalog_value(),
            "default_operation": DEFAULT_OPERATION,
            "operations": [
                {
                    "name": DEFAULT_OPERATION,
                    "input_schema": operation_input_schema(),
                    "output_schema": operation_output_schema()
                }
            ]
        }
    })
    .to_string()
}

pub fn i18n_fallback(key: &str) -> Option<String> {
    i18n::en_messages().get(key).cloned()
}

pub fn i18n_catalog_value() -> Value {
    let en = i18n::en_messages()
        .into_iter()
        .map(|(key, value)| (key, Value::String(value)))
        .collect::<Map<String, Value>>();

    json!({
        "default_locale": "en",
        "available_locales": ["en"],
        "messages": {
            "en": en
        }
    })
}

pub fn fixture_key(reference: &str) -> String {
    reference
        .trim_start_matches("oci://")
        .trim_start_matches("repo://")
        .trim_start_matches("store://")
        .trim_start_matches("file://")
        .replace(['/', ':', '@'], "_")
}

pub fn component_describe_ir() -> greentic_types::schemas::component::v0_6_0::ComponentDescribe {
    use std::collections::BTreeMap;

    use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};
    use greentic_types::schemas::component::v0_6_0::{
        ComponentDescribe, ComponentInfo, ComponentOperation, ComponentRunInput,
        ComponentRunOutput, schema_hash,
    };

    let config_schema = component_config_schema_ir();
    let input_schema = SchemaIr::Object {
        properties: BTreeMap::from([
            ("config".to_string(), config_schema.clone()),
            (
                "input".to_string(),
                SchemaIr::Object {
                    properties: BTreeMap::new(),
                    required: vec![],
                    additional: AdditionalProperties::Allow,
                },
            ),
        ]),
        required: vec!["input".to_string()],
        additional: AdditionalProperties::Forbid,
    };
    let output_schema = SchemaIr::Object {
        properties: BTreeMap::from([(
            "completion".to_string(),
            SchemaIr::String {
                min_len: None,
                max_len: None,
                regex: None,
                format: None,
            },
        )]),
        required: vec!["completion".to_string()],
        additional: AdditionalProperties::Allow,
    };
    let operation_hash =
        schema_hash(&input_schema, &output_schema, &config_schema).expect("schema hash");

    ComponentDescribe {
        info: ComponentInfo {
            id: COMPONENT_ID.to_string(),
            version: COMPONENT_VERSION.to_string(),
            role: "tool".to_string(),
            display_name: None,
        },
        provided_capabilities: Vec::new(),
        required_capabilities: Vec::new(),
        metadata: BTreeMap::new(),
        operations: vec![ComponentOperation {
            id: DEFAULT_OPERATION.to_string(),
            display_name: None,
            input: ComponentRunInput {
                schema: input_schema,
            },
            output: ComponentRunOutput {
                schema: output_schema,
            },
            defaults: BTreeMap::new(),
            redactions: Vec::new(),
            constraints: BTreeMap::new(),
            schema_hash: operation_hash,
        }],
        config_schema,
    }
}

fn operation_input_schema() -> Value {
    let mut config_schema = component_config_schema_value();
    if let Some(obj) = config_schema.as_object_mut() {
        obj.remove("$schema");
        obj.remove("title");
    }

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "component-llm-openai invocation input",
        "oneOf": [
            {
                "type": "object",
                "required": ["input"],
                "properties": {
                    "config": {
                        "$ref": "#/$defs/ComponentConfig",
                        "description": "Optional per-invocation component config override for provider, base URL, API key secret reference, default model, and timeout."
                    },
                    "input": {
                        "$ref": "#/$defs/LlmOpenaiRequest",
                        "description": "Per-invocation input. Any model provided here overrides the component-level default_model."
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "description": "Compatibility shape accepted when runtimes pass the LLM request object directly instead of nesting it under `input`.",
                "required": ["messages"],
                "properties": {
                    "config": {
                        "$ref": "#/$defs/ComponentConfig",
                        "description": "Optional per-invocation component config override for provider, base URL, API key secret reference, default model, and timeout."
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional override of the model for this call. Takes precedence over component config default_model."
                    },
                    "messages": {
                        "type": "array",
                        "items": {
                            "$ref": "#/$defs/ChatMessage"
                        },
                        "minItems": 1
                    },
                    "temperature": {
                        "type": "number"
                    },
                    "top_p": {
                        "type": "number"
                    },
                    "max_tokens": {
                        "type": "integer",
                        "minimum": 1
                    },
                    "extra": {
                        "type": "object",
                        "description": "Provider-specific options merged into the outgoing payload.",
                        "additionalProperties": true
                    }
                },
                "additionalProperties": false
            }
        ],
        "$defs": {
            "ComponentConfig": config_schema,
            "ChatMessage": {
                "type": "object",
                "required": ["role", "content"],
                "properties": {
                    "role": {
                        "type": "string",
                        "description": "Message role (system|user|assistant|tool|etc.)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Message body"
                    }
                },
                "additionalProperties": false
            },
            "LlmOpenaiRequest": {
                "type": "object",
                "required": ["messages"],
                "properties": {
                    "model": {
                        "type": "string",
                        "description": "Optional override of the model for this call. Takes precedence over component config default_model."
                    },
                    "messages": {
                        "type": "array",
                        "items": {
                            "$ref": "#/$defs/ChatMessage"
                        },
                        "minItems": 1
                    },
                    "temperature": {
                        "type": "number"
                    },
                    "top_p": {
                        "type": "number"
                    },
                    "max_tokens": {
                        "type": "integer",
                        "minimum": 1
                    },
                    "extra": {
                        "type": "object",
                        "description": "Provider-specific options merged into the outgoing payload.",
                        "additionalProperties": true
                    }
                },
                "additionalProperties": false
            }
        }
    })
}

fn operation_output_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "component-llm-openai response",
        "type": "object",
        "required": ["completion"],
        "properties": {
            "completion": {
                "type": "string",
                "description": "The main textual output from the assistant."
            },
            "messages": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["role", "content"],
                    "properties": {
                        "role": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "additionalProperties": false
                },
                "default": []
            },
            "raw_provider_response": {
                "description": "Raw provider JSON response for debugging / advanced usage."
            }
        },
        "additionalProperties": false
    })
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn setup_apply_input_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "component-llm-openai setup.apply_answers input",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "mode": {
                "type": "string",
                "enum": ["default", "setup", "update", "upgrade", "remove"]
            },
            "current_config_cbor": {
                "description": "Canonical CBOR bytes for the current component config, when present."
            },
            "answers_cbor": {
                "description": "Canonical CBOR bytes for submitted wizard answers."
            },
            "metadata_cbor": {
                "description": "Optional metadata payload reserved for future use."
            }
        },
        "required": ["mode"]
    })
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn component_config_schema_value() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "component-llm-openai component configuration",
        "type": "object",
        "additionalProperties": false,
        "default": {
            "provider": "openai",
            "base_url": Value::Null,
            "api_key_secret": Value::Null,
            "default_model": Value::Null,
            "timeout_ms": Value::Null,
        },
        "examples": [
            {
                "provider": "openai",
                "api_key_secret": "OPENAI_API_KEY",
            },
            {
                "provider": "ollama",
            },
            {
                "provider": "openrouter",
                "api_key_secret": "OPENROUTER_API_KEY",
                "default_model": "openrouter/auto",
            },
            {
                "provider": "custom",
                "base_url": "https://my-llm.example.com/v1",
                "api_key_secret": "MY_LLM_API_KEY",
                "default_model": "gpt-oss-120b",
                "timeout_ms": 30000,
            }
        ],
        "properties": {
            "provider": {
                "type": "string",
                "enum": ["openai", "ollama", "openrouter", "together", "custom"],
                "default": "openai",
                "description": "Which LLM provider to use by default."
            },
            "base_url": {
                "type": ["string", "null"],
                "description": "Optional base URL override. Built-in providers use their standard endpoint unless this is set. Required for provider=custom."
            },
            "api_key_secret": {
                "type": ["string", "null"],
                "description": "Name of the stored secret to resolve at runtime, not the API key value itself. This is a secret reference name."
            },
            "default_model": {
                "type": ["string", "null"],
                "description": "Optional component-level fallback model. Per-invocation input.model overrides this value. Built-in providers use a provider default when this is empty."
            },
            "timeout_ms": {
                "type": ["integer", "null"],
                "minimum": 0,
                "description": "Optional request timeout in milliseconds. Leave empty to use runtime or host defaults."
            }
        }
    })
}

fn component_config_schema_ir() -> greentic_types::schemas::common::schema_ir::SchemaIr {
    use std::collections::BTreeMap;

    use ciborium::value::Value as CborValue;
    use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};

    let string_schema = || SchemaIr::String {
        min_len: None,
        max_len: None,
        regex: None,
        format: None,
    };

    SchemaIr::Object {
        properties: BTreeMap::from([
            (
                "provider".to_string(),
                SchemaIr::Enum {
                    values: vec![
                        CborValue::Text("openai".to_string()),
                        CborValue::Text("ollama".to_string()),
                        CborValue::Text("openrouter".to_string()),
                        CborValue::Text("together".to_string()),
                        CborValue::Text("custom".to_string()),
                    ],
                },
            ),
            (
                "base_url".to_string(),
                SchemaIr::OneOf {
                    variants: vec![string_schema(), SchemaIr::Null],
                },
            ),
            (
                "api_key_secret".to_string(),
                SchemaIr::OneOf {
                    variants: vec![string_schema(), SchemaIr::Null],
                },
            ),
            (
                "default_model".to_string(),
                SchemaIr::OneOf {
                    variants: vec![string_schema(), SchemaIr::Null],
                },
            ),
            (
                "timeout_ms".to_string(),
                SchemaIr::OneOf {
                    variants: vec![
                        SchemaIr::Int {
                            min: Some(0),
                            max: None,
                        },
                        SchemaIr::Null,
                    ],
                },
            ),
        ]),
        required: Vec::new(),
        additional: AdditionalProperties::Forbid,
    }
}

#[cfg(target_arch = "wasm32")]
fn component_descriptor() -> greentic_interfaces_guest::component_v0_6::node::ComponentDescriptor {
    use greentic_interfaces_guest::component_v0_6::node::{
        ComponentDescriptor, IoSchema, Op, SchemaSource,
    };
    let input_schema = encode_cbor(&operation_input_schema()).expect("encode input schema");
    let output_schema = encode_cbor(&operation_output_schema()).expect("encode output schema");
    let setup_apply_input =
        encode_cbor(&setup_apply_input_schema()).expect("encode setup apply input schema");
    let setup_apply_output =
        encode_cbor(&component_config_schema_value()).expect("encode config output schema");

    ComponentDescriptor {
        name: COMPONENT_NAME.to_string(),
        version: COMPONENT_VERSION.to_string(),
        summary: Some("OpenAI-style LLM component for Greentic".to_string()),
        capabilities: Vec::new(),
        ops: vec![
            Op {
                name: DEFAULT_OPERATION.to_string(),
                summary: Some("Invoke the OpenAI-compatible chat completions bridge".to_string()),
                input: IoSchema {
                    schema: SchemaSource::InlineCbor(input_schema),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                output: IoSchema {
                    schema: SchemaSource::InlineCbor(output_schema),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                examples: Vec::new(),
            },
            Op {
                name: "setup.apply_answers".to_string(),
                summary: Some(
                    "Apply component wizard answers and emit canonical config CBOR.".to_string(),
                ),
                input: IoSchema {
                    schema: SchemaSource::InlineCbor(setup_apply_input),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                output: IoSchema {
                    schema: SchemaSource::InlineCbor(setup_apply_output),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                examples: Vec::new(),
            },
        ],
        schemas: Vec::new(),
        setup: None,
    }
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn encode_cbor<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, ComponentError> {
    greentic_types::cbor::canonical::to_canonical_cbor_allow_floats(value)
        .map_err(|err| ComponentError::new("serialization-failed", err.to_string()))
}

pub fn encode_cbor_for_tests<T: serde::Serialize>(
    value: &T,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(greentic_types::cbor::canonical::to_canonical_cbor_allow_floats(value)?)
}

pub fn fixture_qa_spec_cbor(mode: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mode = qa::normalize_mode(mode)
        .ok_or_else(|| std::io::Error::other(format!("unsupported QA mode `{mode}`")))?;
    Ok(greentic_types::cbor::canonical::to_canonical_cbor(
        &qa::qa_spec(mode),
    )?)
}

pub fn fixture_apply_config_cbor(
    mode: &str,
    answers: &Value,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mode = qa::normalize_mode(mode)
        .ok_or_else(|| std::io::Error::other(format!("unsupported QA mode `{mode}`")))?;
    let mut merged_answers = answers
        .as_object()
        .cloned()
        .ok_or_else(|| std::io::Error::other("answers must be an object"))?;
    merge_fixture_question_defaults(&qa::qa_spec(mode), &mut merged_answers)?;
    let config = qa::apply_answers(mode, None, Some(&Value::Object(merged_answers)))
        .map_err(std::io::Error::other)?;
    Ok(greentic_types::cbor::canonical::to_canonical_cbor(&config)?)
}

fn merge_fixture_question_defaults(
    spec: &greentic_types::schemas::component::v0_6_0::ComponentQaSpec,
    answers: &mut Map<String, Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    for question in &spec.questions {
        if answers.contains_key(&question.id) {
            continue;
        }
        if question_is_skipped(question.skip_if.as_ref(), answers)? {
            continue;
        }
        if let Some(default) = &question.default {
            answers.insert(question.id.clone(), serde_json::to_value(default)?);
        }
    }
    Ok(())
}

fn question_is_skipped(
    expr: Option<&greentic_types::schemas::component::v0_6_0::SkipExpression>,
    answers: &Map<String, Value>,
) -> Result<bool, Box<dyn std::error::Error>> {
    use greentic_types::schemas::component::v0_6_0::SkipExpression;

    let Some(expr) = expr else {
        return Ok(false);
    };

    Ok(match expr {
        SkipExpression::Condition(condition) => {
            let answer = answers.get(&condition.field);
            let answer_is_empty = match answer {
                None | Some(Value::Null) => true,
                Some(Value::String(value)) => value.trim().is_empty(),
                Some(Value::Array(values)) => values.is_empty(),
                Some(Value::Object(map)) => map.is_empty(),
                Some(_) => false,
            };
            let equals = if let Some(expected) = &condition.equals {
                answer == Some(&serde_json::to_value(expected)?)
            } else {
                false
            };
            let not_equals = if let Some(expected) = &condition.not_equals {
                answer != Some(&serde_json::to_value(expected)?)
            } else {
                false
            };
            equals
                || not_equals
                || (condition.is_empty && answer_is_empty)
                || (condition.is_not_empty && !answer_is_empty)
        }
        SkipExpression::And(items) => {
            let mut all = true;
            for item in items {
                all = all && question_is_skipped(Some(item), answers)?;
            }
            all
        }
        SkipExpression::Or(items) => {
            let mut any = false;
            for item in items {
                any = any || question_is_skipped(Some(item), answers)?;
            }
            any
        }
        SkipExpression::Not(item) => !question_is_skipped(Some(item), answers)?,
    })
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn parse_payload(input: &[u8]) -> Value {
    use greentic_types::cbor::canonical;

    if let Ok(value) = canonical::from_cbor(input) {
        value
    } else {
        serde_json::from_slice(input).unwrap_or_else(|_| json!({}))
    }
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn parse_payload_to_json_string(input: &[u8]) -> Result<String, ComponentError> {
    serde_json::to_string(&parse_payload(input))
        .map_err(|err| ComponentError::new("serialization-failed", err.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn qa_spec_value(
    payload: &Value,
) -> Result<Value, greentic_interfaces_guest::component_v0_6::node::NodeError> {
    let raw_mode = payload
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let mode = qa::normalize_mode(raw_mode).ok_or_else(|| {
        ComponentError::new(
            "invalid-qa-mode",
            format!("unsupported QA mode `{raw_mode}`"),
        )
        .into_node_error()
    })?;
    serde_json::to_value(qa::qa_spec(mode)).map_err(|err| {
        ComponentError::new("serialization-failed", err.to_string()).into_node_error()
    })
}

#[cfg(target_arch = "wasm32")]
fn apply_answers_value_from_bytes(
    input: &[u8],
) -> Result<Value, greentic_interfaces_guest::component_v0_6::node::NodeError> {
    let payload = decode_setup_apply_payload(input)
        .map_err(|err| ComponentError::new("invalid-input", err).into_node_error())?;
    apply_answers_value(&payload).map_err(|err| err.into_node_error())
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn decode_setup_apply_payload(input: &[u8]) -> Result<SetupApplyPayload, String> {
    greentic_types::cbor::canonical::from_cbor(input).or_else(|cbor_err| {
        serde_json::from_slice(input).map_err(|json_err| {
            format!("decode setup.apply_answers payload: {cbor_err}; json fallback: {json_err}")
        })
    })
}

#[cfg_attr(any(test, not(target_arch = "wasm32")), allow(dead_code))]
fn apply_answers_value(payload: &SetupApplyPayload) -> Result<Value, ComponentError> {
    let raw_mode = payload.mode.as_deref().unwrap_or("setup");
    let mode = qa::normalize_mode(raw_mode).ok_or_else(|| {
        ComponentError::new(
            "invalid-qa-mode",
            format!("unsupported QA mode `{raw_mode}`"),
        )
    })?;

    let current_value = payload.current_config.clone().or_else(|| {
        payload
            .current_config_cbor
            .as_ref()
            .map(|bytes| parse_payload(bytes))
    });
    let answers_value = payload.answers.clone().or_else(|| {
        payload
            .answers_cbor
            .as_ref()
            .map(|bytes| parse_payload(bytes))
    });

    qa::apply_answers(mode, current_value.as_ref(), answers_value.as_ref())
        .map_err(|err| ComponentError::new("invalid-qa-answers", err))
}

pub fn handle_message(operation: &str, input: &str) -> String {
    match handle_invocation(operation, input, &GuestHost, None) {
        Ok(output) => output,
        Err(err) => err.to_error_json().to_string(),
    }
}

#[doc(hidden)]
pub fn invoke_with_host<H: Host>(
    host: &H,
    config: &LlmOpenaiConfig,
    request: LlmOpenaiRequest,
) -> Result<LlmOpenaiResponse, ComponentError> {
    call_model(host, config, request, None)
}

fn handle_invocation<H: Host>(
    _operation: &str,
    input: &str,
    host: &H,
    tenant: Option<&TenantContext>,
) -> Result<String, ComponentError> {
    let payload = parse_invocation_payload(input)?;
    let config = payload.config.unwrap_or_default();
    let response = call_model(host, &config, payload.input, tenant)?;
    serde_json::to_string(&response)
        .map_err(|err| ComponentError::new("serialization-failed", err.to_string()))
}

fn parse_invocation_payload(input: &str) -> Result<InvocationPayload, ComponentError> {
    let value: Value = serde_json::from_str(input).map_err(|err| {
        ComponentError::new(
            "invalid-input",
            format!("failed to parse input JSON: {err}"),
        )
    })?;
    invocation_payload_from_value(value)
}

fn invocation_payload_from_value(value: Value) -> Result<InvocationPayload, ComponentError> {
    match value {
        Value::Object(mut obj) => {
            let config = obj
                .remove("config")
                .map(serde_json::from_value)
                .transpose()
                .map_err(|err| {
                    ComponentError::new(
                        "invalid-input",
                        format!("failed to parse input JSON: {err}"),
                    )
                })?;

            if let Some(input_value) = obj.remove("input") {
                let request = serde_json::from_value(input_value).map_err(|err| {
                    ComponentError::new(
                        "invalid-input",
                        format!("failed to parse input JSON: {err}"),
                    )
                })?;
                return Ok(InvocationPayload {
                    config,
                    input: request,
                });
            }

            let request = serde_json::from_value(Value::Object(obj)).map_err(|err| {
                ComponentError::new(
                    "invalid-input",
                    format!("failed to parse input JSON: {err}"),
                )
            })?;
            Ok(InvocationPayload {
                config,
                input: request,
            })
        }
        other => {
            let request = serde_json::from_value(other).map_err(|err| {
                ComponentError::new(
                    "invalid-input",
                    format!("failed to parse input JSON: {err}"),
                )
            })?;
            Ok(InvocationPayload {
                config: None,
                input: request,
            })
        }
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

#[allow(dead_code)]
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

        fn with_response(mut self, response: HttpResponse) -> Self {
            self.responses.push_back(response);
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
    fn built_in_provider_can_override_base_url() {
        let config = LlmOpenaiConfig {
            provider: LlmProvider::Openai,
            base_url: Some("https://gateway.example.com/v1".into()),
            api_key_secret: Some("OPENAI_API_KEY".into()),
            ..Default::default()
        };
        let host = MockHost::default().with_secret("OPENAI_API_KEY", Some("token"));
        let resolved = resolve_call(&host, &config, &sample_request()).expect("resolve");
        assert_eq!(resolved.base_url, "https://gateway.example.com/v1");
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
    fn custom_provider_uses_compatibility_model_fallback_when_not_overridden() {
        let config = LlmOpenaiConfig {
            provider: LlmProvider::Custom,
            base_url: Some("https://my-llm.example.com/v1".into()),
            ..Default::default()
        };
        let host = MockHost::default();
        let resolved = resolve_call(&host, &config, &sample_request()).expect("resolve");
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
    fn api_key_secret_is_treated_as_secret_reference_name() {
        let config = LlmOpenaiConfig {
            api_key_secret: Some("OPENAI_API_KEY".into()),
            ..Default::default()
        };
        let host = MockHost::default().with_secret("OPENAI_API_KEY", Some("token-from-store"));
        let resolved = resolve_call(&host, &config, &sample_request()).expect("resolve");
        assert_eq!(resolved.api_key.as_deref(), Some("token-from-store"));
    }

    #[test]
    fn generated_schema_describes_secret_reference_and_model_precedence() {
        let config_schema = component_config_schema_value();
        let secret_description = config_schema["properties"]["api_key_secret"]["description"]
            .as_str()
            .expect("secret description");
        assert!(secret_description.contains("not the API key value itself"));

        let input_schema = operation_input_schema();
        let model_description =
            input_schema["$defs"]["LlmOpenaiRequest"]["properties"]["model"]["description"]
                .as_str()
                .expect("model description");
        assert!(model_description.contains("Takes precedence"));
        assert_eq!(input_schema["oneOf"][0]["required"][0], "input");
        assert_eq!(input_schema["oneOf"][1]["required"][0], "messages");
    }

    #[test]
    fn parse_invocation_payload_accepts_wrapped_input_shape() {
        let payload = parse_invocation_payload(
            &json!({
                "config": {
                    "provider": "ollama"
                },
                "input": {
                    "messages": [{
                        "role": "user",
                        "content": "hello"
                    }]
                }
            })
            .to_string(),
        )
        .expect("wrapped payload");

        assert_eq!(
            payload.config.expect("config").provider,
            LlmProvider::Ollama
        );
        assert_eq!(payload.input.messages.len(), 1);
        assert_eq!(payload.input.messages[0].content, "hello");
    }

    #[test]
    fn parse_invocation_payload_accepts_flattened_request_shape() {
        let payload = parse_invocation_payload(
            &json!({
                "messages": [{
                    "role": "user",
                    "content": "hello"
                }]
            })
            .to_string(),
        )
        .expect("flattened payload");

        assert!(payload.config.is_none());
        assert_eq!(payload.input.messages.len(), 1);
        assert_eq!(payload.input.messages[0].content, "hello");
    }

    #[test]
    fn handle_invocation_accepts_flattened_request_with_top_level_config() {
        let host = MockHost::default().with_response(HttpResponse {
            status: 200,
            headers: Map::from_iter([(
                "content-type".to_string(),
                Value::String("application/json".to_string()),
            )]),
            body: Some(
                json!({
                    "choices": [{
                        "message": {
                            "role": "assistant",
                            "content": "plan"
                        }
                    }]
                })
                .to_string(),
            ),
        });
        let output = handle_invocation(
            DEFAULT_OPERATION,
            &json!({
                "config": {
                    "provider": "ollama",
                    "default_model": "llama3.2",
                    "base_url": "http://127.0.0.1:11434/v1"
                },
                "messages": [{
                    "role": "user",
                    "content": "hello"
                }]
            })
            .to_string(),
            &host,
            None,
        )
        .expect("flattened invocation succeeds");
        let response: LlmOpenaiResponse = serde_json::from_str(&output).expect("response json");
        assert_eq!(response.completion, "plan");
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

    #[test]
    fn setup_apply_payload_decodes_nested_answers_cbor_bytes() {
        let answers_cbor = greentic_types::cbor::canonical::to_canonical_cbor(&json!({
            "provider": "openai",
            "api_key_secret": "OPENAI_API_KEY"
        }))
        .expect("encode answers");
        let payload_cbor =
            greentic_types::cbor::canonical::to_canonical_cbor(&ciborium::value::Value::Map(vec![
                (
                    ciborium::value::Value::Text("mode".to_string()),
                    ciborium::value::Value::Text("default".to_string()),
                ),
                (
                    ciborium::value::Value::Text("current_config_cbor".to_string()),
                    ciborium::value::Value::Null,
                ),
                (
                    ciborium::value::Value::Text("answers_cbor".to_string()),
                    ciborium::value::Value::Bytes(answers_cbor),
                ),
                (
                    ciborium::value::Value::Text("metadata_cbor".to_string()),
                    ciborium::value::Value::Null,
                ),
            ]))
            .expect("encode setup payload");

        let payload = decode_setup_apply_payload(&payload_cbor).expect("decode payload");
        let applied = apply_answers_value(&payload).expect("apply answers");

        assert_eq!(applied["provider"], "openai");
        assert_eq!(applied["api_key_secret"], "OPENAI_API_KEY");
    }

    #[test]
    fn setup_apply_payload_accepts_direct_object_answers_for_compatibility() {
        let payload = SetupApplyPayload {
            mode: Some("default".to_string()),
            answers: Some(json!({
                "provider": "openai",
                "api_key_secret": "OPENAI_API_KEY"
            })),
            ..Default::default()
        };

        let applied = apply_answers_value(&payload).expect("apply answers");
        assert_eq!(applied["api_key_secret"], "OPENAI_API_KEY");
    }
}
