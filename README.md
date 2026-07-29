# boson-valence-identity

[![CI](https://github.com/unified-field-dev/boson-valence-identity/actions/workflows/ci.yml/badge.svg)](https://github.com/unified-field-dev/boson-valence-identity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[GitHub](https://github.com/unified-field-dev/boson-valence-identity) · `cargo doc -p boson-valence-identity --open`

Valence-backed `ExecutionContextFactory` for [Boson](https://github.com/unified-field-dev/boson) task handlers.

```toml
boson-valence-identity = { git = "https://github.com/unified-field-dev/boson-valence-identity" }
```

```rust
use boson_valence_identity::{valence_from_context, ValenceExecutionContextFactory};

// Inside a #[boson::task] handler — recover Valence from the execution context:
let valence = valence_from_context(ctx.as_ref())?;
```

## About

- `ValenceExecutionContextFactory` — reconstruct Valence for Boson task execution
- `valence_from_context` — recover `Valence` from `dyn ExecutionContext` inside a task body

Install the factory when building the Boson host runtime so handlers can call `valence_from_context`.

## Examples

Canonical teaching path and run commands: [examples/README.md](examples/README.md).

## Verify

```bash
export CARGO_BUILD_JOBS=1
cargo test
```

## License

MIT. See [LICENSE](LICENSE), [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
