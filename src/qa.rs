#![cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]

use std::collections::BTreeMap;

use greentic_types::i18n_text::I18nText;
use greentic_types::schemas::component::v0_6_0::{
    ChoiceOption, ComponentQaSpec, QaMode, Question, QuestionKind, SkipCondition, SkipExpression,
};
use serde_json::{Map, Value, json};

use crate::{LlmOpenaiConfig, LlmProvider, i18n_fallback, key_required};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardMode {
    Default,
    Setup,
    Update,
    Remove,
}

impl WizardMode {
    pub fn as_qa_mode(self) -> QaMode {
        match self {
            Self::Default => QaMode::Default,
            Self::Setup => QaMode::Setup,
            Self::Update => QaMode::Update,
            Self::Remove => QaMode::Remove,
        }
    }
}

#[cfg_attr(test, allow(dead_code))]
pub fn normalize_mode(raw: &str) -> Option<WizardMode> {
    match raw {
        "default" => Some(WizardMode::Default),
        "setup" | "install" => Some(WizardMode::Setup),
        "update" | "upgrade" => Some(WizardMode::Update),
        "remove" => Some(WizardMode::Remove),
        _ => None,
    }
}

pub fn qa_spec(mode: WizardMode) -> ComponentQaSpec {
    let (title, description, questions) = match mode {
        WizardMode::Default => (
            text("qa.default.title"),
            Some(text("qa.default.description")),
            default_questions(),
        ),
        WizardMode::Setup => (
            text("qa.setup.title"),
            Some(text("qa.setup.description")),
            setup_questions(),
        ),
        WizardMode::Update => (
            text("qa.update.title"),
            Some(text("qa.update.description")),
            update_questions(),
        ),
        WizardMode::Remove => (
            text("qa.remove.title"),
            Some(text("qa.remove.description")),
            remove_questions(),
        ),
    };

    ComponentQaSpec {
        mode: mode.as_qa_mode(),
        title,
        description,
        questions,
        defaults: BTreeMap::new(),
    }
}

#[cfg_attr(test, allow(dead_code))]
pub fn i18n_keys() -> Vec<String> {
    let mut keys = std::collections::BTreeSet::new();
    for mode in [
        WizardMode::Default,
        WizardMode::Setup,
        WizardMode::Update,
        WizardMode::Remove,
    ] {
        keys.extend(qa_spec(mode).i18n_keys());
    }
    keys.into_iter().collect()
}

pub fn apply_answers(
    mode: WizardMode,
    current_config: Option<&Value>,
    answers: Option<&Value>,
) -> Result<Value, String> {
    match mode {
        WizardMode::Remove => apply_remove_answers(answers),
        WizardMode::Default | WizardMode::Setup | WizardMode::Update => {
            let mut config = parse_config_value(current_config)?;
            let answers = answers
                .and_then(Value::as_object)
                .ok_or_else(|| "answers must be an object".to_string())?;
            apply_answers_to_config(mode, &mut config, answers)?;
            Ok(config_to_value(&config))
        }
    }
}

#[cfg(test)]
pub fn answers_schema_json() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "component-llm-openai wizard answers",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "provider": {
                "type": "string",
                "enum": ["openai", "ollama", "openrouter", "together", "custom"]
            },
            "use_standard_endpoint": { "type": "boolean" },
            "base_url": { "type": "string" },
            "endpoint_requires_api_key": { "type": "boolean" },
            "api_key_secret": { "type": "string" },
            "default_model": { "type": "string" },
            "timeout_behavior": {
                "type": "string",
                "enum": ["runtime_default", "custom"]
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 0
            },
            "update_area": {
                "type": "string",
                "enum": ["provider", "endpoint", "authentication", "default_model", "timeout"]
            },
            "confirm_remove": { "type": "boolean" }
        }
    })
}

fn apply_remove_answers(answers: Option<&Value>) -> Result<Value, String> {
    let confirmed = answers
        .and_then(|value| value.get("confirm_remove"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !confirmed {
        return Err("remove mode requires confirm_remove=true".to_string());
    }
    Ok(Value::Object(Map::new()))
}

fn parse_config_value(current_config: Option<&Value>) -> Result<LlmOpenaiConfig, String> {
    match current_config {
        Some(Value::Object(_)) => serde_json::from_value(current_config.cloned().unwrap())
            .map_err(|err| format!("current_config must match component config schema: {err}")),
        Some(Value::Null) | None => Ok(LlmOpenaiConfig::default()),
        Some(_) => Err("current_config must be an object".to_string()),
    }
}

fn config_to_value(config: &LlmOpenaiConfig) -> Value {
    serde_json::to_value(config).expect("config should serialize")
}

fn apply_answers_to_config(
    mode: WizardMode,
    config: &mut LlmOpenaiConfig,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    match mode {
        WizardMode::Default => apply_default_answers(config, answers),
        WizardMode::Setup => apply_setup_answers(config, answers),
        WizardMode::Update => apply_update_answers(config, answers),
        WizardMode::Remove => Ok(()),
    }
}

fn apply_default_answers(
    config: &mut LlmOpenaiConfig,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    let provider = required_provider(answers)?;
    config.provider = provider.clone();
    apply_provider_defaults(config, &provider);

    match provider {
        LlmProvider::Openai | LlmProvider::Openrouter | LlmProvider::Together => {
            config.api_key_secret = Some(required_non_empty_string(answers, "api_key_secret")?);
        }
        LlmProvider::Ollama => {}
        LlmProvider::Custom => {
            config.base_url = Some(required_non_empty_string(answers, "base_url")?);
            let needs_key = required_bool(answers, "endpoint_requires_api_key")?;
            config.api_key_secret = if needs_key {
                Some(required_non_empty_string(answers, "api_key_secret")?)
            } else {
                None
            };
            config.default_model = Some(required_non_empty_string(answers, "default_model")?);
        }
    }

    Ok(())
}

fn apply_setup_answers(
    config: &mut LlmOpenaiConfig,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    let provider = required_provider(answers)?;
    config.provider = provider.clone();
    apply_provider_defaults(config, &provider);

    apply_endpoint_answers(config, &provider, answers)?;
    apply_authentication_answers(config, &provider, answers)?;
    apply_default_model_answer(config, answers, true);
    apply_timeout_answers(config, answers)?;

    Ok(())
}

fn apply_update_answers(
    config: &mut LlmOpenaiConfig,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    let area = required_choice(
        answers,
        "update_area",
        &[
            "provider",
            "endpoint",
            "authentication",
            "default_model",
            "timeout",
        ],
    )?;

    match area.as_str() {
        "provider" => {
            let provider = required_provider(answers)?;
            config.provider = provider.clone();
            apply_provider_defaults(config, &provider);
            apply_endpoint_answers(config, &provider, answers)?;
            apply_authentication_answers(config, &provider, answers)?;
            apply_default_model_answer(config, answers, false);
        }
        "endpoint" => {
            let provider = provider_from_answers_or_config(answers, config);
            config.provider = provider.clone();
            apply_endpoint_answers(config, &provider, answers)?;
        }
        "authentication" => {
            let provider = provider_from_answers_or_config(answers, config);
            config.provider = provider.clone();
            apply_authentication_answers(config, &provider, answers)?;
        }
        "default_model" => {
            let model = optional_trimmed_string(answers, "default_model");
            config.default_model = model;
        }
        "timeout" => {
            apply_timeout_answers(config, answers)?;
        }
        _ => unreachable!("validated choice"),
    }

    Ok(())
}

fn apply_endpoint_answers(
    config: &mut LlmOpenaiConfig,
    provider: &LlmProvider,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    if matches!(provider, LlmProvider::Custom) {
        config.base_url = Some(required_non_empty_string(answers, "base_url")?);
        return Ok(());
    }

    let use_standard = required_bool(answers, "use_standard_endpoint")?;
    config.base_url = if use_standard {
        None
    } else {
        Some(required_non_empty_string(answers, "base_url")?)
    };
    Ok(())
}

fn apply_authentication_answers(
    config: &mut LlmOpenaiConfig,
    provider: &LlmProvider,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    if key_required(provider) {
        config.api_key_secret = Some(required_non_empty_string(answers, "api_key_secret")?);
        return Ok(());
    }

    if matches!(provider, LlmProvider::Custom) {
        let needs_key = required_bool(answers, "endpoint_requires_api_key")?;
        config.api_key_secret = if needs_key {
            Some(required_non_empty_string(answers, "api_key_secret")?)
        } else {
            None
        };
        return Ok(());
    }

    config.api_key_secret = None;
    Ok(())
}

fn apply_default_model_answer(
    config: &mut LlmOpenaiConfig,
    answers: &Map<String, Value>,
    preserve_when_missing: bool,
) {
    match optional_string_presence(answers, "default_model") {
        AnswerString::Missing if preserve_when_missing => {}
        AnswerString::Missing | AnswerString::Blank => config.default_model = None,
        AnswerString::Value(value) => config.default_model = Some(value),
    }
}

fn apply_timeout_answers(
    config: &mut LlmOpenaiConfig,
    answers: &Map<String, Value>,
) -> Result<(), String> {
    match required_choice(answers, "timeout_behavior", &["runtime_default", "custom"])?.as_str() {
        "runtime_default" => {
            config.timeout_ms = None;
        }
        "custom" => {
            config.timeout_ms = Some(required_u64(answers, "timeout_ms")?);
        }
        _ => unreachable!("validated choice"),
    }
    Ok(())
}

fn apply_provider_defaults(config: &mut LlmOpenaiConfig, provider: &LlmProvider) {
    config.provider = provider.clone();
    config.base_url = None;

    if matches!(provider, LlmProvider::Ollama | LlmProvider::Custom) {
        config.api_key_secret = None;
    }
}

fn provider_from_answers_or_config(
    answers: &Map<String, Value>,
    config: &LlmOpenaiConfig,
) -> LlmProvider {
    answers
        .get("provider")
        .and_then(Value::as_str)
        .and_then(provider_from_str)
        .unwrap_or_else(|| config.provider.clone())
}

fn default_questions() -> Vec<Question> {
    vec![
        provider_question(None),
        base_url_question(Some(skip_if_provider_not("custom"))),
        custom_requires_api_key_question(Some(skip_if_provider_not("custom"))),
        api_key_secret_question(
            true,
            Some(SkipExpression::Or(vec![
                skip_if_provider_equals("ollama"),
                skip_if_custom_without_api_key(),
            ])),
        ),
        default_model_question(true, Some(skip_if_provider_not("custom"))),
    ]
}

fn setup_questions() -> Vec<Question> {
    vec![
        provider_question(None),
        use_standard_endpoint_question(Some(skip_if_provider_equals("custom"))),
        base_url_question(Some(SkipExpression::And(vec![
            skip_if_provider_not("custom"),
            skip_if_use_standard_endpoint(),
        ]))),
        custom_requires_api_key_question(Some(skip_if_provider_not("custom"))),
        api_key_secret_question(
            true,
            Some(SkipExpression::Or(vec![
                skip_if_provider_equals("ollama"),
                skip_if_custom_without_api_key(),
            ])),
        ),
        default_model_question(false, None),
        timeout_behavior_question(None),
        timeout_ms_question(Some(skip_if_timeout_behavior_not_custom())),
    ]
}

fn update_questions() -> Vec<Question> {
    vec![
        update_area_question(None),
        provider_question(Some(SkipExpression::And(vec![
            skip_if_update_area_not("provider"),
            skip_if_update_area_not("endpoint"),
            skip_if_update_area_not("authentication"),
        ]))),
        use_standard_endpoint_question(Some(SkipExpression::Or(vec![
            SkipExpression::And(vec![
                skip_if_update_area_not("provider"),
                skip_if_update_area_not("endpoint"),
            ]),
            skip_if_provider_equals("custom"),
        ]))),
        base_url_question(Some(SkipExpression::Or(vec![
            SkipExpression::And(vec![
                skip_if_update_area_not("provider"),
                skip_if_update_area_not("endpoint"),
            ]),
            SkipExpression::And(vec![
                skip_if_provider_not("custom"),
                skip_if_use_standard_endpoint(),
            ]),
        ]))),
        custom_requires_api_key_question(Some(SkipExpression::Or(vec![
            SkipExpression::And(vec![
                skip_if_update_area_not("provider"),
                skip_if_update_area_not("authentication"),
            ]),
            skip_if_provider_not("custom"),
        ]))),
        api_key_secret_question(
            true,
            Some(SkipExpression::Or(vec![
                SkipExpression::And(vec![
                    skip_if_update_area_not("provider"),
                    skip_if_update_area_not("authentication"),
                ]),
                skip_if_provider_equals("ollama"),
                skip_if_custom_without_api_key(),
            ])),
        ),
        default_model_question(false, Some(skip_if_update_area_not("default_model"))),
        timeout_behavior_question(Some(skip_if_update_area_not("timeout"))),
        timeout_ms_question(Some(SkipExpression::Or(vec![
            skip_if_update_area_not("timeout"),
            skip_if_timeout_behavior_not_custom(),
        ]))),
    ]
}

fn remove_questions() -> Vec<Question> {
    vec![bool_question(
        "confirm_remove",
        "qa.field.confirm_remove.label",
        "qa.field.confirm_remove.help",
        true,
        Some(json_bool(false)),
        None,
    )]
}

fn provider_question(skip_if: Option<SkipExpression>) -> Question {
    choice_question(
        "provider",
        "qa.field.provider.label",
        "qa.field.provider.help",
        true,
        Some(json_string("openai")),
        vec![
            ("openai", "qa.choice.provider.openai"),
            ("ollama", "qa.choice.provider.ollama"),
            ("openrouter", "qa.choice.provider.openrouter"),
            ("together", "qa.choice.provider.together"),
            ("custom", "qa.choice.provider.custom"),
        ],
        skip_if,
    )
}

fn use_standard_endpoint_question(skip_if: Option<SkipExpression>) -> Question {
    bool_question(
        "use_standard_endpoint",
        "qa.field.use_standard_endpoint.label",
        "qa.field.use_standard_endpoint.help",
        true,
        Some(json_bool(true)),
        skip_if,
    )
}

fn base_url_question(skip_if: Option<SkipExpression>) -> Question {
    text_question(
        "base_url",
        "qa.field.base_url.label",
        "qa.field.base_url.help",
        true,
        None,
        skip_if,
    )
}

fn custom_requires_api_key_question(skip_if: Option<SkipExpression>) -> Question {
    bool_question(
        "endpoint_requires_api_key",
        "qa.field.endpoint_requires_api_key.label",
        "qa.field.endpoint_requires_api_key.help",
        true,
        Some(json_bool(true)),
        skip_if,
    )
}

fn api_key_secret_question(required: bool, skip_if: Option<SkipExpression>) -> Question {
    text_question(
        "api_key_secret",
        "qa.field.api_key_secret.label",
        "qa.field.api_key_secret.help",
        required,
        None,
        skip_if,
    )
}

fn default_model_question(required: bool, skip_if: Option<SkipExpression>) -> Question {
    text_question(
        "default_model",
        "qa.field.default_model.label",
        "qa.field.default_model.help",
        required,
        None,
        skip_if,
    )
}

fn timeout_behavior_question(skip_if: Option<SkipExpression>) -> Question {
    choice_question(
        "timeout_behavior",
        "qa.field.timeout_behavior.label",
        "qa.field.timeout_behavior.help",
        true,
        Some(json_string("runtime_default")),
        vec![
            ("runtime_default", "qa.choice.timeout.runtime_default"),
            ("custom", "qa.choice.timeout.custom"),
        ],
        skip_if,
    )
}

fn timeout_ms_question(skip_if: Option<SkipExpression>) -> Question {
    number_question(
        "timeout_ms",
        "qa.field.timeout_ms.label",
        "qa.field.timeout_ms.help",
        true,
        None,
        skip_if,
    )
}

fn update_area_question(skip_if: Option<SkipExpression>) -> Question {
    choice_question(
        "update_area",
        "qa.field.update_area.label",
        "qa.field.update_area.help",
        true,
        None,
        vec![
            ("provider", "qa.choice.update_area.provider"),
            ("endpoint", "qa.choice.update_area.endpoint"),
            ("authentication", "qa.choice.update_area.authentication"),
            ("default_model", "qa.choice.update_area.default_model"),
            ("timeout", "qa.choice.update_area.timeout"),
        ],
        skip_if,
    )
}

fn text_question(
    id: &str,
    label_key: &str,
    help_key: &str,
    required: bool,
    default: Option<Value>,
    skip_if: Option<SkipExpression>,
) -> Question {
    serde_json::from_value(json!({
        "id": id,
        "label": text(label_key),
        "help": text(help_key),
        "error": null,
        "kind": { "type": "text" },
        "required": required,
        "default": default,
        "skip_if": skip_if
    }))
    .expect("question should deserialize")
}

fn number_question(
    id: &str,
    label_key: &str,
    help_key: &str,
    required: bool,
    default: Option<Value>,
    skip_if: Option<SkipExpression>,
) -> Question {
    serde_json::from_value(json!({
        "id": id,
        "label": text(label_key),
        "help": text(help_key),
        "error": null,
        "kind": { "type": "number" },
        "required": required,
        "default": default,
        "skip_if": skip_if
    }))
    .expect("question should deserialize")
}

fn bool_question(
    id: &str,
    label_key: &str,
    help_key: &str,
    required: bool,
    default: Option<Value>,
    skip_if: Option<SkipExpression>,
) -> Question {
    serde_json::from_value(json!({
        "id": id,
        "label": text(label_key),
        "help": text(help_key),
        "error": null,
        "kind": { "type": "bool" },
        "required": required,
        "default": default,
        "skip_if": skip_if
    }))
    .expect("question should deserialize")
}

fn choice_question(
    id: &str,
    label_key: &str,
    help_key: &str,
    required: bool,
    default: Option<Value>,
    options: Vec<(&str, &str)>,
    skip_if: Option<SkipExpression>,
) -> Question {
    let options: Vec<ChoiceOption> = options
        .into_iter()
        .map(|(value, key)| ChoiceOption {
            value: value.to_string(),
            label: text(key),
        })
        .collect();
    Question {
        id: id.to_string(),
        label: text(label_key),
        help: Some(text(help_key)),
        error: None,
        kind: QuestionKind::Choice { options },
        required,
        default: default.map(to_cbor_value),
        skip_if,
    }
}

fn text(key: &str) -> I18nText {
    I18nText::new(key, i18n_fallback(key))
}

fn to_cbor_value(value: Value) -> ciborium::value::Value {
    serde_json::from_value(value).expect("json should convert to CBOR value")
}

fn json_string(value: &str) -> Value {
    Value::String(value.to_string())
}

fn json_bool(value: bool) -> Value {
    Value::Bool(value)
}

fn required_provider(answers: &Map<String, Value>) -> Result<LlmProvider, String> {
    provider_from_str(&required_choice(
        answers,
        "provider",
        &["openai", "ollama", "openrouter", "together", "custom"],
    )?)
    .ok_or_else(|| {
        "provider must be one of openai, ollama, openrouter, together, or custom".to_string()
    })
}

fn provider_from_str(raw: &str) -> Option<LlmProvider> {
    match raw {
        "openai" => Some(LlmProvider::Openai),
        "ollama" => Some(LlmProvider::Ollama),
        "openrouter" => Some(LlmProvider::Openrouter),
        "together" => Some(LlmProvider::Together),
        "custom" => Some(LlmProvider::Custom),
        _ => None,
    }
}

fn required_choice(
    answers: &Map<String, Value>,
    key: &str,
    allowed: &[&str],
) -> Result<String, String> {
    let value = required_non_empty_string(answers, key)?;
    if allowed.iter().any(|allowed| *allowed == value) {
        Ok(value)
    } else {
        Err(format!("{key} must be one of {}", allowed.join(", ")))
    }
}

fn required_non_empty_string(answers: &Map<String, Value>, key: &str) -> Result<String, String> {
    optional_trimmed_string(answers, key).ok_or_else(|| format!("{key} is required"))
}

fn optional_trimmed_string(answers: &Map<String, Value>, key: &str) -> Option<String> {
    answers
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn required_bool(answers: &Map<String, Value>, key: &str) -> Result<bool, String> {
    answers
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{key} must be a boolean"))
}

fn required_u64(answers: &Map<String, Value>, key: &str) -> Result<u64, String> {
    answers
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{key} must be a non-negative integer"))
}

enum AnswerString {
    Missing,
    Blank,
    Value(String),
}

fn optional_string_presence(answers: &Map<String, Value>, key: &str) -> AnswerString {
    match answers.get(key).and_then(Value::as_str) {
        None => AnswerString::Missing,
        Some(value) if value.trim().is_empty() => AnswerString::Blank,
        Some(value) => AnswerString::Value(value.trim().to_string()),
    }
}

fn skip_if_provider_equals(value: &str) -> SkipExpression {
    SkipExpression::Condition(SkipCondition {
        field: "provider".to_string(),
        equals: Some(to_cbor_value(json_string(value))),
        not_equals: None,
        is_empty: false,
        is_not_empty: false,
    })
}

fn skip_if_provider_not(value: &str) -> SkipExpression {
    SkipExpression::Condition(SkipCondition {
        field: "provider".to_string(),
        equals: None,
        not_equals: Some(to_cbor_value(json_string(value))),
        is_empty: false,
        is_not_empty: false,
    })
}

fn skip_if_update_area_not(value: &str) -> SkipExpression {
    SkipExpression::Condition(SkipCondition {
        field: "update_area".to_string(),
        equals: None,
        not_equals: Some(to_cbor_value(json_string(value))),
        is_empty: false,
        is_not_empty: false,
    })
}

fn skip_if_use_standard_endpoint() -> SkipExpression {
    SkipExpression::Condition(SkipCondition {
        field: "use_standard_endpoint".to_string(),
        equals: Some(to_cbor_value(json_bool(true))),
        not_equals: None,
        is_empty: false,
        is_not_empty: false,
    })
}

fn skip_if_timeout_behavior_not_custom() -> SkipExpression {
    SkipExpression::Condition(SkipCondition {
        field: "timeout_behavior".to_string(),
        equals: None,
        not_equals: Some(to_cbor_value(json_string("custom"))),
        is_empty: false,
        is_not_empty: false,
    })
}

fn skip_if_custom_without_api_key() -> SkipExpression {
    SkipExpression::And(vec![
        skip_if_provider_equals("custom"),
        SkipExpression::Condition(SkipCondition {
            field: "endpoint_requires_api_key".to_string(),
            equals: Some(to_cbor_value(json_bool(false))),
            not_equals: None,
            is_empty: false,
            is_not_empty: false,
        }),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_base_url, default_model};

    fn find_question<'a>(spec: &'a ComponentQaSpec, id: &str) -> &'a Question {
        spec.questions
            .iter()
            .find(|question| question.id == id)
            .expect("question should exist")
    }

    #[test]
    fn default_mode_is_minimal_and_provider_first() {
        let spec = qa_spec(WizardMode::Default);
        assert_eq!(spec.mode, QaMode::Default);
        assert_eq!(spec.questions[0].id, "provider");
        assert!(
            !spec
                .questions
                .iter()
                .any(|question| question.id == "timeout_ms")
        );
        let api_key = find_question(&spec, "api_key_secret");
        assert!(api_key.skip_if.is_some());
    }

    #[test]
    fn setup_mode_includes_endpoint_auth_model_and_timeout() {
        let spec = qa_spec(WizardMode::Setup);
        for id in [
            "provider",
            "use_standard_endpoint",
            "base_url",
            "api_key_secret",
            "default_model",
            "timeout_behavior",
            "timeout_ms",
        ] {
            find_question(&spec, id);
        }
    }

    #[test]
    fn update_mode_starts_with_update_area_and_is_selective() {
        let spec = qa_spec(WizardMode::Update);
        assert_eq!(spec.questions[0].id, "update_area");
        let timeout = find_question(&spec, "timeout_behavior");
        assert!(timeout.skip_if.is_some());
    }

    #[test]
    fn default_apply_answers_for_ollama_keeps_config_small() {
        let answers = json!({
            "provider": "ollama"
        });
        let applied = apply_answers(WizardMode::Default, None, Some(&answers)).expect("applied");
        assert_eq!(applied["provider"], "ollama");
        assert!(applied.get("base_url").unwrap().is_null());
        assert!(applied.get("default_model").unwrap().is_null());
    }

    #[test]
    fn default_apply_answers_for_custom_requires_model_and_base_url() {
        let answers = json!({
            "provider": "custom",
            "base_url": "https://my-llm.example.com/v1",
            "endpoint_requires_api_key": false,
            "default_model": "gpt-oss-120b"
        });
        let applied = apply_answers(WizardMode::Default, None, Some(&answers)).expect("applied");
        assert_eq!(applied["provider"], "custom");
        assert_eq!(applied["base_url"], "https://my-llm.example.com/v1");
        assert_eq!(applied["default_model"], "gpt-oss-120b");
        assert!(applied["api_key_secret"].is_null());
    }

    #[test]
    fn setup_apply_answers_clears_base_url_when_standard_endpoint_is_selected() {
        let current = json!({
            "provider": "openai",
            "base_url": "https://proxy.example/v1",
            "api_key_secret": "OPENAI_API_KEY"
        });
        let answers = json!({
            "provider": "openai",
            "use_standard_endpoint": true,
            "api_key_secret": "OPENAI_API_KEY",
            "default_model": "",
            "timeout_behavior": "runtime_default"
        });
        let applied =
            apply_answers(WizardMode::Setup, Some(&current), Some(&answers)).expect("applied");
        assert!(applied["base_url"].is_null());
        assert!(applied["timeout_ms"].is_null());
    }

    #[test]
    fn update_authentication_for_custom_can_clear_secret() {
        let current = json!({
            "provider": "custom",
            "base_url": "https://my-llm.example.com/v1",
            "api_key_secret": "MY_KEY",
            "default_model": "gpt-oss-120b"
        });
        let answers = json!({
            "update_area": "authentication",
            "provider": "custom",
            "endpoint_requires_api_key": false
        });
        let applied =
            apply_answers(WizardMode::Update, Some(&current), Some(&answers)).expect("applied");
        assert!(applied["api_key_secret"].is_null());
    }

    #[test]
    fn remove_mode_requires_confirmation() {
        let err = apply_answers(WizardMode::Remove, None, Some(&json!({}))).unwrap_err();
        assert!(err.contains("confirm_remove"));
    }

    #[test]
    fn provider_defaults_are_explicit_for_docs_and_tests() {
        assert_eq!(
            default_base_url(&LlmProvider::Openai),
            "https://api.openai.com/v1"
        );
        assert_eq!(default_model(&LlmProvider::Openrouter), "openrouter/auto");
        assert_eq!(
            default_base_url(&LlmProvider::Together),
            "https://api.together.xyz/v1"
        );
    }

    #[test]
    fn answers_schema_includes_update_area_and_timeout_controls() {
        let schema = answers_schema_json();
        assert_eq!(schema["properties"]["update_area"]["type"], "string");
        assert_eq!(schema["properties"]["timeout_ms"]["minimum"], 0);
    }
}
