# component-llm-openai

An OpenAI-style LLM component for Greentic. It calls `/chat/completions` against OpenAI-compatible providers with configurable defaults (OpenAI, Ollama, OpenRouter, Together, Custom) and reads API keys via greentic-secrets.

## Requirements

 - Rust 1.89+
- `wasm32-wasip2` target (`rustup target add wasm32-wasip2`)

## Getting Started

```bash
cargo build --target wasm32-wasip2
cargo test
```

The generated `component.manifest.json` references the release artifact at
`target/wasm32-wasip2/release/component_llm_openai.wasm`. Update the manifest hash by
running `greentic-component inspect --json target/wasm32-wasip2/release/component_llm_openai.wasm`.

## Configuration

- `provider`: `openai` (default), `ollama`, `openrouter`, `together`, `custom`.
- `base_url`: optional; required for `custom` (defaults per provider otherwise).
- `api_key_secret`: secret name resolved via greentic-secrets (required for OpenAI/OpenRouter/Together).
- `default_model`: optional fallback when a request omits `model`.
- `timeout_ms`: optional request timeout if supported by the host HTTP client.

## Streaming

`invoke_stream` sets `stream: true` on the OpenAI-compatible request and emits progress/data/done events. Streaming chunks are parsed from SSE `data:` lines (`choices[0].delta.content`), forwarded as `{"delta": "<chunk>"}` events, followed by the final full response.

## Context & timeouts

- The component now passes tenant/request context from `ExecCtx` into host HTTP calls (where available) to aid observability/routing.
- `timeout_ms` is forwarded in the request struct; enforcement depends on host HTTP support.

## Next Steps

- Extend `src/lib.rs` with streaming/tooling if needed.
- Extend `schemas/` with richer inputs/outputs your component expects.
- Wire additional capabilities or telemetry requirements into `component.manifest.json`.
