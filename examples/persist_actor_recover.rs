//! Persist actor JSON, rebuild Valence in a worker-style recovery path.
//!
//! ## When to use
//! Show how Boson task identity survives enqueue → worker via captured actor JSON.
//!
//! ## Command
//! ```bash
//! CARGO_BUILD_JOBS=1 cargo run --example persist_actor_recover
//! ```
//!
//! ## Success
//! Stderr prints `persist_actor_recover: OK — actor persisted + worker Valence recovered`.
//!
//! ## See also
//! [`ValenceExecutionContextFactory`](boson_valence_identity::ValenceExecutionContextFactory),
//! `wire_factory`.

#![allow(clippy::print_stderr, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use boson_core::ExecutionContextFactory;
use boson_valence_identity::{
    router_config_reject_external_system, valence_from_context, ValenceExecutionContextFactory,
};
use valence::{
    DatabaseRouter, InMemoryBackend, RouterValenceFactory, ValenceFactory,
    DEFAULT_IN_MEMORY_ROUTER_KEY,
};

fn mem_factory_external() -> Arc<dyn ValenceFactory> {
    let mut router = DatabaseRouter::new();
    router.register(
        DEFAULT_IN_MEMORY_ROUTER_KEY.to_string(),
        Arc::new(InMemoryBackend::new()),
    );
    RouterValenceFactory::arc(
        Arc::new(router),
        router_config_reject_external_system(DEFAULT_IN_MEMORY_ROUTER_KEY),
    )
}

fn main() -> Result<()> {
    let factory = ValenceExecutionContextFactory::new(mem_factory_external());
    let actor = serde_json::json!({"User": {"user_id": "persist-user"}});

    // Persist actor JSON the way a host would stash it on a queue message.
    let path: PathBuf = std::env::temp_dir().join("boson-valence-identity-actor.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&actor)?)?;

    let loaded: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let context = factory.build(&loaded)?;
    let recovered = valence_from_context(context.as_ref())?;
    assert_eq!(recovered.actor().user_id(), Some("persist-user"));

    let _ = std::fs::remove_file(&path);
    eprintln!("persist_actor_recover: OK — actor persisted + worker Valence recovered");
    Ok(())
}
