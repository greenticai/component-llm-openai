use component_llm_openai::{
    LlmOpenaiConfig, LlmProvider, describe_payload, i18n_catalog_value, i18n_fallback,
};

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
