//! Integration tests for the public Boson ↔ Valence execution-context factory.
//!
//! Exercises `ValenceExecutionContextFactory` and `valence_from_context` from outside the
//! crate (same surface hosts wire into Boson runtimes).

use std::sync::Arc;

use boson_core::{BosonError, ExecutionContextFactory, IdentityError};
use boson_valence_identity::{
    router_config_reject_external_system, valence_from_context, ValenceExecutionContextFactory,
};
use serde_json::json;
use valence::{
    ActorTrust, InMemoryBackend, RouterValenceFactory, ValenceFactory, DEFAULT_IN_MEMORY_ROUTER_KEY,
};

fn mem_factory_internal() -> Arc<dyn ValenceFactory> {
    let mut router = valence::DatabaseRouter::new();
    router.register(
        DEFAULT_IN_MEMORY_ROUTER_KEY.to_string(),
        Arc::new(InMemoryBackend::new()),
    );
    let mut config = router_config_reject_external_system(DEFAULT_IN_MEMORY_ROUTER_KEY);
    config.actor_trust = ActorTrust::Internal;
    RouterValenceFactory::arc(Arc::new(router), config)
}

fn mem_factory_external() -> Arc<dyn ValenceFactory> {
    let mut router = valence::DatabaseRouter::new();
    router.register(
        DEFAULT_IN_MEMORY_ROUTER_KEY.to_string(),
        Arc::new(InMemoryBackend::new()),
    );
    RouterValenceFactory::arc(
        Arc::new(router),
        router_config_reject_external_system(DEFAULT_IN_MEMORY_ROUTER_KEY),
    )
}

struct FailValenceFactory;

impl ValenceFactory for FailValenceFactory {
    fn build(&self, _actor_json: &serde_json::Value) -> valence::Result<valence::Valence> {
        Err(valence::Error::Identity(
            "integ factory build failed".into(),
        ))
    }
}

#[test]
fn build_then_recover_valence_happy() {
    let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
    let actor = json!({"System": {"operation": "integ"}});
    let ctx = ExecutionContextFactory::build(&factory, &actor).expect("context");
    assert_eq!(ctx.actor_json(), &actor);
    let valence = valence_from_context(ctx.as_ref()).expect("recover");
    let _ = valence.database_router();
}

#[test]
fn build_valence_direct_happy() {
    let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
    let valence = factory
        .build_valence(&json!({"System": {"operation": "direct"}}))
        .expect("valence");
    let _ = valence.database_router();
}

#[test]
fn external_rejects_system_actor() {
    let factory = ValenceExecutionContextFactory::new(mem_factory_external());
    match ExecutionContextFactory::build(&factory, &json!({"System": {"operation": "external"}})) {
        Ok(_) => panic!("expected System reject"),
        Err(IdentityError::InvalidActor(msg)) => assert!(msg.contains("System")),
    }
}

#[test]
fn recover_twice_is_internal_error() {
    let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
    let actor = json!({"System": {"operation": "noop-ctx"}});
    let ctx = ExecutionContextFactory::build(&factory, &actor).expect("context");
    let _ = valence_from_context(ctx.as_ref()).expect("first");
    match valence_from_context(ctx.as_ref()) {
        Ok(_) => panic!("expected missing invoke valence"),
        Err(BosonError::Internal { message, .. }) => {
            assert!(message.contains("missing invoke valence"));
        }
        Err(other) => panic!("expected Internal, got {other:?}"),
    }
}

#[test]
fn failing_inner_factory_rejects_context_build() {
    let factory = ValenceExecutionContextFactory::new(Arc::new(FailValenceFactory));
    match ExecutionContextFactory::build(&factory, &json!({"System": {"operation": "x"}})) {
        Ok(_) => panic!("expected identity failure"),
        Err(IdentityError::InvalidActor(msg)) => {
            assert!(msg.contains("integ factory build failed"));
        }
    }
}
