# component-llm-openai

A minimal Rust + WASI-P2 Greentic component scaffolded via `greentic-component new`.

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

## Next Steps

- Implement domain-specific logic inside `src/lib.rs`.
- Extend `schemas/` with the inputs/outputs your component expects.
- Wire additional capabilities or telemetry requirements into `component.manifest.json`.
