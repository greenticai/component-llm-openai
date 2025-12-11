use component_llm_openai::{LlmOpenaiConfig, LlmProvider, describe_payload};

#[test]
fn describe_mentions_world() {
    let payload = describe_payload();
    let json: serde_json::Value = serde_json::from_str(&payload).expect("describe should be json");
    assert_eq!(
        json["component"]["world"],
        "greentic:component/component@0.5.0"
    );
}

#[test]
fn config_defaults_to_openai() {
    let cfg = LlmOpenaiConfig::default();
    assert_eq!(cfg.provider, LlmProvider::Openai);
}
