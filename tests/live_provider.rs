use std::collections::HashMap;
use std::time::Duration;

use component_llm_openai::{
    ChatMessage, ComponentError, Host, HttpRequest, HttpResponse, LlmOpenaiConfig,
    LlmOpenaiRequest, LlmOpenaiResponse, LlmProvider, invoke_with_host,
};
use serde_json::{Map, Value};

struct LiveHttpHost {
    client: reqwest::blocking::Client,
    secrets: HashMap<String, String>,
}

impl Host for LiveHttpHost {
    fn fetch_http(
        &self,
        request: HttpRequest,
        _tenant: Option<&component_llm_openai::TenantContext>,
    ) -> Result<HttpResponse, ComponentError> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|err| {
            ComponentError::new("invalid-method", format!("invalid method: {err}"))
        })?;

        let mut builder = self
            .client
            .request(method, &request.url)
            .body(request.body.clone());

        for (key, value) in request.headers {
            if let Some(text) = value.as_str() {
                builder = builder.header(&key, text);
            }
        }

        let response = builder.send().map_err(|err| {
            ComponentError::retryable(
                "http-request-failed",
                format!("live provider call failed: {err}"),
            )
        })?;
        let status = response.status().as_u16();
        let mut headers = Map::new();
        for (key, value) in response.headers() {
            headers.insert(
                key.to_string(),
                Value::String(value.to_str().unwrap_or_default().to_string()),
            );
        }
        let body = response.text().ok();

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }

    fn get_secret(&self, name: &str) -> Result<Option<String>, ComponentError> {
        Ok(self.secrets.get(name).cloned())
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn provider_from_env(raw: &str) -> LlmProvider {
    match raw {
        "openai" => LlmProvider::Openai,
        "ollama" => LlmProvider::Ollama,
        "openrouter" => LlmProvider::Openrouter,
        "together" => LlmProvider::Together,
        "custom" => LlmProvider::Custom,
        other => panic!("unsupported LIVE_LLM_PROVIDER `{other}`"),
    }
}

#[test]
#[ignore = "requires a reachable live LLM endpoint configured via env or .secrets"]
fn live_provider_roundtrip() {
    let provider = provider_from_env(&env_or_default("LIVE_LLM_PROVIDER", "ollama"));
    let base_url = std::env::var("LIVE_LLM_BASE_URL").ok();
    let default_model = std::env::var("LIVE_LLM_MODEL").ok();
    let live_api_key = std::env::var("LIVE_LLM_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let prompt = env_or_default(
        "LIVE_LLM_PROMPT",
        "Respond with exactly the text PONG_TEST_42 and nothing else.",
    );
    let expected = env_or_default("LIVE_LLM_EXPECT_CONTAINS", "PONG_TEST_42");

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .expect("build reqwest client");

    let mut secrets = HashMap::new();
    let api_key_secret = live_api_key
        .as_ref()
        .map(|_| "LIVE_LLM_API_KEY".to_string());
    if let Some(api_key) = live_api_key {
        secrets.insert("LIVE_LLM_API_KEY".to_string(), api_key);
    }

    let host = LiveHttpHost { client, secrets };
    let config = LlmOpenaiConfig {
        provider,
        base_url,
        api_key_secret,
        default_model,
        timeout_ms: Some(60_000),
    };
    let request = LlmOpenaiRequest {
        model: None,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(32),
        extra: None,
    };

    let response: LlmOpenaiResponse =
        invoke_with_host(&host, &config, request).expect("live provider invocation should succeed");

    assert!(
        !response.completion.trim().is_empty(),
        "live provider returned empty completion"
    );
    assert!(
        response.completion.contains(&expected),
        "expected completion to contain `{expected}`, got `{}`",
        response.completion
    );
}
