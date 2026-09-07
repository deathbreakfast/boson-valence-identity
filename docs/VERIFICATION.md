# boson-valence-identity verification

Re-run after code or doc changes. Boson `ExecutionContextFactory` over Valence — covered by
unit + integration tests below.

## Environment

Match [`.github/workflows/ci.yml`](../.github/workflows/ci.yml):

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-boson-valence-identity
export RUSTFLAGS="-D warnings"
```

Toolchain: stable (same as CI).

## PR CI parity

Required PR job `quality`:

| CI step | Local command |
|--------|----------------|
| Formatting | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy --all-targets --all-features -- -D warnings` |
| Test | `cargo test --all-features` |
| cargo doc | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` |

## Unit + integration (CI)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

### TEST_MAP

| Behavior | Level | Happy | Sad | Notes |
|----------|-------|-------|-----|-------|
| `ValenceExecutionContextFactory::new` | unit | wraps `ValenceFactory` | — | construction only |
| `build_valence` | unit + integ | returns usable `Valence` | maps inner `ValenceFactory` failure → `IdentityError::InvalidActor` | direct construction |
| `ExecutionContextFactory::build` | unit + integ | stages invoke valence; context `label` + `actor_json` | maps failure → `IdentityError::InvalidActor` | Boson dispatch path |
| `valence_from_context` / `take_invoke_valence` | unit + integ | recovers staged `Valence` | empty / second take → `BosonError::Internal` (`"missing invoke valence"`) | process `OnceLock` + `Mutex<HashMap>` keyed by invoke id |

## Notes

- Tests may `unwrap`/`expect`; production paths map failures to typed Boson /
  identity errors (no ordinary-path unwrap).
- Sad-path assertions check typed variants and message content, (stronger than `is_err()` alone).
