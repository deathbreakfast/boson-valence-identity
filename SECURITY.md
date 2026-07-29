# Security Policy

## Supported versions

Security fixes are accepted against the latest `main` branch and tagged releases (`0.1.x`) of this repository's crates (`boson-valence-identity`).

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security-sensitive reports.

Prefer one of the following:

1. **GitHub Security Advisories** — use [Report a vulnerability](https://github.com/unified-field-dev/boson-valence-identity/security/advisories/new) on this repository when available.
2. Contact the maintainers privately via the repository owner listed at https://github.com/unified-field-dev/boson-valence-identity.

Include:

- a description of the issue and its impact
- steps to reproduce or a proof of concept when possible
- affected crate names and versions

We will acknowledge receipt as soon as practical and coordinate a fix and disclosure timeline with you.

## Scope

In scope: vulnerabilities in this repository's published crates and documentation that could cause unsafe production defaults, plus CI/supply-chain issues in this repository.

Out of scope: vulnerabilities solely in third-party dependencies unless this project mishandles them in a security-relevant way.

## Host checklist: Valence factory trust

1. **External enqueue paths** — build the inner [`ValenceFactory`] with
   [`router_config_reject_external_system`](src/lib.rs) (installs
   `RejectExternalSystemActor` at `ActorTrust::External`). Client JSON must not mint
   `Actor::System`.
2. **Internal workers** — when System actors are required for platform jobs, set
   `config.actor_trust = ActorTrust::Internal` on that worker-only factory.
3. **Invoke recovery** — use [`valence_from_context`](src/lib.rs) with the dispatch
   `ExecutionContext`; staged Valence is keyed by invoke id (not thread-local).
