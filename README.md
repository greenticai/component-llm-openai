# component-llm-openai

`component-llm-openai` is a Greentic component that sends OpenAI-compatible `/chat/completions` requests to a selected provider. It keeps the component-level config small and lets per-invocation input override request-specific values such as `model`, `messages`, `temperature`, `top_p`, `max_tokens`, and `extra`.

## What the component config owns

The canonical component config is:

- `provider`
- `base_url`
- `api_key_secret`
- `default_model`
- `timeout_ms`

`api_key_secret` is the name of a stored secret to look up at runtime, not the API key value itself.

Static manifest secrets remain empty because the real requirement is conditional:

- `openai`: API key secret usually required
- `openrouter`: API key secret required
- `together`: API key secret required
- `ollama`: API key secret usually not required
- `custom`: depends on the endpoint

## Provider defaults

Built-in providers use explicit runtime defaults unless overridden by component config:

| Provider | Default base URL | Default model |
|---|---|---|
| `openai` | `https://api.openai.com/v1` | `gpt-4.1-mini` |
| `ollama` | `http://localhost:11434/v1` | `llama3:8b` |
| `openrouter` | `https://openrouter.ai/api/v1` | `openrouter/auto` |
| `together` | `https://api.together.xyz/v1` | `togethercomputer/llama-3-8b-instruct` |
| `custom` | no built-in endpoint; you must supply `base_url` | compatibility fallback `gpt-4.1-mini` if no model is supplied |

`custom` is treated specially:

- `base_url` must be supplied
- the setup/default wizard asks for `default_model`
- runtime still keeps the current compatibility fallback model if nothing is configured

## Config precedence

Resolution order is:

1. explicit per-invocation override, such as `input.model`
2. component config
3. runtime provider defaults
4. compatibility fallback only where still needed

In practice, `input.model` overrides component `default_model`, and component `default_model` overrides the provider default.

## Example component configs

OpenAI:

```json
{
  "provider": "openai",
  "api_key_secret": "OPENAI_API_KEY"
}
```

Ollama:

```json
{
  "provider": "ollama"
}
```

OpenRouter:

```json
{
  "provider": "openrouter",
  "api_key_secret": "OPENROUTER_API_KEY",
  "default_model": "openrouter/auto"
}
```

Custom:

```json
{
  "provider": "custom",
  "base_url": "https://my-llm.example.com/v1",
  "api_key_secret": "MY_LLM_API_KEY",
  "default_model": "gpt-oss-120b",
  "timeout_ms": 30000
}
```

## Per-invocation input

Per-invocation input is request-specific and includes:

- `model`
- `messages`
- `temperature`
- `top_p`
- `max_tokens`
- `extra`

Use component config for defaults. Use invocation input for request-by-request behavior.

## Wizard behavior

The component exports Greentic v0.6 QA lifecycle flows for:

- `default`
- `setup`
- `update`
- `remove`

Default mode keeps things minimal:

- choose provider first
- OpenAI/OpenRouter/Together ask only for API key secret name
- Ollama asks nothing else
- Custom asks for base URL, whether an API key is required, optional secret reference, and default model

Setup mode is fuller:

- provider
- standard endpoint yes/no for built-in providers
- base URL when needed
- authentication questions when needed
- default model
- timeout handling
- timeout in milliseconds when custom timeout is selected

Update mode starts by asking which area to change:

- provider
- endpoint
- authentication
- default model
- timeout

Then it asks only the relevant follow-up questions.

Remove mode stays minimal and asks for confirmation.

## Streaming

The current `greentic:component/component@0.6.0` export uses `invoke` with a CBOR envelope and does not expose a separate `invoke_stream` ABI entrypoint. The internal streaming response parser is still present for future use with OpenAI-compatible SSE `data:` chunks.

## Build and test

```bash
cargo test
cargo build --target wasm32-wasip2
```

The generated `component.manifest.json` points at `target/wasm32-wasip2/release/component_llm_openai.wasm`. After rebuilding the release wasm, refresh the manifest hash with `greentic-component inspect --json target/wasm32-wasip2/release/component_llm_openai.wasm`.

## Live gtest

The repo also includes a live `greentic-integration-tester` gtest at [tests/gtests/live/01_live_provider.gtest](/projects/ai/greentic-ng/component-llm-openai/tests/gtests/live/01_live_provider.gtest).

It is designed to:

- use local Ollama by default during local development
- use CI-provided OpenAI-compatible settings when secrets are present in GitHub Actions

Local setup:

```bash
cp .secrets.sample .secrets
```

The file is sourced by `bash`, so keep values shell-safe. In particular, quote values that contain spaces.

The default local `.secrets` example targets Ollama:

```dotenv
LIVE_LLM_PROVIDER=ollama
LIVE_LLM_BASE_URL=http://localhost:11434/v1
LIVE_LLM_MODEL=llama3:8b
LIVE_LLM_API_KEY=
```

Then run:

```bash
ollama serve
ollama pull llama3:8b
greentic-integration-tester run --gtest tests/gtests/live --artifacts-dir artifacts/live-gtests --errors
```

If `.secrets` is missing, the live gtest now exits cleanly and prints the setup steps instead of failing with an unhelpful ignored-test message.

If you prefer, you can also source the same settings manually before running the Rust test directly:

```bash
set -a
. ./.secrets
set +a
cargo test live_provider_roundtrip --test live_provider -- --exact
```

`.secrets` is gitignored and should stay local-only.

CI behavior:

- `.github/workflows/ci.yml` writes a temporary `.secrets` file from CI secrets
- if `OPENAI_API_KEY` is present, the live gtest runs against OpenAI by default using `gpt-5-mini`
- `LIVE_LLM_BASE_URL`, `LIVE_LLM_MODEL`, and `LIVE_LLM_PROVIDER` can optionally override that CI default

`LIVE_LLM_API_KEY` is optional overall. Leave it empty for local Ollama, or provide it for OpenAI, OpenRouter, Together, or a custom authenticated endpoint.
