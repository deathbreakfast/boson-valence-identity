# boson-valence-identity examples

Canonical teaching path for Valence-backed Boson execution context — in-memory router;
examples build/recover Valence without starting a queue worker.

## `wire_factory` — build, dispatch, recover Valence

Run when you want to confirm `ValenceExecutionContextFactory` builds User Valence, wraps it in
`ExecutionContext`, and `valence_from_context` recovers the same session inside a task handler.

```bash
cargo run -p boson-valence-identity --example wire_factory
```

Success: stderr prints `wire_factory: System rejected as expected — …` (expected) and
`wire_factory: OK — built and recovered Valence with external System reject`.

## `persist_actor_recover` — file-persisted actor JSON → worker Valence

```bash
CARGO_BUILD_JOBS=1 cargo run --example persist_actor_recover
```

Success: stderr prints `persist_actor_recover: OK — actor persisted + worker Valence recovered`.

Walkthrough: the example registers an in-memory backend with
`router_config_reject_external_system`, builds Valence directly and via `factory.build`,
recovers through `valence_from_context`, then verifies external System JSON fails closed.
Internal workers that need System set `ActorTrust::Internal` on the factory config.

See `examples/wire_factory.rs`, then install the factory on the host runtime so
`#[boson::task]` handlers can call `valence_from_context`.
