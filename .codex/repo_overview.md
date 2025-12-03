# Repository Overview

## 1. High-Level Purpose
- Rust/WASI-P2 Greentic component exposing `greentic:component/node@0.4.0` that issues OpenAI-style `/chat/completions` requests.
- Supports provider profiles (OpenAI, Ollama, OpenRouter, Together, Custom), secrets-based auth, JSON-typed inputs/outputs for chat completions, and a streaming path that parses SSE `data:` chunks into streamed events.

## 2. Main Components and Functionality
- **Path:** `src/lib.rs`  
  **Role:** Core component implementation with config/request/response types, host bindings, HTTP invocation, error mapping, and WASM exports.  
  **Key functionality:** Resolves provider/base URL/model defaults; fetches API keys via greentic secrets; builds POST requests to `{base_url}/chat/completions` with optional `extra` merge; parses OpenAI-style responses into `LlmOpenaiResponse`; maps errors to `NodeError`; streaming mode sets `stream: true` and parses SSE `data:` chunks into `{"delta": ...}` stream events plus the final response. Unit tests cover config/model resolution, auth requirements, and response parsing (including streaming).  
  **Key dependencies / integration points:** `greentic-interfaces-guest` (component-node, http-client, secrets) for host HTTP and secrets; `serde`/`serde_json` for payloads.
- **Path:** `schemas/component.schema.json`  
  **Role:** Component configuration schema.  
  **Key functionality:** Declares `provider`, `base_url`, `api_key_secret`, `default_model`, `timeout_ms` with provider defaults and auth expectations.
- **Path:** `schemas/io/input.schema.json`  
  **Role:** Invocation input schema.  
  **Key functionality:** Optional `config` (component schema) plus `input` (`LlmOpenaiRequest` with chat messages, optional model/temperature/top_p/max_tokens/extra).
- **Path:** `schemas/io/output.schema.json`  
  **Role:** Invocation output schema.  
  **Key functionality:** `LlmOpenaiResponse` with `completion`, conversation messages, and optional raw provider response.
- **Path:** `component.manifest.json`  
  **Role:** Component metadata/capabilities.  
  **Key functionality:** Advertises host HTTP client and secrets capabilities, messaging/telemetry, limits, and wasm artifact/hash placeholder.
- **Path:** `README.md`  
  **Role:** Usage notes.  
  **Key functionality:** Describes supported providers and configuration fields, streaming notes, tenant context propagation, timeout notes, and build steps.
- **Path:** `tests/conformance.rs`  
  **Role:** Integration sanity checks.  
  **Key functionality:** Confirms manifest world entry and default provider.
- **Path:** `ci/local_check.sh`  
  **Role:** Local sanity script.  
  **Key functionality:** Runs `cargo fmt`, `cargo clippy --all-targets -D warnings`, and `cargo test --all-targets`.
- **Path:** `Makefile`  
  **Role:** Helper targets for build/check/lint/test.

## 3. Work In Progress, TODOs, and Stubs
- None observed in code comments or markers.

## 4. Broken, Failing, or Conflicting Areas
- None detected; `cargo test --all-targets` currently passes.

## 5. Notes for Future Work
- Extend with tool-calling support if required by flows.
- Honor `timeout_ms` once host HTTP surfaces timeout support; extend tenant context mapping if additional fields become available.
- Regenerate `component.manifest.json` hashes after producing a release wasm artifact.
