use component_llm_openai::{
    LlmOpenaiConfig, LlmProvider, describe_payload, fixture_apply_config_cbor, i18n_catalog_value,
    i18n_fallback,
};
use serde_json::json;
use std::fs;
use std::path::Path;

#[test]
fn describe_mentions_world() {
    let payload = describe_payload();
    let json: serde_json::Value = serde_json::from_str(&payload).expect("describe should be json");
    assert_eq!(
        json["component"]["world"],
        "greentic:component/component@0.6.0"
    );
    assert_eq!(json["component"]["default_operation"], "handle_message");
    assert!(json["component"]["config_schema"].is_object());
}

#[test]
fn config_defaults_to_openai() {
    let cfg = LlmOpenaiConfig::default();
    assert_eq!(cfg.provider, LlmProvider::Openai);
}

#[test]
fn describe_is_self_contained_and_has_no_external_schema_paths() {
    let payload = describe_payload();
    assert!(!payload.contains("component.schema.json"));
    assert!(!payload.contains("schemas/io"));
}

#[test]
fn english_i18n_catalog_is_embedded_in_rust() {
    let catalog = i18n_catalog_value();
    assert_eq!(catalog["default_locale"], "en");
    assert_eq!(
        catalog["messages"]["en"]["qa.field.api_key_secret.label"],
        "API key secret name"
    );
}

#[test]
fn i18n_fallback_exists_for_known_key() {
    assert_eq!(
        i18n_fallback("qa.choice.provider.custom"),
        Some("Custom OpenAI-compatible endpoint".to_string())
    );
}

#[test]
fn token_only_default_answers_expand_to_openai_config() {
    let cbor = fixture_apply_config_cbor("default", &json!({"api_key_secret": "OPENAI_API_KEY"}))
        .expect("default config cbor");
    let json: serde_json::Value =
        greentic_types::cbor::canonical::from_cbor(&cbor).expect("decode config cbor");
    assert_eq!(json["provider"], "openai");
    assert_eq!(json["api_key_secret"], "OPENAI_API_KEY");
    assert!(json["base_url"].is_null());
}

#[test]
fn personalized_setup_answers_expand_to_custom_config() {
    let cbor = fixture_apply_config_cbor(
        "setup",
        &json!({
            "provider": "custom",
            "base_url": "https://my-llm.example.com/v1",
            "endpoint_requires_api_key": true,
            "api_key_secret": "MY_LLM_API_KEY",
            "default_model": "gpt-oss-120b",
            "timeout_behavior": "custom",
            "timeout_ms": 30000
        }),
    )
    .expect("setup config cbor");
    let json: serde_json::Value =
        greentic_types::cbor::canonical::from_cbor(&cbor).expect("decode config cbor");
    assert_eq!(json["provider"], "custom");
    assert_eq!(json["base_url"], "https://my-llm.example.com/v1");
    assert_eq!(json["default_model"], "gpt-oss-120b");
    assert_eq!(json["timeout_ms"], 30000);
}

#[test]
fn manifest_id_matches_describe_id_and_declares_http_client_capability() {
    let payload = describe_payload();
    let describe: serde_json::Value =
        serde_json::from_str(&payload).expect("describe should be valid json");

    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("component.manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path).expect("read component.manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).expect("manifest should be valid json");

    assert_eq!(manifest["id"], "component-llm-openai");
    assert_eq!(describe["component"]["id"], manifest["id"]);
    assert_eq!(describe["component"]["name"], manifest["name"]);
    assert_eq!(manifest["capabilities"]["host"]["http"]["client"], true);
    assert_eq!(
        manifest["capabilities"]["host"]["secrets"]["required"],
        json!([])
    );
}
