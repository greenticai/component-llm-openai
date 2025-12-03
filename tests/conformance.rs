use component_llm_openai::{describe_payload, handle_message};

#[test]
fn describe_mentions_world() {
    let payload = describe_payload();
    let json: serde_json::Value = serde_json::from_str(&payload).expect("describe should be json");
    assert_eq!(json["component"]["world"], "greentic:component/component@0.4.0");
}

#[test]
fn handle_echoes_input() {
    let response = handle_message("invoke", "ping");
    assert!(response.contains("ping"));
}
