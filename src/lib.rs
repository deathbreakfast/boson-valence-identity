//! Builds and recovers Valence execution context for Boson task dispatch.
//!
//! Runnable: `cargo run -p boson-valence-identity --example wire_factory`
//!
//! Implements [`ExecutionContextFactory`] using Valence's [`ValenceFactory`]. Task handlers
//! use `Box<dyn ExecutionContext>` from the upstream `#[boson::task]` macro and recover
//! [`Valence`] via [`valence_from_context`].
//!
//! ## Features
//!
//! - **Execution context factory** — [`ValenceExecutionContextFactory`] implements Boson's
//!   [`ExecutionContextFactory`] from a [`ValenceFactory`], so `#[boson::task]` handlers can
//!   recover a live [`Valence`] instead of a raw actor blob.
//! - **Invoke-scoped recovery** — [`valence_from_context`] hands a task handler the
//!   [`Valence`] staged for that dispatch (process map keyed by invoke id — async-safe).
//! - **Direct construction** — [`ValenceExecutionContextFactory::build_valence`] builds a
//!   [`Valence`] straight from actor JSON, for callers that don't go through Boson dispatch.
//!
//! ## Security
//!
//! Hosts that expose Boson enqueue to external clients **must** install
//! [`RejectExternalSystemActor`] on the [`ValenceFactory`] (see
//! [`router_config_reject_external_system`]). Internal workers that legitimately mint
//! [`valence::Actor::System`] should set [`valence::ActorTrust::Internal`] on the factory config.
//!
//! # Concern → API
//!
//! | Concern | API |
//! |---------|-----|
//! | Install the factory when building the Boson runtime | [`ValenceExecutionContextFactory::new`] |
//! | Recover `Valence` inside a `#[boson::task]` handler | [`valence_from_context`] |
//! | Build `Valence` directly from actor JSON | [`ValenceExecutionContextFactory::build_valence`] |
//! | Default external-safe router config | [`router_config_reject_external_system`] |
//!
//! Runnable deep dive: `cargo run -p boson-valence-identity --example wire_factory`
//!
//! # Highlights
//!
//! Config → factory → direct build / dispatch recover:
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use boson_core::ExecutionContextFactory;
//! use boson_valence_identity::{
//!     router_config_reject_external_system, valence_from_context, ValenceExecutionContextFactory,
//! };
//! use valence::{
//!     DatabaseRouter, InMemoryBackend, RouterValenceFactory, DEFAULT_IN_MEMORY_ROUTER_KEY,
//! };
//!
//! let mut router = DatabaseRouter::new();
//! router.register(
//!     DEFAULT_IN_MEMORY_ROUTER_KEY.to_string(),
//!     Arc::new(InMemoryBackend::new()),
//! );
//! let valence_factory = RouterValenceFactory::arc(
//!     Arc::new(router),
//!     router_config_reject_external_system(DEFAULT_IN_MEMORY_ROUTER_KEY),
//! );
//! let factory = ValenceExecutionContextFactory::new(valence_factory);
//! let actor = serde_json::json!({"User": {"user_id": "u1"}});
//! let _direct = factory.build_valence(&actor)?;
//! let ctx = factory.build(&actor)?;
//! let _recovered = valence_from_context(ctx.as_ref())?;
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use boson_core::{BosonError, ExecutionContext, ExecutionContextFactory, IdentityError};
use serde_json::Value;
use valence::{RejectExternalSystemActor, RouterValenceFactoryConfig, Valence, ValenceFactory};

static NEXT_INVOKE_ID: AtomicU64 = AtomicU64::new(1);
static STAGED_VALENCE: OnceLock<Mutex<HashMap<u64, Valence>>> = OnceLock::new();

fn staged_map() -> &'static Mutex<HashMap<u64, Valence>> {
    STAGED_VALENCE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Router config that rejects System-shaped actors on the default external trust path.
///
/// Use for any factory that may receive client-supplied `actor_json`. For in-process System
/// workers, clone and set [`valence::ActorTrust::Internal`] on the returned config.
///
/// # Examples
///
/// ```rust,ignore
/// use valence::{RouterValenceFactory, DEFAULT_IN_MEMORY_ROUTER_KEY};
/// use boson_valence_identity::router_config_reject_external_system;
///
/// let config = router_config_reject_external_system(DEFAULT_IN_MEMORY_ROUTER_KEY);
/// let _factory = RouterValenceFactory::arc(router, config);
/// ```
#[must_use]
pub fn router_config_reject_external_system(
    default_backend_key: impl Into<String>,
) -> RouterValenceFactoryConfig {
    RouterValenceFactoryConfig::new(default_backend_key)
        .actor_json_policy(RejectExternalSystemActor)
}

/// Recover the staged [`Valence`] for the current task dispatch.
///
/// Valid only inside a running Boson task handler when
/// [`ValenceExecutionContextFactory`] built the execution context.
///
/// # Errors
///
/// Returns [`BosonError`] when the context label is not an invoke id produced by this
/// factory, or when the staged [`Valence`] was already taken.
///
/// # Examples
///
/// ```rust,ignore
/// use boson_core::ExecutionContextFactory;
/// use boson_valence_identity::{valence_from_context, ValenceExecutionContextFactory};
///
/// let factory = ValenceExecutionContextFactory::new(valence_factory);
/// let ctx = factory.build(&actor_json)?;
/// let valence = valence_from_context(ctx.as_ref())?;
/// let _ = valence.database_router();
/// ```
pub fn valence_from_context(ctx: &dyn ExecutionContext) -> Result<Valence, BosonError> {
    let invoke_id = parse_invoke_id(ctx.label()).ok_or_else(|| {
        BosonError::internal("missing invoke valence (invalid execution context label)")
    })?;
    take_staged(invoke_id)
}

fn parse_invoke_id(label: &str) -> Option<u64> {
    let rest = label.strip_prefix("invoke:")?;
    let id_str = rest.split_once('|').map_or(rest, |(id, _)| id);
    id_str.parse().ok()
}

fn take_staged(invoke_id: u64) -> Result<Valence, BosonError> {
    staged_map()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&invoke_id)
        .ok_or_else(|| BosonError::internal("missing invoke valence"))
}

/// Type alias for [`Valence`] when a handler parameter is typed as `ExecutionContext`.
pub type ExecutionContextAlias = Valence;

/// Wraps a [`ValenceFactory`] as an [`ExecutionContextFactory`].
#[derive(Clone)]
pub struct ValenceExecutionContextFactory {
    inner: Arc<dyn ValenceFactory>,
}

impl ValenceExecutionContextFactory {
    /// Create from host factory.
    ///
    /// Prefer a factory built with [`router_config_reject_external_system`] unless this worker
    /// intentionally accepts System actors (`ActorTrust::Internal`).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use boson_valence_identity::ValenceExecutionContextFactory;
    ///
    /// let factory = ValenceExecutionContextFactory::new(valence_factory);
    /// ```
    pub fn new(inner: Arc<dyn ValenceFactory>) -> Self {
        Self { inner }
    }

    /// Build a [`Valence`] for task dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::InvalidActor`] when the inner [`ValenceFactory`] rejects the
    /// actor JSON (including System actors on the external trust path).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let actor = serde_json::json!({"User": {"user_id": "u1"}});
    /// let valence = factory.build_valence(&actor)?;
    /// let _ = valence.database_router();
    /// ```
    pub fn build_valence(&self, actor_json: &Value) -> Result<Valence, IdentityError> {
        self.inner
            .build(actor_json)
            .map_err(|e| IdentityError::InvalidActor(e.to_string()))
    }

    /// Always errors; use [`valence_from_context`] with the dispatch [`ExecutionContext`].
    ///
    /// # Errors
    ///
    /// Always returns [`BosonError`] directing callers to [`valence_from_context`].
    pub fn take_invoke_valence() -> Result<Valence, BosonError> {
        Err(BosonError::internal(
            "missing invoke valence (use valence_from_context with the dispatch ExecutionContext)",
        ))
    }
}

struct ValenceContext {
    label: String,
    actor_json: Value,
}

impl ExecutionContext for ValenceContext {
    fn label(&self) -> &str {
        &self.label
    }

    fn actor_json(&self) -> &Value {
        &self.actor_json
    }
}

impl ExecutionContextFactory for ValenceExecutionContextFactory {
    fn build(&self, actor_json: &Value) -> Result<Box<dyn ExecutionContext>, IdentityError> {
        let valence = self.build_valence(actor_json)?;
        let invoke_id = NEXT_INVOKE_ID.fetch_add(1, Ordering::Relaxed);
        staged_map()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(invoke_id, valence);
        let actor_label = serde_json::to_string(actor_json).unwrap_or_else(|_| "actor".into());
        let label = format!("invoke:{invoke_id}|{actor_label}");
        Ok(Box::new(ValenceContext {
            label,
            actor_json: actor_json.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boson_core::{BosonError, ExecutionContextFactory, IdentityError};
    use std::sync::Arc;
    use valence::{
        ActorTrust, InMemoryBackend, RouterValenceFactory, ValenceFactory,
        DEFAULT_IN_MEMORY_ROUTER_KEY,
    };

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

    struct FailValenceFactory;

    impl ValenceFactory for FailValenceFactory {
        fn build(&self, _actor_json: &Value) -> valence::Result<valence::Valence> {
            Err(valence::Error::Identity("factory build failed".into()))
        }
    }

    #[test]
    fn external_factory_rejects_system_actor_json() {
        let factory = ValenceExecutionContextFactory::new(mem_factory_external());
        let actor = serde_json::json!({"System": {"operation": "test"}});
        match factory.build(&actor) {
            Ok(_) => panic!("System must be rejected on external trust"),
            Err(IdentityError::InvalidActor(msg)) => assert!(msg.contains("System")),
        }
    }

    #[test]
    fn factory_stages_valence_for_invoke() {
        let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
        let actor = serde_json::json!({"System": {"operation": "test"}});
        let ctx = factory.build(&actor).expect("ok");
        assert_eq!(ctx.actor_json(), &actor);
        assert!(ctx.label().starts_with("invoke:"));
        let v = valence_from_context(ctx.as_ref()).expect("staged");
        let _ = v.database_router();
    }

    #[test]
    fn build_valence_roundtrip() {
        let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
        let v = factory
            .build_valence(&serde_json::json!({"System": {"operation": "t"}}))
            .expect("valence");
        let _ = v.database_router();
    }

    #[test]
    fn take_invoke_valence_without_context_errors() {
        match ValenceExecutionContextFactory::take_invoke_valence() {
            Ok(_) => panic!("expected missing invoke valence"),
            Err(BosonError::Internal { message, .. }) => {
                assert!(message.contains("missing invoke valence"));
            }
            Err(other) => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn valence_from_context_errors_when_already_taken() {
        let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
        let actor = serde_json::json!({"System": {"operation": "test"}});
        let ctx = factory.build(&actor).expect("ok");
        let _ = valence_from_context(ctx.as_ref()).expect("first");
        match valence_from_context(ctx.as_ref()) {
            Ok(_) => panic!("expected unstaged recovery to fail"),
            Err(BosonError::Internal { message, .. }) => {
                assert!(message.contains("missing invoke valence"));
            }
            Err(other) => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn invoke_valence_is_one_shot() {
        let factory = ValenceExecutionContextFactory::new(mem_factory_internal());
        let actor = serde_json::json!({"System": {"operation": "once"}});
        let ctx = factory.build(&actor).expect("ok");
        let _ = valence_from_context(ctx.as_ref()).expect("first take");
        match valence_from_context(ctx.as_ref()) {
            Ok(_) => panic!("second take should fail"),
            Err(BosonError::Internal { message, .. }) => {
                assert!(message.contains("missing invoke valence"));
            }
            Err(other) => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn build_valence_maps_factory_failure() {
        let factory = ValenceExecutionContextFactory::new(Arc::new(FailValenceFactory));
        match factory.build_valence(&serde_json::json!({"System": {"operation": "x"}})) {
            Ok(_) => panic!("build should fail"),
            Err(IdentityError::InvalidActor(msg)) => {
                assert!(msg.contains("factory build failed"));
            }
        }
    }

    #[test]
    fn context_factory_build_maps_identity_error() {
        let factory = ValenceExecutionContextFactory::new(Arc::new(FailValenceFactory));
        match factory.build(&serde_json::json!({"System": {"operation": "x"}})) {
            Ok(_) => panic!("build should fail"),
            Err(IdentityError::InvalidActor(msg)) => {
                assert!(msg.contains("factory build failed"));
            }
        }
    }
}
