//! Stable, zero-authority facade for embedding Agora.
//!
//! This trusted-host distribution groups the engine, serialized volatile
//! service, and authenticated host needed to construct an in-process Agora
//! stack. Runtime policy, transports, credential verification, persistence,
//! and effectful adapters remain choices of the embedding application.

pub use agora_engine as engine;
pub use agora_host as host;
pub use agora_service as service;
