use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

fn find_secrets_file() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?.join(".secrets");
    if current.is_file() {
        return Some(current);
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(".secrets");
    if manifest_dir.is_file() {
        return Some(manifest_dir);
    }

    None
}

fn parse_secrets_file(path: &Path) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return values;
    };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        let mut value = raw_value.trim().to_string();
        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            value = value[1..value.len() - 1].to_string();
        }
        values.insert(key.trim().to_string(), value);
    }

    values
}

fn load_live_env() -> Option<HashMap<String, String>> {
    let path = find_secrets_file()?;
    Some(parse_secrets_file(&path))
}

fn env_or_loaded(
    key: &str,
    loaded: Option<&HashMap<String, String>>,
    default: Option<&str>,
) -> Option<String> {
    if let Ok(value) = std::env::var(key) {
        return Some(value);
    }

    if let Some(value) = loaded.and_then(|values| values.get(key)).cloned() {
        return Some(value);
    }

    default.map(ToString::to_string)
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
fn live_provider_roundtrip() {
    let loaded = load_live_env();
    if loaded.is_none() && std::env::var("LIVE_LLM_PROVIDER").is_err() {
        eprintln!("No .secrets file or LIVE_LLM_* environment found for live_provider_roundtrip.");
        eprintln!("Set up the live test first:");
        eprintln!("  cp .secrets.sample .secrets");
        eprintln!("  ollama serve");
        eprintln!("  ollama pull llama3:8b");
        eprintln!("Then rerun: cargo test live_provider_roundtrip --test live_provider -- --exact");
        return;
    }

    let provider = provider_from_env(
        &env_or_loaded("LIVE_LLM_PROVIDER", loaded.as_ref(), Some("ollama"))
            .expect("provider default should always be available"),
    );
    let base_url = env_or_loaded("LIVE_LLM_BASE_URL", loaded.as_ref(), None);
    let default_model = env_or_loaded("LIVE_LLM_MODEL", loaded.as_ref(), None);
    let live_api_key = env_or_loaded("LIVE_LLM_API_KEY", loaded.as_ref(), None)
        .filter(|value| !value.trim().is_empty());
    let prompt = env_or_loaded(
        "LIVE_LLM_PROMPT",
        loaded.as_ref(),
        Some("Respond with exactly the text PONG_TEST_42 and nothing else."),
    )
    .expect("prompt default should always be available");
    let expected = env_or_loaded(
        "LIVE_LLM_EXPECT_CONTAINS",
        loaded.as_ref(),
        Some("PONG_TEST_42"),
    )
    .expect("expected default should always be available");

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
